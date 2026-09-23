use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use vj_ingest::analyzer::{analyze_media, ProposedAction, QuadrantCrop};
use vj_ingest::thumb::extract_thumbnail;
use vj_ingest::transcode::{transcode_file, TranscodeOptions};

struct SyntheticCorpus {
    dir: PathBuf,
}

impl SyntheticCorpus {
    fn new(test_name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("vj_corpus_{}_{}", test_name, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn path(&self, filename: &str) -> PathBuf {
        self.dir.join(filename)
    }

    /// Generate a seamless solid color loop (1 sec, 30fps)
    fn generate_seamless_loop(&self, name: &str) -> PathBuf {
        let target = self.path(name);
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "color=c=blue:s=320x240:d=1.0:r=30",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                target.to_str().unwrap(),
            ])
            .status()
            .expect("ffmpeg failed");
        assert!(status.success());
        target
    }

    /// Generate a video with a severe seam discontinuity and distinct quadrant content
    fn generate_bad_seam(&self, name: &str) -> PathBuf {
        let target = self.path(name);
        // Animate 4 distinct quadrants (red, green, blue, yellow) then flash white at the end (t >= 0.8)
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "color=c=black:s=320x240:d=1.0:r=30,drawbox=x=0:y=0:w=160:h=120:color=red:t=fill,drawbox=x=160:y=0:w=160:h=120:color=green:t=fill,drawbox=x=0:y=120:w=160:h=120:color=blue:t=fill,drawbox=x=160:y=120:w=160:h=120:color=yellow:t=fill[bg];[bg]drawbox=x=0:y=0:w=320:h=240:color=white:t=fill:enable='gte(t,0.8)'",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                target.to_str().unwrap(),
            ])
            .status()
            .expect("ffmpeg failed");
        assert!(status.success());
        target
    }

    /// Generate a true quad mirror video: all 4 quadrants identical
    fn generate_true_quad_mirror(&self, name: &str) -> PathBuf {
        let target = self.path(name);
        // Create 1 quadrant, then tile into 2x2 grid
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "color=c=red:s=160x120:d=1.0:r=30",
                "-filter_complex", "[0:v][0:v]hstack[top];[0:v][0:v]hstack[bot];[top][bot]vstack[out]",
                "-map", "[out]",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                target.to_str().unwrap(),
            ])
            .status()
            .expect("ffmpeg failed");
        assert!(status.success());
        target
    }

    /// Generate non-quad video: 4 quadrants each with completely different colors
    fn generate_non_quad(&self, name: &str) -> PathBuf {
        let target = self.path(name);
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi", "-i", "color=c=red:s=160x120:d=1.0:r=30",
                "-f", "lavfi", "-i", "color=c=green:s=160x120:d=1.0:r=30",
                "-f", "lavfi", "-i", "color=c=blue:s=160x120:d=1.0:r=30",
                "-f", "lavfi", "-i", "color=c=yellow:s=160x120:d=1.0:r=30",
                "-filter_complex", "[0:v][1:v]hstack[top];[2:v][3:v]hstack[bot];[top][bot]vstack[out]",
                "-map", "[out]",
                "-c:v", "libx264",
                "-pix_fmt", "yuv420p",
                target.to_str().unwrap(),
            ])
            .status()
            .expect("ffmpeg failed");
        assert!(status.success());
        target
    }

    /// Generate variable rate / non-standard fps video
    fn generate_variable_fps(&self, name: &str) -> PathBuf {
        let target = self.path(name);
        let status = Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "lavfi",
                "-i", "testsrc=duration=1:size=320x240:rate=23.976",
                "-c:v", "libx264",
                "-r", "23.976",
                "-pix_fmt", "yuv420p",
                target.to_str().unwrap(),
            ])
            .status()
            .expect("ffmpeg failed");
        assert!(status.success());
        target
    }


    /// Generate corrupted file
    fn generate_corrupted(&self, name: &str) -> PathBuf {
        let target = self.path(name);
        let mut file = File::create(&target).unwrap();
        file.write_all(b"CORRUPTED_TRUNCATED_MP4_HEADER_MOOV").unwrap();
        target
    }

    /// Generate unsupported format
    fn generate_unsupported(&self, name: &str) -> PathBuf {
        let target = self.path(name);
        let mut file = File::create(&target).unwrap();
        file.write_all(b"UNKNOWN_BINARY_DATA").unwrap();
        target
    }
}

