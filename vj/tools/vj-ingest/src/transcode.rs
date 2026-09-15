//! Automated video transcoding pipeline using ffmpeg presets.
//! Supports:
//! - Standard 1080p60 HEVC / H.264 Pi-optimized renditions
//! - Policy-approved quadrant crop (TopLeft, TopRight, BottomLeft, BottomRight, Center)
//! - Seamless crossfade loop repair preserving original source files
//! - Priority & cooperative pausing control

use crate::analyzer::QuadrantCrop;
use crate::priority::PauseToken;
use std::path::Path;
use std::process::Command;
use vj_types::Tier;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VideoCodec {
    Hevc,
    H264,
}

#[derive(Clone, Debug)]
pub struct TranscodeOptions {
    pub target_tier: Tier,
    pub codec: VideoCodec,
    pub max_width: u32,
    pub max_height: u32,
    pub target_fps: u32,
    pub crf: u8,
    pub gop_size: u32,
    pub preset: String,
    pub quadrant_crop: Option<QuadrantCrop>,
    pub crossfade_repair: Option<f32>, // duration in seconds if seam repair requested
}

impl Default for TranscodeOptions {
    fn default() -> Self {
        Self {
            target_tier: Tier::Appliance,
            codec: VideoCodec::Hevc,
            max_width: 1920,
            max_height: 1080,
            target_fps: 60,
            crf: 20,
            gop_size: 15,
            preset: "fast".into(),
            quadrant_crop: None,
            crossfade_repair: None,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TranscodeError {
    #[error("ffmpeg process failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("ffmpeg failed with exit status: {0}")]
    ProcessFailed(String),
    #[error("transcode cancelled or paused: {0}")]
    Interrupted(String),
}

/// Builds the ffmpeg video filter chain for scaling, padding, quadrant cropping, and crossfading.
pub fn build_video_filter(options: &TranscodeOptions, source_duration: Option<f64>) -> String {
    let mut filters = Vec::new();

    // 1. Quadrant crop if specified
    if let Some(crop) = &options.quadrant_crop {
        match crop {
            QuadrantCrop::TopLeft => filters.push("crop=iw/2:ih/2:0:0".to_string()),
            QuadrantCrop::TopRight => filters.push("crop=iw/2:ih/2:iw/2:0".to_string()),
            QuadrantCrop::BottomLeft => filters.push("crop=iw/2:ih/2:0:ih/2".to_string()),
            QuadrantCrop::BottomRight => filters.push("crop=iw/2:ih/2:iw/2:ih/2".to_string()),
            QuadrantCrop::Center => filters.push("crop=iw/2:ih/2:iw/4:ih/4".to_string()),
        }
    }

    // 2. Scale & Pad to target dimensions (1080p60)
    let scale_pad = format!(
        "scale={}:{}:force_original_aspect_ratio=decrease,pad={}:{}:(ow-iw)/2:(oh-ih)/2,format=yuv420p",
        options.max_width, options.max_height, options.max_width, options.max_height
    );
    filters.push(scale_pad);

    // 3. Crossfade loop seam repair if requested
    // If crossfade requested and source duration available, append xfade
    if let (Some(xfade_sec), Some(dur)) = (options.crossfade_repair, source_duration) {
        if dur > (xfade_sec as f64 * 2.0) {
            let xfade_str = format!("split[v1][v2];[v1]trim=start=0:end={:.3},setpts=PTS-STARTPTS[a];[v2]trim=start={:.3},setpts=PTS-STARTPTS[b];[b][a]xfade=transition=fade:duration={:.3}:offset=0", dur - (xfade_sec as f64), dur - (xfade_sec as f64), xfade_sec);
            return xfade_str;
        }
    }


    filters.join(",")
}

/// Builds the ffmpeg command arguments for transcoding a video to Pi-native 1080p60 format.
pub fn build_ffmpeg_args(
    input_path: &str,
    output_path: &str,
    options: &TranscodeOptions,
    source_duration: Option<f64>,
) -> Vec<String> {
    let filter = build_video_filter(options, source_duration);

    let (codec_name, tag_args) = match options.codec {
        VideoCodec::Hevc => ("libx265", vec!["-tag:v".into(), "hvc1".into()]),
        VideoCodec::H264 => ("libx264", vec![]),
    };

    let mut args = vec![
        "-y".into(),
        "-i".into(),
        input_path.into(),
        "-c:v".into(),
        codec_name.into(),
    ];

    args.extend(tag_args);

    args.extend([
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-preset".into(),
        options.preset.clone(),
        "-crf".into(),
        options.crf.to_string(),
        "-vf".into(),
        filter,
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
    ]);

    args
}

/// Transcode a source clip to a target file on disk, respecting optional PauseToken.
pub fn transcode_file(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &TranscodeOptions,
    pause_token: Option<&PauseToken>,
) -> Result<(), TranscodeError> {
    if let Some(token) = pause_token {
        token
            .wait_if_paused()
            .map_err(TranscodeError::Interrupted)?;
    }

    let in_str = input.as_ref().to_string_lossy().to_string();
    let out_str = output.as_ref().to_string_lossy().to_string();

    let args = build_ffmpeg_args(&in_str, &out_str, options, None);

    let status = Command::new("ffmpeg").args(&args).status()?;

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
        let args = build_ffmpeg_args("input.mov", "output.mp4", &opts, None);

        assert!(args.contains(&"libx265".to_string()));
        assert!(args.contains(&"hvc1".to_string()));
        assert!(args.contains(&"yuv420p".to_string()));
        assert!(args.contains(&"+faststart".to_string()));
        assert!(args.contains(&"-an".to_string()));
        assert!(args.contains(&"-g".to_string()));
        assert!(args.contains(&"15".to_string()));
    }

    #[test]
    fn builds_h264_quad_crop_arguments() {
        let mut opts = TranscodeOptions::default();
        opts.codec = VideoCodec::H264;
        opts.quadrant_crop = Some(QuadrantCrop::TopLeft);

        let args = build_ffmpeg_args("input.mov", "output.mp4", &opts, None);
        assert!(args.contains(&"libx264".to_string()));
        let vf_idx = args.iter().position(|r| r == "-vf").unwrap();
        let vf_str = &args[vf_idx + 1];
        assert!(vf_str.contains("crop=iw/2:ih/2:0:0"));
        assert!(vf_str.contains("scale=1920:1080"));
    }
}
