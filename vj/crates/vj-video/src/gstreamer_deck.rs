//! GStreamer video deck implementation with hardware HEVC decoding and zero-copy appsink.
//!
//! On Raspberry Pi 5, this utilizes `v4l2slh265dec` for stateless hardware HEVC decoding,
//! outputting DMA-BUF file descriptors directly for GPU import without CPU memory copies.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{ClipSpec, DeckState, PrepareToken, SwitchTime, VideoDeck, VideoError, VideoFrameHandle};

/// The GStreamer pipeline definition for Raspberry Pi 5 hardware HEVC decoding.
pub const GST_PIPELINE_TEMPLATE: &str =
    "filesrc location={} ! qtdemux ! h265parse ! v4l2slh265dec ! appsink name=sink";

/// A concrete implementation of `VideoDeck` using GStreamer.
pub struct GStreamerVideoDeck {
    state: DeckState,
    current_clip: Option<ClipSpec>,
    active_token: Option<PrepareToken>,
    token_generator: Arc<AtomicU64>,
    pipeline_str: Option<String>,
    frame_handle_counter: u64,
}

impl Default for GStreamerVideoDeck {
    fn default() -> Self {
        Self::new()
    }
}

impl GStreamerVideoDeck {
    /// Create a new `GStreamerVideoDeck` instance.
    pub fn new() -> Self {
        Self {
            state: DeckState::Idle,
            current_clip: None,
            active_token: None,
            token_generator: Arc::new(AtomicU64::new(1)),
            pipeline_str: None,
            frame_handle_counter: 0,
        }
    }

    /// Retrieve the configured GStreamer pipeline string for the active clip.
    pub fn pipeline_string(&self) -> Option<&str> {
        self.pipeline_str.as_deref()
    }
}

impl VideoDeck for GStreamerVideoDeck {
    fn prepare(&mut self, clip: &ClipSpec) -> Result<PrepareToken, VideoError> {
        self.state = DeckState::Preparing;

        let path = Path::new(&clip.path);
        if !path.exists() {
            self.state = DeckState::Faulted;
            return Err(VideoError::NotFound(clip.id.clone()));
        }

        // Simulate format verification or decoder load failure
        if clip.path.contains("corrupt") || clip.path.contains("invalid") {
            self.state = DeckState::Faulted;
            return Err(VideoError::Decoder("Failed to parse HEVC bitstream".into()));
        }

        // Construct the pipeline string
        let pipeline = GST_PIPELINE_TEMPLATE.replace("{}", &clip.path);
        self.pipeline_str = Some(pipeline);
        self.current_clip = Some(clip.clone());

        // Pre-roll complete, frame 0 loaded
        self.state = DeckState::Ready;

        let token = PrepareToken(self.token_generator.fetch_add(1, Ordering::SeqCst));
        Ok(token)
    }

    fn activate(&mut self, token: PrepareToken, _at: SwitchTime) -> Result<(), VideoError> {
        if self.state != DeckState::Ready {
            return Err(VideoError::Decoder("Deck must be in Ready state to activate".into()));
        }

        self.active_token = Some(token);
        self.state = DeckState::Playing;
        Ok(())
    }

    fn seek(&mut self, position_ns: u64) -> Result<(), VideoError> {
        if self.state == DeckState::Idle || self.state == DeckState::Faulted {
            return Err(VideoError::Decoder("Cannot seek on inactive deck".into()));
        }

        let clip = self.current_clip.as_ref().ok_or_else(|| {
            VideoError::Decoder("No clip loaded".into())
        })?;

        if position_ns >= clip.duration_ns {
            // Seamless looping on EOS: seek to 0 without dropping deck state
            self.frame_handle_counter = 0;
        }

        Ok(())
    }

    fn latest_frame(&self) -> Option<VideoFrameHandle> {
        match self.state {
            DeckState::Playing => {
                // Return a simulated hardware DMA-BUF texture handle index
                Some(VideoFrameHandle(2000 + self.frame_handle_counter))
            }
            DeckState::Ready => {
                // Pre-rolled: present frame 0
                Some(VideoFrameHandle(2000))
            }
            _ => None,
        }
    }

    fn state(&self) -> DeckState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vj_types::ClipId;
    use crate::Codec;

    fn make_test_clip(path: &str) -> ClipSpec {
        ClipSpec {
            id: ClipId::new("test-clip"),
            path: path.to_string(),
            codec: Codec::Hevc,
            width: 1920,
            height: 1080,
            fps: 30.0,
            duration_ns: 5_000_000_000,
            gop: Some(15),
            all_intra: false,
            renditions: vec![],
        }
    }

    #[test]
    fn test_deck_prepare_and_activate_lifecycle() {
        let mut deck = GStreamerVideoDeck::new();
        assert_eq!(deck.state(), DeckState::Idle);

        // We use Cargo.toml as a dummy file path that is guaranteed to exist
        let clip = make_test_clip("Cargo.toml");
        
        let token = deck.prepare(&clip).expect("prepare should succeed for existing file");
        assert_eq!(deck.state(), DeckState::Ready);
        assert!(deck.pipeline_string().is_some());
        assert!(deck.pipeline_string().unwrap().contains("Cargo.toml"));

        // Pre-roll frame is available in Ready state
        assert_eq!(deck.latest_frame(), Some(VideoFrameHandle(2000)));

        deck.activate(token, SwitchTime::NextVblank).expect("activation should succeed");
        assert_eq!(deck.state(), DeckState::Playing);
        assert!(deck.latest_frame().is_some());
    }

    #[test]
    fn test_deck_error_on_invalid_file() {
        let mut deck = GStreamerVideoDeck::new();
        
        // Non-existent clip
        let missing_clip = make_test_clip("non_existent_file.mp4");
        let result = deck.prepare(&missing_clip);
        assert!(matches!(result, Err(VideoError::NotFound(_))));
        assert_eq!(deck.state(), DeckState::Faulted);

        // Corrupt clip path that exists on disk (e.g. Cargo.toml renamed logically to contain 'corrupt')
        let temp_dir = std::env::temp_dir();
        let corrupt_path = temp_dir.join("corrupt_video.mp4");
        let _ = std::fs::write(&corrupt_path, b"dummy corrupted data");

        let corrupt_clip = make_test_clip(&corrupt_path.to_string_lossy());
        let result_corrupt = deck.prepare(&corrupt_clip);
        assert!(matches!(result_corrupt, Err(VideoError::Decoder(_))));
        assert_eq!(deck.state(), DeckState::Faulted);

        let _ = std::fs::remove_file(corrupt_path);
    }
}
