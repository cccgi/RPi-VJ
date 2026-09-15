//! Media analysis: perceptual hashing, quadrant similarity, seam continuity score, and validation.

use crate::probe::VideoMetadata;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// Advisory classification and proposed action for media ingest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposedAction {

    /// Zero-copy copy direct to active show generation
    KeepOriginal,
    /// Transcode directly to 1080p60 HEVC/H.264
    TranscodeStandard,
    /// Crop quad quadrant (top-left / top-right / bottom-left / bottom-right / center)
    CropQuadrant { quadrant: QuadrantCrop },
    /// Apply configurable crossfade repair for bad loop seam
    RepairSeam { crossfade_duration_secs: f32 },
    /// Reject unreadable or completely corrupted/unsupported file
    Reject { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuadrantCrop {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuadrantScores {
    /// Similarity between Top-Left and Top-Right (0.0 to 1.0)
    pub tl_tr: f32,
    /// Similarity between Top-Left and Bottom-Left (0.0 to 1.0)
    pub tl_bl: f32,
    /// Similarity between Top-Left and Bottom-Right (0.0 to 1.0)
    pub tl_br: f32,
    /// Similarity between Top-Right and Bottom-Right (0.0 to 1.0)
    pub tr_br: f32,
    /// Similarity between Bottom-Left and Bottom-Right (0.0 to 1.0)
    pub bl_br: f32,
    /// Whether all 4 quadrants match above the quad threshold (true quad mirror)
    pub is_true_quad_mirror: bool,
    /// Average pairwise similarity across all 4 quadrants
    pub avg_quad_similarity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaAnalysisReport {
    pub file_path: String,
    pub metadata: Option<VideoMetadata>,
    pub decode_valid: bool,
    pub decode_errors: Vec<String>,
    pub perceptual_hashes: Vec<String>,
    pub quadrant_scores: QuadrantScores,
    pub seam_similarity_score: f32,
    pub is_seamless_loop: bool,
    pub is_variable_framerate: bool,
    pub proposed_action: ProposedAction,
    pub advisory_notes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AnalyzerConfig {
    pub quad_similarity_threshold: f32,
    pub seam_similarity_threshold: f32,
    pub crossfade_default_secs: f32,
    pub sample_hash_count: usize,
}

impl Default for AnalyzerConfig {
    fn default() -> Self {
        Self {
            quad_similarity_threshold: 0.92,
            seam_similarity_threshold: 0.85,
            crossfade_default_secs: 0.5,
            sample_hash_count: 5,
        }
    }
}

/// Compute a 64-bit dHash (difference hash) from a 9x8 grayscale image buffer.
pub fn compute_dhash(gray_9x8: &[u8]) -> u64 {
    if gray_9x8.len() < 72 {
        return 0;
    }
    let mut hash = 0u64;
    for row in 0..8 {
        for col in 0..8 {
            let left = gray_9x8[row * 9 + col];
            let right = gray_9x8[row * 9 + col + 1];
            if left > right {
                hash |= 1 << (row * 8 + col);
            }
        }
    }
    hash
}

/// Normalized Hamming similarity (1.0 = identical, 0.0 = completely opposite)
pub fn hash_similarity(a: u64, b: u64) -> f32 {
    let diff = (a ^ b).count_ones();
    1.0 - (diff as f32 / 64.0)
}

/// Compute Normalized Mean Absolute Difference similarity between two raw equal-sized grayscale buffers.
/// 1.0 = identical, 0.0 = completely inverted black/white.
pub fn buffer_similarity(buf_a: &[u8], buf_b: &[u8]) -> f32 {
    if buf_a.is_empty() || buf_a.len() != buf_b.len() {
        return 0.0;
    }
    let total_diff: u64 = buf_a
        .iter()
        .zip(buf_b.iter())
        .map(|(&a, &b)| (a as i32 - b as i32).unsigned_abs() as u64)
        .sum();
    let max_diff = buf_a.len() as f64 * 255.0;
    (1.0 - (total_diff as f64 / max_diff)).clamp(0.0, 1.0) as f32
}

/// Extract a raw grayscale frame buffer (width x height) using ffmpeg at a given timestamp.
pub fn extract_raw_frame(
    path: impl AsRef<Path>,
    timestamp_secs: f64,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let path_str = path.as_ref().to_string_lossy().to_string();
    let vf = format!("scale={}:{}:force_original_aspect_ratio=disable,format=gray", width, height);

    let output = Command::new("ffmpeg")
        .args([
            "-v", "error",
            "-ss", &format!("{:.3}", timestamp_secs.max(0.0)),
            "-i", &path_str,
            "-vframes", "1",
            "-vf", &vf,
            "-f", "rawvideo",
            "-pix_fmt", "gray",
            "pipe:1",
        ])
        .output()
        .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffmpeg frame extraction failed with code {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let expected_len = (width * height) as usize;
    if output.stdout.len() < expected_len {
        return Err(format!(
            "Short read from ffmpeg: expected {} bytes, got {}",
            expected_len,
            output.stdout.len()
        ));
    }

    Ok(output.stdout[..expected_len].to_vec())
}

/// Full media analysis engine: validates decode, computes dHashes, compares quadrants,
/// checks loop seam similarity, and proposes conservative ingest actions.
pub fn analyze_media(
    path: impl AsRef<Path>,
    config: Option<&AnalyzerConfig>,
) -> MediaAnalysisReport {
    let cfg = config.cloned().unwrap_or_default();
    let path_ref = path.as_ref();
    let path_str = path_ref.to_string_lossy().to_string();

    let mut advisory_notes = Vec::new();
    let mut decode_errors = Vec::new();

    // 1. Probe metadata
    let metadata_res = VideoMetadata::probe_file(path_ref);
    let metadata = match metadata_res {
        Ok(m) => Some(m),
        Err(e) => {
            decode_errors.push(format!("Probe error: {}", e));
            None
        }
    };

    if metadata.is_none() {
        return MediaAnalysisReport {
            file_path: path_str,
            metadata: None,
            decode_valid: false,
            decode_errors,
            perceptual_hashes: Vec::new(),
            quadrant_scores: QuadrantScores {
                tl_tr: 0.0,
                tl_bl: 0.0,
                tl_br: 0.0,
                tr_br: 0.0,
                bl_br: 0.0,
                is_true_quad_mirror: false,
                avg_quad_similarity: 0.0,
            },
            seam_similarity_score: 0.0,
            is_seamless_loop: false,
            is_variable_framerate: false,
            proposed_action: ProposedAction::Reject {
                reason: "Unreadable format or missing video stream".into(),
            },
            advisory_notes: vec!["File failed ffprobe interrogation".into()],
        };
    }

    let meta = metadata.as_ref().unwrap();

    // 2. Full or sampled decode validation using ffmpeg null sink
    let decode_status = Command::new("ffmpeg")
        .args([
            "-v", "error",
            "-i", &path_str,
            "-f", "null",
            "-",
        ])
        .output();

    let decode_valid = match decode_status {
        Ok(output) => {
            if !output.status.success() {
                let err_msg = String::from_utf8_lossy(&output.stderr).to_string();
                decode_errors.push(err_msg);
                false
            } else {
                true
            }
        }
        Err(e) => {
            decode_errors.push(format!("Decode execution failed: {}", e));
            false
        }
    };

    if !decode_valid {
        advisory_notes.push("Decode validation emitted errors or warnings".into());
    }

    // 3. Perceptual Hash Sampling (dHash across N timestamps)
    let mut perceptual_hashes = Vec::new();
    let duration = meta.duration_secs.max(0.1);
    let num_samples = cfg.sample_hash_count.max(1);
    for i in 0..num_samples {
        let ts = if num_samples == 1 {
            duration * 0.5
        } else {
            (i as f64 / (num_samples - 1) as f64) * (duration * 0.95)
        };

        if let Ok(raw_9x8) = extract_raw_frame(path_ref, ts, 9, 8) {
            let hash = compute_dhash(&raw_9x8);
            perceptual_hashes.push(format!("{:016x}", hash));
        }
    }

    // 4. Quadrant Analysis: Check for 4-quadrant layout / quad mirror
    // Extract a representative mid-frame at 64x64 resolution, then slice into 4 quadrants (32x32 each)
    let quad_scores = if let Ok(frame_64) = extract_raw_frame(path_ref, duration * 0.5, 64, 64) {
        let mut tl = Vec::with_capacity(32 * 32);
        let mut tr = Vec::with_capacity(32 * 32);
        let mut bl = Vec::with_capacity(32 * 32);
        let mut br = Vec::with_capacity(32 * 32);

        for y in 0..64 {
            for x in 0..64 {
                let pixel = frame_64[y * 64 + x];
                match (y < 32, x < 32) {
                    (true, true) => tl.push(pixel),
                    (true, false) => tr.push(pixel),
                    (false, true) => bl.push(pixel),
                    (false, false) => br.push(pixel),
                }
            }
        }

        let tl_tr = buffer_similarity(&tl, &tr);
        let tl_bl = buffer_similarity(&tl, &bl);
        let tl_br = buffer_similarity(&tl, &br);
        let tr_br = buffer_similarity(&tr, &br);
        let bl_br = buffer_similarity(&bl, &br);

        let avg = (tl_tr + tl_bl + tl_br + tr_br + bl_br) / 5.0;
        let is_true_quad = tl_tr >= cfg.quad_similarity_threshold
            && tl_bl >= cfg.quad_similarity_threshold
            && tl_br >= cfg.quad_similarity_threshold
            && tr_br >= cfg.quad_similarity_threshold
            && bl_br >= cfg.quad_similarity_threshold;

        QuadrantScores {
            tl_tr,
            tl_bl,
            tl_br,
            tr_br,
            bl_br,
            is_true_quad_mirror: is_true_quad,
            avg_quad_similarity: avg,
        }
    } else {
        QuadrantScores {
            tl_tr: 0.0,
            tl_bl: 0.0,
            tl_br: 0.0,
            tr_br: 0.0,
            bl_br: 0.0,
            is_true_quad_mirror: false,
            avg_quad_similarity: 0.0,
        }
    };

    if quad_scores.is_true_quad_mirror {
        advisory_notes.push(format!(
            "True quad mirror detected (avg similarity: {:.2})",
            quad_scores.avg_quad_similarity
        ));
    } else if quad_scores.avg_quad_similarity > 0.70 {
        advisory_notes.push(format!(
            "Independent quadrants detected (avg similarity: {:.2})",
            quad_scores.avg_quad_similarity
        ));
    }

    // 5. First-frame to Last-frame seam continuity score
    let first_frame = extract_raw_frame(path_ref, 0.0, 32, 32);
    let last_frame_ts = (duration - 0.04).max(0.0);
    let last_frame = extract_raw_frame(path_ref, last_frame_ts, 32, 32);

    let seam_score = match (first_frame, last_frame) {
        (Ok(f1), Ok(f2)) => buffer_similarity(&f1, &f2),
        _ => 0.0,
    };

    let is_seamless = seam_score >= cfg.seam_similarity_threshold;
    if !is_seamless {
        advisory_notes.push(format!(
            "Loop seam score is {:.2} (below threshold {:.2})",
            seam_score, cfg.seam_similarity_threshold
        ));
    }

    // 6. Variable or non-standard framerate check
    let is_variable_framerate = meta.fps <= 0.0 || (meta.fps - meta.fps.round()).abs() > 0.005 || meta.fps < 24.0 || meta.fps > 60.5;
    if is_variable_framerate {
        advisory_notes.push(format!(
            "Variable or non-standard framerate detected: {:.3} fps",
            meta.fps
        ));
    }

    // 7. Proposed Action Determination (Conservative & Advisory)
    let proposed_action = if !decode_valid {
        ProposedAction::Reject {
            reason: format!("Corrupted media: {}", decode_errors.join("; ")),
        }
    } else if quad_scores.is_true_quad_mirror {
        ProposedAction::CropQuadrant {
            quadrant: QuadrantCrop::TopLeft,
        }
    } else if !is_seamless && duration >= 0.5 {
        ProposedAction::RepairSeam {
            crossfade_duration_secs: cfg.crossfade_default_secs,
        }
    } else if meta.is_pi5_compliant() && !is_variable_framerate {
        ProposedAction::KeepOriginal
    } else {
        ProposedAction::TranscodeStandard
    };


    MediaAnalysisReport {
        file_path: path_str,
        metadata: Some(meta.clone()),
        decode_valid,
        decode_errors,
        perceptual_hashes,
        quadrant_scores: quad_scores,
        seam_similarity_score: seam_score,
        is_seamless_loop: is_seamless,
        is_variable_framerate,
        proposed_action,
        advisory_notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dhash_and_similarity() {
        let mut gray1 = vec![0u8; 72];
        for i in 0..72 {
            gray1[i] = (i % 9) as u8 * 20;
        }
        let hash1 = compute_dhash(&gray1);
        let hash2 = compute_dhash(&gray1);
        assert_eq!(hash1, hash2);
        assert!((hash_similarity(hash1, hash2) - 1.0).abs() < f32::EPSILON);

        let gray_inverted: Vec<u8> = gray1.iter().map(|&p| 255 - p).collect();
        let hash_inv = compute_dhash(&gray_inverted);
        assert_eq!(hash_similarity(hash1, hash_inv), 0.0);
    }

    #[test]
    fn test_buffer_similarity() {
        let buf_a = vec![128u8; 100];
        let buf_b = vec![128u8; 100];
        assert_eq!(buffer_similarity(&buf_a, &buf_b), 1.0);

        let buf_c = vec![0u8; 100];
        let buf_d = vec![255u8; 100];
        assert_eq!(buffer_similarity(&buf_c, &buf_d), 0.0);
    }
}
