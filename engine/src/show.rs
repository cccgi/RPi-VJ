use crate::control::{Command, State};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clip {
    pub label: String,
    pub path: PathBuf,
    pub thumbnail: Option<PathBuf>,
    #[serde(default)]
    pub quad_mirror: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shader {
    pub label: String,
    pub path: PathBuf,
    #[serde(default = "default_blend")]
    pub blend: crate::control::BlendMode,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    /// Height of this shader's offscreen target; the compositor upscales to the output.
    #[serde(default = "default_render_height")]
    pub render_height: u32,
}
/// Allowed shader render tiers. Width is always derived at 16:9.
pub const RENDER_HEIGHTS: [u32; 4] = [1080, 720, 540, 360];
pub fn tier_size(height: u32) -> Option<(u32, u32)> {
    RENDER_HEIGHTS
        .contains(&height)
        .then_some((height * 16 / 9, height))
}
impl Shader {
    /// Validated by `Show::load`; an unchecked value falls back to full resolution.
    pub fn render_size(&self) -> (u32, u32) {
        tier_size(self.render_height).unwrap_or((1920, 1080))
    }
}
fn default_render_height() -> u32 {
    1080
}
fn default_blend() -> crate::control::BlendMode {
    crate::control::BlendMode::Additive
}
fn default_opacity() -> f32 {
    0.9
}
fn default_color() -> [u8; 3] {
    [35, 65, 95]
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub key: u8,
    pub label: String,
    pub command: Option<Command>,
    #[serde(default = "default_color")]
    pub color: [u8; 3],
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub name: String,
    pub bindings: Vec<Binding>,
}
fn default_bpm() -> f32 {
    128.0
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Show {
    pub version: u32,
    #[serde(default)]
    pub clips: Vec<Clip>,
    pub audio_device: Option<String>,
    #[serde(default)]
    pub shaders: Vec<Shader>,
    #[serde(default)]
    pub pages: Vec<Page>,
    #[serde(default = "default_bpm")]
    pub bpm: f32,
    #[serde(skip)]
    pub warnings: Vec<String>,
}
impl Show {
    pub fn load(path: &Path) -> Result<Self> {
        let mut show: Self = serde_json::from_slice(
            &std::fs::read(path).with_context(|| format!("read show {}", path.display()))?,
        )?;
        ensure!(
            show.version == 1 || show.version == 2,
            "unsupported show version"
        );
        ensure!(
            show.clips.len() <= 4096 && show.shaders.len() <= 1024 && show.pages.len() <= 128,
            "show exceeds asset/page limits"
        );
        ensure!(
            show.bpm.is_finite() && (30.0..=300.0).contains(&show.bpm),
            "invalid show tempo"
        );
        let base = path.parent().unwrap_or(Path::new("."));
        for clip in &mut show.clips {
            ensure!(!clip.label.trim().is_empty(), "clip label is empty");
            clip.path = base.join(&clip.path);
            if !clip.path.is_file() {
                show.warnings
                    .push(format!("missing clip: {}", clip.path.display()));
            }
            if let Some(thumb) = &mut clip.thumbnail {
                *thumb = base.join(&*thumb);
                if !thumb.is_file() {
                    show.warnings
                        .push(format!("missing thumbnail: {}", thumb.display()));
                }
            }
        }
        for shader in &mut show.shaders {
            ensure!(
                shader.opacity.is_finite() && (0.0..=1.0).contains(&shader.opacity),
                "invalid shader opacity"
            );
            ensure!(!shader.label.trim().is_empty(), "shader label is empty");
            ensure!(
                tier_size(shader.render_height).is_some(),
                "invalid render_height {} for shader {}: expected 1080, 720, 540 or 360",
                shader.render_height,
                shader.label
            );
            shader.path = base.join(&shader.path);
            if !shader.path.is_file() {
                show.warnings
                    .push(format!("missing shader: {}", shader.path.display()));
            }
        }
        for page in &show.pages {
            ensure!(!page.name.trim().is_empty(), "page name is empty");
            let mut seen = [false; 32];
            for binding in &page.bindings {
                ensure!(binding.key < 32, "key outside XL");
                ensure!(!seen[binding.key as usize], "duplicate key");
                seen[binding.key as usize] = true;
                if let Some(command) = binding.command {
                    State::from_show(&show)
                        .apply(command)
                        .map_err(anyhow::Error::msg)?;
                }
            }
        }
        Ok(show)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_manifest_defaults_and_empty_show() {
        let s: Show =
            serde_json::from_str(r#"{"version":1,"clips":[],"audio_device":null}"#).unwrap();
        assert!(s.pages.is_empty());
        assert_eq!(s.bpm, 128.0);
    }
    #[test]
    fn migrated_show_keeps_nine_pages_and_missing_assets_bootable() {
        let s =
            Show::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/legacy-show.json"))
                .unwrap();
        assert_eq!(s.pages.len(), 9);
        assert_eq!(s.shaders.len(), 18);
        let state = State::from_show(&s);
        assert!(state.key_in_show(&s, 24).is_some());
        assert!(state.key_in_show(&s, 31).is_some());
        // Manifests written before render tiers existed stay at full resolution.
        assert!(s.shaders.iter().all(|x| x.render_size() == (1920, 1080)));
    }
    fn load_json(name: &str, json: &str) -> Result<Show> {
        let path = std::env::temp_dir().join(format!("rpi-vj-{}-{name}.json", std::process::id()));
        std::fs::write(&path, json).unwrap();
        let show = Show::load(&path);
        let _ = std::fs::remove_file(&path);
        show
    }
    #[test]
    fn render_height_is_optional_and_validated() {
        let show = |extra: &str| {
            format!(
                r#"{{"version":2,"audio_device":null,"shaders":[{{"label":"a","path":"a.fs"{extra}}}]}}"#
            )
        };
        assert_eq!(
            load_json("default", &show("")).unwrap().shaders[0].render_height,
            1080
        );
        for (height, size) in [
            (1080, (1920, 1080)),
            (720, (1280, 720)),
            (540, (960, 540)),
            (360, (640, 360)),
        ] {
            let s = load_json("tier", &show(&format!(r#","render_height":{height}"#))).unwrap();
            assert_eq!(s.shaders[0].render_size(), size);
        }
        for bad in ["0", "480", "719", "2160", "-720", "\"720\"", "720.5"] {
            assert!(
                load_json("bad", &show(&format!(r#","render_height":{bad}"#))).is_err(),
                "{bad}"
            );
        }
    }
}
