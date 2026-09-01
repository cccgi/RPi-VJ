//! Shared vocabulary for the VJ engine.
//!
//! This crate is a leaf: no I/O, no platform code, no GPU. Everything here is
//! `Clone + Serialize + Deserialize` so it can cross the admin API, the show
//! file, and the test harness unchanged.

use serde::{Deserialize, Serialize};
use std::fmt;

pub mod show;
pub use show::*;

// ---------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------

macro_rules! string_id {
    ($($(#[$m:meta])* $name:ident),* $(,)?) => {$(
        $(#[$m])*
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self { Self(s.into()) }
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0)
            }
        }
        impl From<&str> for $name {
            fn from(s: &str) -> Self { Self(s.to_owned()) }
        }
    )*};
}

string_id! {
    /// A clip in the media index.
    ClipId,
    /// A compiled shader asset.
    ShaderId,
    /// A connected control surface instance.
    SurfaceId,
    /// A `SurfaceModel` (device type), e.g. `"akai.fire"`.
    SurfaceModelId,
    /// A named region of the virtual control surface, e.g. `"clip.grid"`.
    RegionId,
    /// A physical control on a device, e.g. `"pad.3.11"`.
    ControlId,
    /// Correlates a command with its lifecycle events.
    RequestId,
}

/// A video deck within the deck pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeckId(pub u16);

/// A compositor layer, ordered back-to-front by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerId(pub u16);

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// A dotted path addressing anything modulatable, e.g. `layer.2.opacity`
/// or `fx.a.param.3`. Parsed once; compared as a whole.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ParameterPath(pub String);

impl ParameterPath {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('.')
    }
}

impl fmt::Debug for ParameterPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ParameterPath({})", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ParameterValue {
    Bool(bool),
    Float(f32),
    Int(i64),
    /// Linear RGBA, 0..1.
    Color([f32; 4]),
    Point2D([f32; 2]),
    /// A momentary event; carries no payload.
    Event,
    Enum(String),
}

// ---------------------------------------------------------------------------
// Compositing
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum BlendMode {
    #[default]
    Alpha,
    Additive,
    Multiply,
    Screen,
    LumaKey { threshold: f32, softness: f32 },
    ChromaKey { hue: f32, tolerance: f32, softness: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransitionKind {
    Cut,
    Crossfade,
    Dip { color: [f32; 4] },
    Wipe { angle: f32, softness: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TransitionSpec {
    pub kind: TransitionKind,
    pub duration_ms: u32,
}

impl Default for TransitionSpec {
    fn default() -> Self {
        Self { kind: TransitionKind::Cut, duration_ms: 0 }
    }
}

// ---------------------------------------------------------------------------
// Tempo
// ---------------------------------------------------------------------------

/// When a queued action should take effect.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "quantize", rename_all = "snake_case")]
pub enum Quantize {
    /// Next vblank.
    #[default]
    Off,
    Beat,
    HalfBar,
    Bar,
    /// Next `bars`-bar boundary.
    Phrase { bars: u8 },
}

impl Quantize {
    /// Length of the quantization grid in beats, given the meter.
    /// `Off` has no grid.
    pub fn beats(self, beats_per_bar: u8) -> Option<f32> {
        let bpb = beats_per_bar.max(1) as f32;
        match self {
            Quantize::Off => None,
            Quantize::Beat => Some(1.0),
            Quantize::HalfBar => Some(bpb / 2.0),
            Quantize::Bar => Some(bpb),
            Quantize::Phrase { bars } => Some(bpb * bars.max(1) as f32),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TempoSource {
    Manual,
    Link,
    MidiClock,
    Audio,
    ProDjLink,
}

// ---------------------------------------------------------------------------
// Capability tiers
// ---------------------------------------------------------------------------

/// Hardware class a show is running on. Ordered: `Appliance < Portable < Workstation`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Appliance,
    Portable,
    Workstation,
}

/// What a tier is willing to run. Populated at startup from the detected
/// platform, then reconciled against a show's requirements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TierBudget {
    pub tier: Tier,
    pub visible_decks: u16,
    pub hot_decoder_slots: u16,
    pub shader_layers: u16,
    pub max_isf_passes: u8,
    pub render_scale: f32,
}

impl TierBudget {
    pub fn appliance() -> Self {
        Self {
            tier: Tier::Appliance,
            visible_decks: 2,
            hot_decoder_slots: 4,
            shader_layers: 2,
            max_isf_passes: 4,
            render_scale: 1.0,
        }
    }
    pub fn portable() -> Self {
        Self {
            tier: Tier::Portable,
            visible_decks: 4,
            hot_decoder_slots: 12,
            shader_layers: 6,
            max_isf_passes: 8,
            render_scale: 1.0,
        }
    }
    pub fn workstation() -> Self {
        Self {
            tier: Tier::Workstation,
            visible_decks: 8,
            hot_decoder_slots: 32,
            shader_layers: 8,
            max_isf_passes: 8,
            render_scale: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnknownClip,
    UnknownShader,
    UnknownLayer,
    UnknownDeck,
    TierExceeded,
    AssetNotReady,
    DeviceUnavailable,
    InvalidParameter,
    QueueSaturated,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_grid_lengths() {
        assert_eq!(Quantize::Off.beats(4), None);
        assert_eq!(Quantize::Beat.beats(4), Some(1.0));
        assert_eq!(Quantize::HalfBar.beats(4), Some(2.0));
        assert_eq!(Quantize::Bar.beats(4), Some(4.0));
        assert_eq!(Quantize::Phrase { bars: 8 }.beats(4), Some(32.0));
        // Meter is respected, not assumed to be 4/4.
        assert_eq!(Quantize::Bar.beats(3), Some(3.0));
    }

    #[test]
    fn tiers_are_ordered() {
        assert!(Tier::Appliance < Tier::Portable);
        assert!(Tier::Portable < Tier::Workstation);
    }

    #[test]
    fn parameter_path_segments() {
        let p = ParameterPath::new("layer.2.opacity");
        assert_eq!(p.segments().collect::<Vec<_>>(), vec!["layer", "2", "opacity"]);
    }
}
