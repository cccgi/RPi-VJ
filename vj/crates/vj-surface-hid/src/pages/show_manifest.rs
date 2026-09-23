//! Reads the real `show.json` the "gig" web engine uses, so the native
//! Stream Deck controller can show real pad names/thumbnails instead of
//! generic numbers. Both `rpi-vj-controller` and `vj-broker` run on the
//! same Pi filesystem, so this is a plain file read -- no HTTP client, no
//! new broker API. Anything wrong with the file (missing, malformed, a pad
//! that doesn't exist for a given bank/key) resolves to `None`; callers
//! fall back to today's numeric rendering. This must never panic and must
//! never block startup on a slow/corrupt asset -- one bad thumbnail cannot
//! be allowed to take down the whole deck mid-show.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::graphics::{Rgb, ICON_HEIGHT, ICON_PIXELS, ICON_WIDTH};

#[derive(Clone)]
pub struct PadInfo {
    pub label: String,
    /// True for any pad type other than "clip" (shader, videofx, shaderfx,
    /// blend, system, ...) -- the real show uses several effect/utility pad
    /// types beyond plain shaders, and none of them have a "clip #" to
    /// preserve, so they all render their name instead of a numeric ID.
    pub use_name_label: bool,
    /// Pre-decoded, pre-resized 96x96 thumbnail, if one was configured and
    /// decoded successfully. Decoding happens once at load time, never per
    /// render -- see module docs.
    pub thumbnail: Option<Box<[Rgb; ICON_PIXELS]>>,
}

pub struct ShowManifest {
    /// (bank_index, key) -> pad info. A plain HashMap rather than a
    /// Vec<[Option<PadInfo>; 32]> because most banks don't define all 32
    /// keys (system row keys 24/25/30/31 never appear here) and a sparse
    /// map is simpler than juggling fixed-size arrays of an unsized-ish type.
    pads: HashMap<(u32, u8), PadInfo>,
    bank_count: u32,
}

impl ShowManifest {
    /// Load and parse `show.json` at `path`, decoding any referenced
    /// thumbnails relative to `web_root`. Returns `None` on any failure
    /// (missing file, bad JSON, wrong shape) -- this is a soft-fail path by
    /// design, not an error the caller needs to report.
    pub fn load(path: &Path, web_root: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let banks = json.get("banks")?.as_array()?;

        let mut pads = HashMap::new();
        for (bank_idx, bank) in banks.iter().enumerate() {
            let Some(pad_list) = bank.get("pads").and_then(|p| p.as_array()) else {
                continue;
            };
            for pad in pad_list {
                let Some(key) = pad.get("key").and_then(|k| k.as_u64()) else {
                    continue;
                };
                let Ok(key) = u8::try_from(key) else { continue };
                let label = pad
                    .get("label")
                    .and_then(|l| l.as_str())
                    .unwrap_or_default()
                    .to_string();
                if label.is_empty() {
                    continue;
                }
                // Anything other than "clip" (shader, videofx, shaderfx,
                // blend, system, ...) has no numeric "clip #" worth keeping.
                let use_name_label = pad.get("type").and_then(|t| t.as_str()) != Some("clip");

                let thumbnail = pad
                    .get("thumb")
                    .and_then(|t| t.as_str())
                    .and_then(|rel| decode_thumbnail(&web_root.join(rel)));

                pads.insert(
                    (bank_idx as u32, key),
                    PadInfo {
                        label,
                        use_name_label,
                        thumbnail,
                    },
                );
            }
        }

        Some(Self {
            pads,
            bank_count: banks.len() as u32,
        })
    }

    pub fn pad(&self, bank: u32, key: u8) -> Option<&PadInfo> {
        self.pads.get(&(bank, key))
    }

    /// Number of banks defined in the loaded show, for the Performance
    /// page's bank PREV/NEXT wraparound. `0` if the show defines no banks
    /// at all (callers should fall back to a sane default rather than
    /// dividing/modulo by zero).
    pub fn bank_count(&self) -> u32 {
        self.bank_count
    }
}

/// Decode a thumbnail JPEG to a fixed 96x96 `Rgb` buffer. `None` on any
/// failure (missing file, corrupt image, decode error) -- deliberately
/// swallowed here rather than propagated, per the module's soft-fail
/// contract; a caller that wants to know why can add logging later without
/// changing this function's contract.
fn decode_thumbnail(path: &Path) -> Option<Box<[Rgb; ICON_PIXELS]>> {
    let img = image::open(path).ok()?;
    let resized = img.resize_exact(
        ICON_WIDTH as u32,
        ICON_HEIGHT as u32,
        image::imageops::FilterType::Triangle,
    );
    let rgb = resized.to_rgb8();

    let mut out = Box::new([Rgb::BLACK; ICON_PIXELS]);
    for (i, px) in rgb.pixels().enumerate() {
        out[i] = Rgb::new(px[0], px[1], px[2]);
    }
    Some(out)
}