impl Drop for SyntheticCorpus {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn test_corpus_seamless_loop_analysis_and_transcode() {
    let corpus = SyntheticCorpus::new("seamless");
    let clip = corpus.generate_seamless_loop("seamless.mp4");

    let report = analyze_media(&clip, None);
    assert!(report.decode_valid);
    assert!(report.is_seamless_loop);
    assert!(report.seam_similarity_score >= 0.85);

    // Transcode to Appliance 1080p60 HEVC
    let out = corpus.path("seamless_1080p.mp4");
    let opts = TranscodeOptions::default();
    transcode_file(&clip, &out, &opts, None).expect("transcode seamless failed");

    assert!(out.exists());
    let out_report = analyze_media(&out, None);
    assert!(out_report.decode_valid);
    let meta = out_report.metadata.unwrap();
    assert_eq!(meta.width, 1920);
    assert_eq!(meta.height, 1080);
    assert_eq!(meta.fps, 60.0);
}

#[test]
fn test_corpus_bad_seam_detection_and_repair() {
    let corpus = SyntheticCorpus::new("bad_seam");
    let clip = corpus.generate_bad_seam("bad_seam.mp4");

    let report = analyze_media(&clip, None);
    assert!(report.decode_valid);
    assert!(!report.is_seamless_loop);
    assert!(matches!(report.proposed_action, ProposedAction::RepairSeam { .. }));

    // Transcode with crossfade repair requested
    let out = corpus.path("repaired.mp4");
    let mut opts = TranscodeOptions::default();
    opts.crossfade_repair = Some(0.2);
    transcode_file(&clip, &out, &opts, None).expect("transcode crossfade failed");
    assert!(out.exists());
}

#[test]
fn test_corpus_true_quad_mirror_vs_non_quad() {
    let corpus = SyntheticCorpus::new("quad");
    let quad_clip = corpus.generate_true_quad_mirror("quad.mp4");
    let non_quad_clip = corpus.generate_non_quad("non_quad.mp4");

    let quad_report = analyze_media(&quad_clip, None);
    assert!(quad_report.quadrant_scores.is_true_quad_mirror);
    assert!(quad_report.quadrant_scores.avg_quad_similarity >= 0.90);
    assert!(matches!(
        quad_report.proposed_action,
        ProposedAction::CropQuadrant { quadrant: QuadrantCrop::TopLeft }
    ));

    // Crop top-left quadrant and transcode
    let out_quad = corpus.path("cropped_quad.mp4");
    let mut opts = TranscodeOptions::default();
    opts.quadrant_crop = Some(QuadrantCrop::TopLeft);
    transcode_file(&quad_clip, &out_quad, &opts, None).expect("quad crop transcode failed");
    assert!(out_quad.exists());

    let non_quad_report = analyze_media(&non_quad_clip, None);
    assert!(!non_quad_report.quadrant_scores.is_true_quad_mirror);
}

#[test]
fn test_corpus_variable_framerate_and_corrupt_unsupported() {
    let corpus = SyntheticCorpus::new("errors");
    let vfr = corpus.generate_variable_fps("vfr.mp4");
    let corrupt = corpus.generate_corrupted("bad.mp4");
    let unsupp = corpus.generate_unsupported("unknown.xyz");

    // VFR
    let vfr_report = analyze_media(&vfr, None);
    assert!(vfr_report.decode_valid);
    assert!(vfr_report.is_variable_framerate);

    // Corrupt
    let corrupt_report = analyze_media(&corrupt, None);
    assert!(!corrupt_report.decode_valid || corrupt_report.metadata.is_none());
    assert!(matches!(corrupt_report.proposed_action, ProposedAction::Reject { .. }));

    // Unsupported
    let unsupp_report = analyze_media(&unsupp, None);
    assert!(!unsupp_report.decode_valid || unsupp_report.metadata.is_none());
    assert!(matches!(unsupp_report.proposed_action, ProposedAction::Reject { .. }));
}

#[test]
fn test_thumbnailer_streamdeck_96x96() {
    let corpus = SyntheticCorpus::new("thumb");
    let clip = corpus.generate_seamless_loop("thumb_source.mp4");
    let thumb_path = corpus.path("icon.jpg");

    extract_thumbnail(&clip, &thumb_path, 0.5).expect("thumbnail extraction failed");
    assert!(thumb_path.exists());
    assert!(fs::metadata(&thumb_path).unwrap().len() > 0);
}
