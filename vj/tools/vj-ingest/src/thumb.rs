//! Stream Deck XL 96x96 thumbnail extraction using ffmpeg.

use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum ThumbError {
    #[error("ffmpeg thumbnail extraction failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("thumbnail process exited with error: {0}")]
    Failed(String),
}

/// Builds ffmpeg arguments to extract a single 96x96 center-cropped JPEG thumbnail.
pub fn build_thumb_args(
    video_path: &str,
    thumb_path: &str,
    timestamp_secs: f64,
) -> Vec<String> {
    vec![
        "-y".into(),
        "-ss".into(),
        format!("{:.3}", timestamp_secs.max(0.0)),
        "-i".into(),
        video_path.into(),
        "-vframes".into(),
        "1".into(),
        "-vf".into(),
        "scale=96:96:force_original_aspect_ratio=increase,crop=96:96,format=yuvj420p".into(),
        "-q:v".into(),
        "2".into(),
        thumb_path.into(),
    ]
}

/// Extract a 96x96 JPEG thumbnail from a video file.
pub fn extract_thumbnail(
    video_path: impl AsRef<Path>,
    thumb_path: impl AsRef<Path>,
    timestamp_secs: f64,
) -> Result<(), ThumbError> {
    let in_str = video_path.as_ref().to_string_lossy().to_string();
    let out_str = thumb_path.as_ref().to_string_lossy().to_string();

    let args = build_thumb_args(&in_str, &out_str, timestamp_secs);

    let status = Command::new("ffmpeg")
        .args(&args)
        .status()?;

    if !status.success() {
        return Err(ThumbError::Failed(format!(
            "ffmpeg thumb extraction exited with status {:?}",
            status.code()
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_96x96_streamdeck_crop_filter() {
        let args = build_thumb_args("clip.mp4", "thumb.jpg", 1.5);
        assert!(args.contains(&"scale=96:96:force_original_aspect_ratio=increase,crop=96:96,format=yuvj420p".to_string()));
        assert!(args.contains(&"1.500".to_string()));
    }
}
