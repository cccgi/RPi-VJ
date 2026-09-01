//! Ingest and show preparation library for the VJ engine.

pub mod probe;
pub mod thumb;
pub mod transcode;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use vj_types::{ClipId, ClipManifestEntry, ShowManifest, Tier};

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("probe error: {0}")]
    Probe(#[from] probe::ProbeError),
    #[error("transcode error: {0}")]
    Transcode(#[from] transcode::TranscodeError),
    #[error("thumbnail error: {0}")]
    Thumb(#[from] thumb::ThumbError),
    #[error("show error: {0}")]
    Show(#[from] vj_types::ShowError),
}

/// Initialize a new show bundle folder structure.
pub fn init_show(name: &str, out_dir: impl AsRef<Path>) -> Result<ShowManifest, IngestError> {
    let base = out_dir.as_ref();
    fs::create_dir_all(base.join("media").join("hevc"))?;
    fs::create_dir_all(base.join("media").join("hap"))?;
    fs::create_dir_all(base.join("thumbs"))?;
    fs::create_dir_all(base.join("shaders"))?;

    let show = ShowManifest::new(name);
    let toml = show.to_toml()?;
    fs::write(base.join("show.toml"), toml)?;

    Ok(show)
}

/// Ingest and process a video clip into a show bundle.
///
/// 1. Probes the video file for codec and resolution.
/// 2. If non-compliant for Pi 5 (Appliance tier), automatically transcodes to 1080p60 HEVC.
/// 3. Extracts a 96x96 JPEG thumbnail for Stream Deck XL.
/// 4. Updates `show.toml` with the new clip entry.
pub fn ingest_clip(
    input_file: impl AsRef<Path>,
    show_dir: impl AsRef<Path>,
    options: Option<transcode::TranscodeOptions>,
) -> Result<ClipManifestEntry, IngestError> {
    let in_path = input_file.as_ref();
    let base = show_dir.as_ref();

    let stem = in_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("clip")
        .replace(' ', "_")
        .to_lowercase();
    let clip_id = ClipId::new(&stem);

    let meta = probe::VideoMetadata::probe_file(in_path)?;
    let opts = options.unwrap_or_default();

    // Ensure output folders exist
    fs::create_dir_all(base.join("media").join("hevc"))?;
    fs::create_dir_all(base.join("thumbs"))?;

    let rel_hevc_path = format!("media/hevc/{}_pi.mp4", stem);
    let target_hevc = base.join(&rel_hevc_path);

    if meta.is_pi5_compliant() && in_path.extension().map_or(false, |ext| ext == "mp4") {
        // Already compliant, copy directly
        fs::copy(in_path, &target_hevc)?;
    } else {
        // Transcode to compliant Pi 5 HEVC format
        transcode::transcode_file(in_path, &target_hevc, &opts)?;
    }

    // Extract 96x96 Stream Deck thumbnail
    let rel_thumb_path = format!("thumbs/{}.jpg", stem);
    let target_thumb = base.join(&rel_thumb_path);
    let mid_timestamp = (meta.duration_secs / 2.0).clamp(0.0, 5.0);
    thumb::extract_thumbnail(in_path, &target_thumb, mid_timestamp)?;

    let mut renditions = HashMap::new();
    renditions.insert(Tier::Appliance, rel_hevc_path);

    let entry = ClipManifestEntry {
        id: clip_id,
        title: in_path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| stem.clone()),
        duration_ms: (meta.duration_secs * 1000.0) as u64,
        thumb_path: rel_thumb_path,
        renditions,
    };

    // Update show.toml if it exists
    let show_toml_path = base.join("show.toml");
    if show_toml_path.exists() {
        let content = fs::read_to_string(&show_toml_path)?;
        let mut show = ShowManifest::from_toml(&content)?;
        show.clips.retain(|c| c.id != entry.id);
        show.clips.push(entry.clone());
        fs::write(&show_toml_path, show.to_toml()?)?;
    }

    Ok(entry)
}

/// Copy all show assets (show.toml, media, thumbs, shaders) to a USB drive for Pi 5 appliance deployment.
pub fn export_to_usb(
    show_dir: impl AsRef<Path>,
    usb_target: impl AsRef<Path>,
) -> Result<(), IngestError> {
    let src = show_dir.as_ref();
    let dst = usb_target.as_ref();

    if !src.exists() {
        return Err(IngestError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Show directory '{}' does not exist", src.display()),
        )));
    }

    fs::create_dir_all(dst)?;
    println!("Exporting show bundle from '{}' to USB drive '{}'...", src.display(), dst.display());

    // 1. Copy show.toml
    let toml_src = src.join("show.toml");
    if toml_src.exists() {
        fs::copy(&toml_src, dst.join("show.toml"))?;
        println!("  -> Copied show.toml");
    }

    // 2. Copy media/hevc
    let media_src = src.join("media").join("hevc");
    if media_src.exists() {
        let media_dst = dst.join("media").join("hevc");
        fs::create_dir_all(&media_dst)?;
        for entry in fs::read_dir(media_src)? {
            let entry = entry?;
            if entry.path().is_file() {
                fs::copy(entry.path(), media_dst.join(entry.file_name()))?;
                println!("  -> Copied media: {}", entry.file_name().to_string_lossy());
            }
        }
    }

    // 3. Copy thumbs
    let thumb_src = src.join("thumbs");
    if thumb_src.exists() {
        let thumb_dst = dst.join("thumbs");
        fs::create_dir_all(&thumb_dst)?;
        for entry in fs::read_dir(thumb_src)? {
            let entry = entry?;
            if entry.path().is_file() {
                fs::copy(entry.path(), thumb_dst.join(entry.file_name()))?;
            }
        }
        println!("  -> Copied all button thumbnails");
    }

    // 4. Copy shaders
    let shader_src = src.join("shaders");
    if shader_src.exists() {
        let shader_dst = dst.join("shaders");
        fs::create_dir_all(&shader_dst)?;
        for entry in fs::read_dir(shader_src)? {
            let entry = entry?;
            if entry.path().is_file() {
                fs::copy(entry.path(), shader_dst.join(entry.file_name()))?;
            }
        }
        println!("  -> Copied all ISF shaders");
    }

    println!("\n✅ USB Export Complete! Ready to plug into Raspberry Pi 5.");
    Ok(())
}

