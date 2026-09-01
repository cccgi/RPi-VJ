//! Automated video transcoding pipeline using ffmpeg presets.

use std::path::Path;
use std::process::Command;
use vj_types::Tier;

#[derive(Clone, Debug)]
pub struct TranscodeOptions {
    pub target_tier: Tier,
    pub max_width: u32,
    pub max_height: u32,
    pub target_fps: u32,
    pub crf: u8,
    pub gop_size: u32,
    pub preset: String,
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            target_tier: Tier::Appliance,
            max_width: 1920,
            max_height: 1080,
            target_fps: 60,
            crf: 20,
            gop_size: 15,
            preset: "fast".into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    #[error("ffmpeg process failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg failed with exit status: {0}")]
    ProcessFailed(String),
}

/// Builds the ffmpeg command arguments for transcoding a video to Pi 5-native HEVC format.
pub fn build_pi5_ffmpeg_args(
    input_path: &str,
    output_path: &str,
    options: &TranscodeOptions,
) -> Vec<String> {
    let scale_filter = format!(
        "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,format=yuv420p",
        options.max_width, options.max_height, options.max_width, options.max_height
    );

    vec![
        "-y".into(),
        "-i".into(),
        input_path.into(),
        "-c:v".into(),
        "libx265".into(),
        "-tag:v".into(),
        "hvc1".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-preset".into(),
        options.preset.clone(),
        "-crf".into(),
        options.crf.to_string(),
        "-vf".into(),
        scale_filter,
        "-r".into(),
        options.target_fps.to_string(),
        "-g".into(),
        options.gop_size.to_string(),
        "-keyint_min".into(),
        options.gop_size.to_string(),
        "-sc_threshold".into(),
        "0".into(),
        "-movflags".into(),
        "+faststart".into(),
        "-an".into(),
        output_path.into(),
    ]
}

/// Transcode a source clip to a target file on disk.
pub fn transcode_file(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &TranscodeOptions,
) -> Result<(), TranscodeError> {
    let in_str = input.as_ref().to_string_lossy().to_string();
    let out_str = output.as_ref().to_string_lossy().to_string();

    let args = build_pi5_ffmpeg_args(&in_str, &out_str, options);

    let status = Command::new("ffmpeg")
        .args(&args)
        .status()?;

    if !status.success() {
        return Err(TranscodeError::ProcessFailed(format!(
            "ffmpeg exited with status {:?}",
            status.code()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_pi5_hevc_arguments() {
        let opts = TranscodeOptions::default();
        let args = build_pi5_ffmpeg_args("input.mov", "output.mp4", &opts);

        assert!(args.contains(&"libx265".to_string()));
        assert!(args.contains(&"hvc1".to_string()));
        assert!(args.contains(&"yuv420p".to_string()));
        assert!(args.contains(&"+faststart".to_string()));
        assert!(args.contains(&"-an".to_string()));
        assert!(args.contains(&"-g".to_string()));
        assert!(args.contains(&"15".to_string()));
    }
}
