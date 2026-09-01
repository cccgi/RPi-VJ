//! Show manifest, bank specification, and show bundle schema.
//!
//! A `.vjshow` bundle contains a `show.toml` manifest, a `media/` directory
//! with per-tier transcoded clips, a `thumbs/` directory with 96x96 Stream Deck
//! JPEG button icons, and a `shaders/` directory with ISF shaders.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{
    BlendMode, ClipId, DeckId, LayerId, Quantize, ShaderId, Tier, TransitionSpec,
};

pub const SHOW_SCHEMA_VERSION: u32 = 1;

/// Top-level manifest for a VJ show package.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShowManifest {
    pub schema_version: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_authored_tier")]
    pub authored_tier: Tier,
    #[serde(default = "default_bpm")]
    pub bpm_default: f32,
    #[serde(default = "default_beats_per_bar")]
    pub beats_per_bar: u8,
    pub requirements: ShowRequirements,
    #[serde(default)]
    pub degradations: HashMap<Tier, TierDegradation>,
    #[serde(default)]
    pub layers: Vec<LayerConfig>,
    #[serde(default)]
    pub banks: Vec<BankSpec>,
    #[serde(default)]
    pub clips: Vec<ClipManifestEntry>,
    #[serde(default)]
    pub shaders: Vec<ShaderManifestEntry>,
}

fn default_authored_tier() -> Tier {
    Tier::Workstation
}

fn default_bpm() -> f32 {
    128.0
}

fn default_beats_per_bar() -> u8 {
    4
}

/// Baseline show resource requirements as authored.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShowRequirements {
    #[serde(default = "default_min_tier")]
    pub min_tier: Tier,
    pub video_decks: u16,
    pub shader_layers: u16,
    #[serde(default = "default_scale")]
    pub render_scale: f32,
}

fn default_min_tier() -> Tier {
    Tier::Appliance
}

fn default_scale() -> f32 {
    1.0
}

/// Declared degradation for lower-power tiers (e.g. Raspberry Pi 5).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TierDegradation {
    pub video_decks: u16,
    pub shader_layers: u16,
    pub render_scale: f32,
    #[serde(default)]
    pub disable_layers: Vec<String>,
    #[serde(default)]
    pub shader_quality: Option<String>,
}

/// Configuration for a compositing layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayerConfig {
    pub id: LayerId,
    pub name: String,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
    #[serde(default)]
    pub blend: BlendMode,
    #[serde(default)]
    pub default_shader: Option<ShaderId>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_opacity() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

/// A 4x8 bank of slots, matching the physical 32-key layout of Stream Deck XL.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BankSpec {
    pub id: u32,
    pub name: String,
    /// 4 rows x 8 columns = 32 keys
    pub grid: [[Option<GridSlot>; 8]; 4],
}

impl BankSpec {
    pub fn empty(id: u32, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            grid: Default::default(),
        }
    }

    pub fn get_slot(&self, row: usize, col: usize) -> Option<&GridSlot> {
        if row < 4 && col < 8 {
            self.grid[row][col].as_ref()
        } else {
            None
        }
    }

    pub fn set_slot(&mut self, row: usize, col: usize, slot: Option<GridSlot>) {
        if row < 4 && col < 8 {
            self.grid[row][col] = slot;
        }
    }
}

/// Content or action assigned to a specific Stream Deck XL key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GridSlot {
    /// Plays or triggers a video clip.
    Clip {
        clip_id: ClipId,
        #[serde(default)]
        deck: DeckId,
        #[serde(default = "default_true")]
        looping: bool,
        #[serde(default)]
        transition: TransitionSpec,
        #[serde(default)]
        quantize: Quantize,
        #[serde(default)]
        thumb_path: Option<String>,
    },
    /// Activates an audio-reactive ISF shader on a layer.
    Shader {
        shader_id: ShaderId,
        layer: LayerId,
        #[serde(default)]
        thumb_path: Option<String>,
    },
    /// Performs a system or performance action.
    Action {
        action: SystemAction,
        label: String,
        #[serde(default)]
        color: Option<[u8; 3]>,
    },
}

/// System action triggered by a controller button.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemAction {
    BankNext,
    BankPrev,
    BankSelect(u32),
    Blackout,
    TapTempo,
    BpmUp,
    BpmDown,
    BeatSync,
    PhaseNudgeHalf,
    QuantizeCycle,
    DeckStop(DeckId),
    MasterOpacity(u8),
}

/// Clip entry in the manifest index.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipManifestEntry {
    pub id: ClipId,
    pub title: String,
    pub duration_ms: u64,
    pub thumb_path: String,
    /// Per-tier file paths relative to the show bundle root.
    pub renditions: HashMap<Tier, String>,
}

