//! Clip index, deck pool, and the decode backend seam.
//!
//! This crate never touches a GPU API. Decoders hand out opaque
//! `VideoFrameHandle`s and `vj-render` performs the import.

use serde::{Deserialize, Serialize};
use vj_types::{ClipId, Tier};

pub mod gstreamer_deck;
pub use gstreamer_deck::{GStreamerVideoDeck, GST_PIPELINE_TEMPLATE};

/// The three caching tiers, which must never be conflated. Only
/// `FrameReady` satisfies a hard sub-50 ms trigger budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Warmth {
    /// Nothing but a path.
    Cold,
    /// File bytes are in the page cache. Removes storage latency only.
    PageCache,
    /// Container metadata parsed and the first GOP read. Decoder startup remains.
    Demuxed,
    /// A live decoder holds a presentable first frame.
    FrameReady,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    Hevc,
    H264,
    Hap,
    HapQ,
    ProRes,
    Other,
}

impl Codec {
    /// Whether this codec has a hardware decode path on the given tier.
    pub fn hardware_on(self, tier: Tier) -> bool {
        match tier {
            // BCM2712 has one codec block and it only does HEVC.
            Tier::Appliance => matches!(self, Codec::Hevc),
            Tier::Portable => matches!(self, Codec::Hevc | Codec::H264 | Codec::ProRes),
            Tier::Workstation => matches!(self, Codec::Hevc | Codec::H264),
        }
    }

    /// HAP carries BC1/BC3 blocks. V3D has no S3TC/BC support, so on the
    /// appliance tier it would need a per-frame CPU transcode — worse than
    /// the hardware HEVC path.
    pub fn viable_on(self, tier: Tier) -> bool {
        !matches!((self, tier), (Codec::Hap | Codec::HapQ, Tier::Appliance))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipSpec {
    pub id: ClipId,
    pub path: String,
    pub codec: Codec,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub duration_ns: u64,
    pub gop: Option<u32>,
    pub all_intra: bool,
    /// Which tiers have a rendition of this clip on disk.
    pub renditions: Vec<Tier>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrepareToken(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchTime {
    NextVblank,
    At { ns: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoFrameHandle(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckState {
    Idle,
    Preparing,
    Ready,
    Playing,
    Faulted,
}

#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("clip not found: {0}")]
    NotFound(ClipId),
    #[error("codec {0:?} is not viable on tier {1:?}")]
    UnviableCodec(Codec, Tier),
    #[error("no rendition of {0} for tier {1:?}")]
    MissingRendition(ClipId, Tier),
    #[error("decoder error: {0}")]
    Decoder(String),
}

pub trait VideoDeck: Send {
    fn prepare(&mut self, clip: &ClipSpec) -> Result<PrepareToken, VideoError>;
    fn activate(&mut self, token: PrepareToken, at: SwitchTime) -> Result<(), VideoError>;
    fn seek(&mut self, position_ns: u64) -> Result<(), VideoError>;
    fn latest_frame(&self) -> Option<VideoFrameHandle>;
    fn state(&self) -> DeckState;
}

/// Check a clip against a tier before a show tries to load it.
pub fn validate(clip: &ClipSpec, tier: Tier) -> Result<(), VideoError> {
    if !clip.codec.viable_on(tier) {
        return Err(VideoError::UnviableCodec(clip.codec, tier));
    }
    if !clip.renditions.contains(&tier) {
        return Err(VideoError::MissingRendition(clip.id.clone(), tier));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(codec: Codec, renditions: Vec<Tier>) -> ClipSpec {
        ClipSpec {
            id: ClipId::new("c"),
            path: "/x.mp4".into(),
            codec,
            width: 1920,
            height: 1080,
            fps: 60.0,
            duration_ns: 10_000_000_000,
            gop: Some(15),
            all_intra: false,
            renditions,
        }
    }

    #[test]
    fn pi_hardware_decodes_hevc_only() {
        assert!(Codec::Hevc.hardware_on(Tier::Appliance));
        assert!(!Codec::H264.hardware_on(Tier::Appliance));
    }

    #[test]
    fn hap_is_rejected_on_the_appliance_tier() {
        let err = validate(&clip(Codec::Hap, vec![Tier::Appliance]), Tier::Appliance);
        assert!(matches!(
            err,
            Err(VideoError::UnviableCodec(Codec::Hap, Tier::Appliance))
        ));
    }

    #[test]
    fn missing_rendition_fails_loudly_rather_than_half_loading() {
        let err = validate(&clip(Codec::Hevc, vec![Tier::Workstation]), Tier::Appliance);
        assert!(matches!(err, Err(VideoError::MissingRendition(..))));
    }

    #[test]
    fn warmth_is_ordered_so_only_the_top_tier_meets_the_budget() {
        assert!(Warmth::FrameReady > Warmth::Demuxed);
        assert!(Warmth::Demuxed > Warmth::PageCache);
        assert!(Warmth::PageCache > Warmth::Cold);
    }
}