/// Default on-disk location of the deployed show, matching where
/// `vj-broker`'s `/gig/` route and `launch-renderer.sh` already point.
pub fn default_show_json_path() -> PathBuf {
    PathBuf::from("/opt/rpi-vj/web/show.json")
}

pub fn default_web_root() -> PathBuf {
    PathBuf::from("/opt/rpi-vj/web")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Each call gets its own directory (pid + a monotonic counter, not
    /// just pid) so tests that run in parallel within the same process --
    /// and reuse the same `name`, e.g. "show.json" -- never race on the
    /// same path.
    fn write_temp(name: &str, contents: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "show-manifest-test-{}-{}",
            std::process::id(),
            nonce
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_loads_real_pad_labels_and_types() {
        let json = r#"{
            "banks": [
                { "name": "CLIPS", "pads": [
                    { "key": 0, "type": "clip", "label": "Laser" },
                    { "key": 16, "type": "shader", "label": "TUNNEL" }
                ]}
            ]
        }"#;
        let path = write_temp("show.json", json);
        let manifest = ShowManifest::load(&path, path.parent().unwrap()).expect("should parse");

        let clip = manifest.pad(0, 0).expect("clip pad present");
        assert_eq!(clip.label, "Laser");
        assert!(!clip.use_name_label);
        assert!(clip.thumbnail.is_none(), "no thumb configured");

        let shader = manifest.pad(0, 16).expect("shader pad present");
        assert_eq!(shader.label, "TUNNEL");
        assert!(shader.use_name_label);

        assert!(manifest.pad(0, 5).is_none(), "key 5 has no pad in this bank");
        assert!(manifest.pad(1, 0).is_none(), "bank 1 doesn't exist");
        assert_eq!(manifest.bank_count(), 1);
    }

    #[test]
    fn test_non_clip_non_shader_types_also_use_name_label() {
        // The real show uses several pad types beyond "clip"/"shader"
        // (videofx, shaderfx, blend, system) -- none of them have a
        // numeric "clip #" worth preserving.
        let json = r#"{
            "banks": [
                { "name": "MIX", "pads": [
                    { "key": 0, "type": "blend", "label": "ADD" },
                    { "key": 1, "type": "videofx", "label": "GLITCH" },
                    { "key": 2, "type": "shaderfx", "label": "WARP" },
                    { "key": 3, "type": "system", "label": "BPM TAP" }
                ]}
            ]
        }"#;
        let path = write_temp("show.json", json);
        let manifest = ShowManifest::load(&path, path.parent().unwrap()).expect("should parse");

        for key in 0..=3u8 {
            let pad = manifest.pad(0, key).expect("pad present");
            assert!(pad.use_name_label, "key {key} should use its name, not a number");
        }
    }

    #[test]
    fn test_bank_count_matches_number_of_banks() {
        let json = r#"{
            "banks": [
                { "name": "CLIPS", "pads": [] },
                { "name": "MIX", "pads": [] },
                { "name": "NEON", "pads": [] }
            ]
        }"#;
        let path = write_temp("show.json", json);
        let manifest = ShowManifest::load(&path, path.parent().unwrap()).expect("should parse");
        assert_eq!(manifest.bank_count(), 3);
    }

    #[test]
    fn test_missing_file_returns_none_not_panic() {
        let bogus = PathBuf::from("/definitely/does/not/exist/show.json");
        assert!(ShowManifest::load(&bogus, Path::new("/tmp")).is_none());
    }

    #[test]
    fn test_malformed_json_returns_none_not_panic() {
        let path = write_temp("bad.json", "{ this is not valid json");
        assert!(ShowManifest::load(&path, path.parent().unwrap()).is_none());
    }

    #[test]
    fn test_pad_without_label_is_skipped() {
        let json = r#"{"banks":[{"pads":[{"key":0,"type":"clip"}]}]}"#;
        let path = write_temp("nolabel.json", json);
        let manifest = ShowManifest::load(&path, path.parent().unwrap()).expect("should parse");
        assert!(manifest.pad(0, 0).is_none());
    }
}