/// Sync show directory directly to Raspberry Pi 5 over local network using rsync/scp.
pub fn sync_to_pi(
    show_dir: impl AsRef<Path>,
    target_host: &str,
) -> Result<(), IngestError> {
    let src = show_dir.as_ref();
    let src_str = format!("{}/", src.to_string_lossy());
    let remote_dest = format!("{}:/home/pi/show/", target_host);

    println!("⚡ Syncing '{}' directly to '{}'...", src.display(), remote_dest);

    let status = std::process::Command::new("rsync")
        .args(["-avz", "--progress", &src_str, &remote_dest])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("\n✅ Network Sync Complete! Pi 5 show is updated.");
            Ok(())
        }
        _ => {
            println!("  (rsync unavailable, falling back to scp...)");
            let scp_status = std::process::Command::new("scp")
                .args(["-r", &src_str, &remote_dest])
                .status()?;
            if scp_status.success() {
                println!("\n✅ Network Sync Complete via scp!");
                Ok(())
            } else {
                Err(IngestError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "Failed to transfer files over network",
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_show_creates_directories_and_manifest() {
        let temp_dir = std::env::temp_dir().join(format!("vj_test_show_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);

        let show = init_show("Test Gig", &temp_dir).expect("init show failed");
        assert_eq!(show.name, "Test Gig");
        assert!(temp_dir.join("show.toml").exists());
        assert!(temp_dir.join("media/hevc").exists());
        assert!(temp_dir.join("thumbs").exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
