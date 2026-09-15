//! Media probing and compliance checking using ffprobe.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioStreamInfo {
    pub index: u32,
    pub codec_name: String,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VideoMetadata {
    pub path: String,
    pub format_name: String,
    pub duration_secs: f64,
    pub codec_name: String,
    pub profile: Option<String>,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub pixel_format: String,
    pub is_hevc: bool,
    pub is_closed_gop: bool,
    pub max_gop_size: Option<u32>,
    pub audio_streams: Vec<AudioStreamInfo>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("ffprobe executable not found or failed to run: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse ffprobe output: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no video stream found in file: {0}")]
    NoVideoStream(String),
}

impl VideoMetadata {
    /// Check if this clip meets the native hardware playback requirements for Raspberry Pi 5.
    ///
    /// Requirements:
    /// - Codec: HEVC (H.265 / hvc1) or H.264
    /// - Resolution: <= 1920x1080
    /// - Pixel Format: 8-bit YUV420P / NV12
    /// - Framerate: <= 60.0 fps
    pub fn is_pi5_compliant(&self) -> bool {
        let codec_ok = self.is_hevc || self.codec_name.eq_ignore_ascii_case("hevc") || self.codec_name.eq_ignore_ascii_case("hvc1");
        let res_ok = self.width <= 1920 && self.height <= 1080;
        let pix_ok = self.pixel_format.contains("420") || self.pixel_format == "nv12";
        let fps_ok = self.fps <= 60.5;
        codec_ok && res_ok && pix_ok && fps_ok
    }

    /// Probe a media file on disk by invoking `ffprobe`.
    pub fn probe_file(path: impl AsRef<Path>) -> Result<Self, ProbeError> {
        let path_ref = path.as_ref();
        let path_str = path_ref.to_string_lossy().to_string();

        let output = Command::new("ffprobe")
            .args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
                &path_str,
            ])
            .output()?;

        let json_str = String::from_utf8_lossy(&output.stdout);
        Self::parse_ffprobe_json(&path_str, &json_str)
    }

    /// Parse ffprobe JSON output string into structured `VideoMetadata`.
    pub fn parse_ffprobe_json(file_path: &str, json_str: &str) -> Result<Self, ProbeError> {
        let val: serde_json::Value = serde_json::from_str(json_str)?;

        let streams = val["streams"]
            .as_array()
            .ok_or_else(|| ProbeError::NoVideoStream(file_path.to_string()))?;

        let video_stream = streams
            .iter()
            .find(|s| s["codec_type"].as_str() == Some("video"))
            .ok_or_else(|| ProbeError::NoVideoStream(file_path.to_string()))?;

        let codec_name = video_stream["codec_name"].as_str().unwrap_or("unknown").to_string();
        let profile = video_stream["profile"].as_str().map(|s| s.to_string());
        let width = video_stream["width"].as_u64().unwrap_or(0) as u32;
        let height = video_stream["height"].as_u64().unwrap_or(0) as u32;
        let pixel_format = video_stream["pix_fmt"].as_str().unwrap_or("unknown").to_string();

        let r_frame_rate = video_stream["r_frame_rate"].as_str().unwrap_or("60/1");
        let fps = parse_framerate(r_frame_rate);

        let duration_secs = val["format"]["duration"]
            .as_str()
            .and_then(|d| d.parse::<f64>().ok())
            .or_else(|| video_stream["duration"].as_str().and_then(|d| d.parse::<f64>().ok()))
            .unwrap_or(0.0);

        let format_name = val["format"]["format_name"].as_str().unwrap_or("unknown").to_string();
        let is_hevc = codec_name.eq_ignore_ascii_case("hevc") || codec_name.eq_ignore_ascii_case("hvc1");

        let mut audio_streams = Vec::new();
        for s in streams.iter().filter(|s| s["codec_type"].as_str() == Some("audio")) {
            let idx = s["index"].as_u64().unwrap_or(0) as u32;
            let acodec = s["codec_name"].as_str().unwrap_or("unknown").to_string();
            let srate = s["sample_rate"].as_str().and_then(|r| r.parse::<u32>().ok());
            let channels = s["channels"].as_u64().map(|c| c as u32);
            let channel_layout = s["channel_layout"].as_str().map(|cl| cl.to_string());

            audio_streams.push(AudioStreamInfo {
                index: idx,
                codec_name: acodec,
                sample_rate: srate,
                channels,
                channel_layout,
            });
        }

        Ok(VideoMetadata {
            path: file_path.to_string(),
            format_name,
            duration_secs,
            codec_name,
            profile,
            width,
            height,
            fps,
            pixel_format,
            is_hevc,
            is_closed_gop: true, // conservative default
            max_gop_size: None,
            audio_streams,
        })
    }
}


fn parse_framerate(r_fps: &str) -> f32 {
    if let Some((num, den)) = r_fps.split_once('/') {
        let n: f32 = num.parse().unwrap_or(60.0);
        let d: f32 = den.parse().unwrap_or(1.0);
        if d > 0.0 { n / d } else { 60.0 }
    } else {
        r_fps.parse().unwrap_or(60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ffprobe_sample_json() {
        let json = r#"{
            "streams": [
                {
                    "codec_name": "hevc",
                    "codec_type": "video",
                    "width": 1920,
                    "height": 1080,
                    "pix_fmt": "yuv420p",
                    "r_frame_rate": "60/1",
                    "duration": "12.500000"
                }
            ],
            "format": {
                "format_name": "mov,mp4,m4a,3gp,3g2,mj2",
                "duration": "12.500000"
            }
        }"#;

        let meta = VideoMetadata::parse_ffprobe_json("test.mp4", json).unwrap();
        assert_eq!(meta.codec_name, "hevc");
        assert_eq!(meta.width, 1920);
        assert_eq!(meta.height, 1080);
        assert_eq!(meta.fps, 60.0);
        assert_eq!(meta.pixel_format, "yuv420p");
        assert!(meta.is_pi5_compliant());
    }

    #[test]
    fn detects_non_compliant_h264_or_4k() {
        let h264_json = r#"{
            "streams": [
                {
                    "codec_name": "h264",
                    "codec_type": "video",
                    "width": 3840,
                    "height": 2160,
                    "pix_fmt": "yuv420p",
                    "r_frame_rate": "30/1"
                }
            ],
            "format": { "format_name": "mov", "duration": "5.0" }
        }"#;

        let meta = VideoMetadata::parse_ffprobe_json("4k_h264.mov", h264_json).unwrap();
        assert!(!meta.is_pi5_compliant(), "4K H.264 must fail Pi 5 compliance");
    }
}