/// Shader entry in the manifest index.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShaderManifestEntry {
    pub id: ShaderId,
    pub name: String,
    pub file_path: String,
    #[serde(default)]
    pub thumb_path: Option<String>,
    #[serde(default)]
    pub is_audio_reactive: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ShowError {
    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(u32),
    #[error("running tier {0:?} is below show minimum requirement {1:?}")]
    TierBelowMinimum(Tier, Tier),
    #[error("clip '{0}' is missing a rendition for tier {1:?}")]
    MissingRendition(ClipId, Tier),
    #[error("show serialization error: {0}")]
    Serialization(String),
}

impl ShowManifest {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema_version: SHOW_SCHEMA_VERSION,
            name: name.into(),
            description: None,
            authored_tier: Tier::Workstation,
            bpm_default: 128.0,
            beats_per_bar: 4,
            requirements: ShowRequirements {
                min_tier: Tier::Appliance,
                video_decks: 2,
                shader_layers: 2,
                render_scale: 1.0,
            },
            degradations: HashMap::new(),
            layers: vec![
                LayerConfig {
                    id: LayerId(0),
                    name: "Video Layer 1".into(),
                    opacity: 1.0,
                    blend: BlendMode::Alpha,
                    default_shader: None,
                    enabled: true,
                },
                LayerConfig {
                    id: LayerId(1),
                    name: "Shader Layer 1".into(),
                    opacity: 1.0,
                    blend: BlendMode::Additive,
                    default_shader: None,
                    enabled: true,
                },
            ],
            banks: vec![BankSpec::empty(0, "Main Bank")],
            clips: Vec::new(),
            shaders: Vec::new(),
        }
    }

    pub fn to_toml(&self) -> Result<String, ShowError> {
        toml::to_string_pretty(self).map_err(|e| ShowError::Serialization(e.to_string()))
    }

    pub fn from_toml(content: &str) -> Result<Self, ShowError> {
        let manifest: Self = toml::from_str(content).map_err(|e| ShowError::Serialization(e.to_string()))?;
        if manifest.schema_version != SHOW_SCHEMA_VERSION {
            return Err(ShowError::UnsupportedSchema(manifest.schema_version));
        }
        Ok(manifest)
    }

    pub fn validate_for_tier(&self, tier: Tier) -> Result<(), ShowError> {
        if tier < self.requirements.min_tier {
            return Err(ShowError::TierBelowMinimum(tier, self.requirements.min_tier));
        }
        for clip in &self.clips {
            if !clip.renditions.contains_key(&tier) {
                return Err(ShowError::MissingRendition(clip.id.clone(), tier));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_manifest_roundtrips_toml() {
        let mut show = ShowManifest::new("Warehouse Rave 2026");
        show.banks[0].set_slot(
            0,
            0,
            Some(GridSlot::Clip {
                clip_id: ClipId::new("tunnel_loop"),
                deck: DeckId(0),
                looping: true,
                transition: TransitionSpec::default(),
                quantize: Quantize::Bar,
                thumb_path: Some("thumbs/tunnel_loop.jpg".into()),
            }),
        );
        show.banks[0].set_slot(
            0,
            7,
            Some(GridSlot::Action {
                action: SystemAction::Blackout,
                label: "BLACKOUT".into(),
                color: Some([255, 0, 0]),
            }),
        );

        let mut renditions = HashMap::new();
        renditions.insert(Tier::Appliance, "media/hevc/tunnel_pi.mp4".into());
        renditions.insert(Tier::Workstation, "media/hap/tunnel.mov".into());

        show.clips.push(ClipManifestEntry {
            id: ClipId::new("tunnel_loop"),
            title: "Neon Tunnel Loop".into(),
            duration_ms: 8000,
            thumb_path: "thumbs/tunnel_loop.jpg".into(),
            renditions,
        });

        let toml_str = show.to_toml().expect("failed to serialize show");
        let parsed = ShowManifest::from_toml(&toml_str).expect("failed to deserialize show");

        assert_eq!(parsed.name, "Warehouse Rave 2026");
        assert_eq!(parsed.banks[0].grid[0][0], show.banks[0].grid[0][0]);
        assert_eq!(parsed.clips.len(), 1);
        assert!(parsed.validate_for_tier(Tier::Appliance).is_ok());
    }

    #[test]
    fn show_fails_validation_if_tier_rendition_missing() {
        let mut show = ShowManifest::new("Desktop Only Show");
        let mut renditions = HashMap::new();
        renditions.insert(Tier::Workstation, "media/hap/heavy.mov".into());

        show.clips.push(ClipManifestEntry {
            id: ClipId::new("heavy"),
            title: "Heavy Clip".into(),
            duration_ms: 5000,
            thumb_path: "thumbs/heavy.jpg".into(),
            renditions,
        });

        assert!(matches!(
            show.validate_for_tier(Tier::Appliance),
            Err(ShowError::MissingRendition(..))
        ));
    }
}
