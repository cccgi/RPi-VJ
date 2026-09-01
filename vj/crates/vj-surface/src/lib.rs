//! Control surface abstraction.
//!
//! Three layers, specified in `docs/v2/03-CONTROL-SURFACES.md`:
//!
//! 1. transport   — `vj-surface-midi`, `vj-surface-hid`
//! 2. device profile — [`SurfaceModel`], [`SurfaceDriver`] (implemented in `vj-devices`)
//! 3. semantic mapping — [`VirtualSurface`], [`Binding`] (this module)
//!
//! Rule: nothing in this crate may name a specific product.

use serde::{Deserialize, Serialize};
use vj_types::{ControlId, ParameterValue, RegionId, SurfaceId, SurfaceModelId};

pub mod takeover;

// ---------------------------------------------------------------------------
// Physical topology — what a device *is*
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridPos {
    pub row: u16,
    pub col: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CcMode {
    /// 0..127 absolute. Requires soft takeover.
    Absolute,
    /// 1..63 up, 127..65 down.
    RelativeTwosComplement,
    /// 1..63 up, 65..127 down.
    RelativeSignedBit,
    /// 64 is zero; offset either side.
    RelativeBinaryOffset,
}

impl CcMode {
    pub fn is_relative(self) -> bool {
        !matches!(self, CcMode::Absolute)
    }

    /// Decode a 7-bit CC value into a delta for relative modes.
    /// Returns `None` for [`CcMode::Absolute`].
    pub fn delta(self, raw: u8) -> Option<i8> {
        let raw = raw & 0x7f;
        match self {
            CcMode::Absolute => None,
            CcMode::RelativeTwosComplement => {
                Some(if raw < 64 { raw as i8 } else { (raw as i16 - 128) as i8 })
            }
            CcMode::RelativeSignedBit => Some(if raw < 64 { raw as i8 } else { -((raw - 64) as i8) }),
            CcMode::RelativeBinaryOffset => Some((raw as i16 - 64) as i8),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ControlKind {
    /// Momentary, grid member.
    Pad,
    /// Momentary, non-grid: transport, scene, function.
    Button,
    /// Piano key. Velocity-sensitive; not a launch trigger by default.
    Key { note: u8 },
    Encoder { detents: Option<u16> },
    Fader,
    Joystick { axes: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "via", rename_all = "snake_case")]
pub enum InputSpec {
    Note { channel: u8, number: u8 },
    Cc { channel: u8, number: u8, mode: CcMode },
    HidKey { index: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RingMode {
    Single,
    Pan,
    Fan,
    Spread,
    Trim,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFormat {
    Jpeg,
    Bmp,
    Raw,
}

/// How (and whether) a control can show state back to the operator.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "feedback", rename_all = "snake_case")]
pub enum FeedbackCapability {
    None,
    /// On / off only.
    Monochrome,
    /// Velocity-indexed colour table.
    Palette { size: u16 },
    /// Arbitrary RGB, typically via SysEx.
    Rgb,
    /// A per-key bitmap, e.g. a Stream Deck key.
    Bitmap { w: u16, h: u16, format: ImageFormat },
    /// Segmented LED ring around an encoder.
    EncoderRing { segments: u8, modes: Vec<RingMode> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlDescriptor {
    pub id: ControlId,
    pub kind: ControlKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<GridPos>,
    pub input: InputSpec,
    #[serde(default = "FeedbackCapability::none")]
    pub feedback: FeedbackCapability,
}

impl FeedbackCapability {
    fn none() -> Self {
        FeedbackCapability::None
    }
    pub fn has_color(&self) -> bool {
        matches!(self, FeedbackCapability::Palette { .. } | FeedbackCapability::Rgb)
    }
}

/// A text or graphic display that is not attached to a single control,
/// e.g. the Akai Fire's OLED strip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DisplayDescriptor {
    pub id: ControlId,
    pub cols: u8,
    pub rows: u8,
    #[serde(default)]
    pub graphic: Option<(u16, u16)>,
}

/// An action the driver must perform on connect and on every reconnect —
/// entering a device's programmer/standard mode, for instance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum InitAction {
    SysEx { bytes: Vec<u8> },
    Cc { channel: u8, number: u8, value: u8 },
    ClearAll,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum TransportSpec {
    Midi {
        /// Substring matched against enumerated port names.
        in_port: String,
        out_port: String,
    },
    Hid {
        vendor_id: u16,
        product_ids: Vec<u16>,
    },
}

/// The complete physical description of one device type.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfaceModel {
    pub id: SurfaceModelId,
    pub display_name: String,
    pub transport: TransportSpec,
    #[serde(default)]
    pub init: Vec<InitAction>,
    pub controls: Vec<ControlDescriptor>,
    #[serde(default)]
    pub displays: Vec<DisplayDescriptor>,
    /// Set when LED/display output needs a Rust driver rather than the
    /// generic profile engine.
    #[serde(default)]
    pub driver: Option<String>,
    /// Cap on outbound feedback messages per second.
    #[serde(default = "default_feedback_hz")]
    pub feedback_hz: u16,
}

fn default_feedback_hz() -> u16 {
    30
}

impl SurfaceModel {
    pub fn control(&self, id: &ControlId) -> Option<&ControlDescriptor> {
        self.controls.iter().find(|c| &c.id == id)
    }

    /// Bounding size of the device's pad grid, if it has one.
    pub fn grid_extent(&self) -> Option<(u16, u16)> {
        let mut rows = 0;
        let mut cols = 0;
        let mut any = false;
        for c in &self.controls {
            if let (ControlKind::Pad, Some(p)) = (c.kind, c.position) {
                any = true;
                rows = rows.max(p.row + 1);
                cols = cols.max(p.col + 1);
            }
        }
        any.then_some((rows, cols))
    }
}

// ---------------------------------------------------------------------------
// Events in and feedback out
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ControlEvent {
    Press { control: ControlId, velocity: u8, at_ns: u64 },
    Release { control: ControlId, at_ns: u64 },
    /// Absolute continuous control, normalised 0..1.
    Absolute { control: ControlId, value: f32, at_ns: u64 },
    /// Relative encoder movement, in detents.
    Relative { control: ControlId, delta: i16, at_ns: u64 },
    Connected { surface: SurfaceId },
    Disconnected { surface: SurfaceId },
}

/// The colour a control should show. Drivers map this onto whatever their
/// hardware can actually do.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FeedbackColor {
    pub rgb: [u8; 3],
    /// Hardware pulse rate in beats, when the device supports it.
    /// `None` means solid.
    pub pulse_beats: Option<f32>,
}

impl FeedbackColor {
    pub const OFF: Self = Self { rgb: [0, 0, 0], pulse_beats: None };
    pub fn solid(rgb: [u8; 3]) -> Self {
        Self { rgb, pulse_beats: None }
    }
    pub fn pulsing(rgb: [u8; 3], beats: f32) -> Self {
        Self { rgb, pulse_beats: Some(beats) }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "cell", rename_all = "snake_case")]
pub enum FeedbackCell {
    Color(FeedbackColor),
    /// Index into a device-ready image cache built by the media indexer.
    /// The feedback worker never encodes images itself.
    Image { cache_key: u64 },
    Ring { value: f32, mode: RingMode },
    Text { line: u8, text: String },
    Off,
}

/// Priority band. When a device's output budget saturates, low bands are
/// dropped first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackPriority {
    /// Press acknowledgement, blackout, error.
    Critical = 0,
    /// Active clip / shader selection.
    Selection = 1,
    /// Parameter values.
    Parameter = 2,
    /// Thumbnails and loaded state.
    Content = 3,
    /// Meters, beat animation.
    Decorative = 4,
}

/// The complete desired state of one device. Drivers diff this against what
/// they last sent and emit only the difference.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FeedbackFrame {
    pub revision: u64,
    pub cells: Vec<(ControlId, FeedbackCell, FeedbackPriority)>,
}

// ---------------------------------------------------------------------------
// Driver and adapter
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("device not connected")]
    NotConnected,
    #[error("transport error: {0}")]
    Transport(String),
    #[error("unsupported feedback for control {0}")]
    UnsupportedFeedback(ControlId),
    #[error("profile error: {0}")]
    Profile(String),
}

pub trait MidiSink {
    fn send(&mut self, bytes: &[u8]) -> Result<(), SurfaceError>;
}

pub struct RawInput<'a> {
    pub bytes: &'a [u8],
    pub at_ns: u64,
}

/// Implemented in `vj-devices` for products whose LED/display protocol needs
/// code rather than a declarative profile.
pub trait SurfaceDriver: Send {
    fn model(&self) -> &SurfaceModel;
    fn initialise(&mut self, out: &mut dyn MidiSink) -> Result<(), SurfaceError>;
    fn decode(&mut self, raw: RawInput<'_>, out: &mut Vec<ControlEvent>) -> Result<(), SurfaceError>;
    /// Called at most `model().feedback_hz` times per second, off the input thread.
    fn encode_feedback(
        &mut self,
        frame: &FeedbackFrame,
        out: &mut dyn MidiSink,
    ) -> Result<(), SurfaceError>;
    fn shutdown(&mut self, out: &mut dyn MidiSink);
}

/// What the engine sees. One per connected device.
pub trait ControllerAdapter: Send {
    fn surface_id(&self) -> SurfaceId;
    fn poll(&mut self, out: &mut Vec<ControlEvent>) -> Result<(), SurfaceError>;
    fn apply_feedback(&mut self, frame: &FeedbackFrame) -> Result<(), SurfaceError>;
    fn connected(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Semantic mapping
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingMode {
    /// A 2-D window onto a 2-D engine region.
    Grid,
    /// One control per indexed instance of a region, e.g. `layer.{n}.opacity`.
    FanOut,
    /// A single control to a single target.
    Single,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PressBehaviour {
    Momentary,
    Toggle,
    /// Fires on release only if held under `hold_ms`.
    Tap,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Binding {
    pub region: RegionId,
    /// Glob over control ids, e.g. `"pad.*"`, `"enc.*"`, `"func.top.7"`.
    pub controls: String,
    pub mode: BindingMode,
    #[serde(default)]
    pub origin: Option<GridPos>,
    #[serde(default = "default_takeover")]
    pub takeover: takeover::TakeoverMode,
    #[serde(default)]
    pub behaviour: Option<PressBehaviour>,
    #[serde(default)]
    pub ring: Option<RingMode>,
    /// Device-side layer/bank this binding belongs to, e.g. X-Touch "A"/"B".
    #[serde(default)]
    pub layer: Option<String>,
}

fn default_takeover() -> takeover::TakeoverMode {
    takeover::TakeoverMode::Scale
}

/// One device's live mapping state.
#[derive(Clone, Debug)]
pub struct BoundSurface {
    pub surface: SurfaceId,
    pub model: SurfaceModelId,
    pub bindings: Vec<Binding>,
    /// Scroll offset per region, for `Grid` bindings.
    pub windows: Vec<(RegionId, GridPos)>,
    pub takeover: takeover::TakeoverState,
}

/// All bound devices plus the region vocabulary the engine exposes.
#[derive(Debug, Default)]
pub struct VirtualSurface {
    pub surfaces: Vec<BoundSurface>,
}

impl VirtualSurface {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&mut self, bound: BoundSurface) {
        self.surfaces.retain(|s| s.surface != bound.surface);
        self.surfaces.push(bound);
    }

    pub fn unbind(&mut self, surface: &SurfaceId) {
        self.surfaces.retain(|s| &s.surface != surface);
    }

    pub fn window(&self, surface: &SurfaceId, region: &RegionId) -> Option<GridPos> {
        self.surfaces
            .iter()
            .find(|s| &s.surface == surface)?
            .windows
            .iter()
            .find(|(r, _)| r == region)
            .map(|(_, p)| *p)
    }

    /// Project a device-local grid position into engine-region coordinates
    /// using the surface's current scroll window.
    pub fn project(&self, surface: &SurfaceId, region: &RegionId, local: GridPos) -> Option<GridPos> {
        let origin = self.window(surface, region).unwrap_or(GridPos { row: 0, col: 0 });
        Some(GridPos { row: origin.row + local.row, col: origin.col + local.col })
    }
}

/// A captured mapping, produced by MIDI learn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LearnedMapping {
    pub region: RegionId,
    pub control: ControlId,
    pub input: InputSpec,
    /// Observed extremes, used to infer whether a CC is absolute or relative.
    pub observed_min: u8,
    pub observed_max: u8,
    pub inferred: CcMode,
}

/// Infer a CC mode from a sweep. Relative encoders cluster near the ends or
/// around 64; an absolute control sweeps the full range monotonically.
pub fn infer_cc_mode(samples: &[u8]) -> CcMode {
    if samples.len() < 4 {
        return CcMode::Absolute;
    }
    let low = samples.iter().filter(|&&v| (1..=3).contains(&v)).count();
    let high = samples.iter().filter(|&&v| (125..=127).contains(&v)).count();
    let near64 = samples.iter().filter(|&&v| (61..=67).contains(&v)).count();
    let n = samples.len();

    if near64 * 2 > n {
        CcMode::RelativeBinaryOffset
    } else if (low + high) * 2 > n {
        CcMode::RelativeTwosComplement
    } else {
        CcMode::Absolute
    }
}

/// Normalise a raw 7-bit value to 0..1.
pub fn normalise7(raw: u8) -> f32 {
    (raw & 0x7f) as f32 / 127.0
}

/// Denormalise 0..1 into a 7-bit value.
pub fn denormalise7(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 127.0).round() as u8
}

/// Convert a normalised parameter into a `ParameterValue`.
pub fn as_float(v: f32) -> ParameterValue {
    ParameterValue::Float(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_two_complement_decode() {
        assert_eq!(CcMode::RelativeTwosComplement.delta(1), Some(1));
        assert_eq!(CcMode::RelativeTwosComplement.delta(3), Some(3));
        assert_eq!(CcMode::RelativeTwosComplement.delta(127), Some(-1));
        assert_eq!(CcMode::RelativeTwosComplement.delta(125), Some(-3));
        assert_eq!(CcMode::Absolute.delta(64), None);
    }

    #[test]
    fn binary_offset_decode() {
        assert_eq!(CcMode::RelativeBinaryOffset.delta(64), Some(0));
        assert_eq!(CcMode::RelativeBinaryOffset.delta(65), Some(1));
        assert_eq!(CcMode::RelativeBinaryOffset.delta(63), Some(-1));
    }

    #[test]
    fn infers_absolute_from_a_full_sweep() {
        let sweep: Vec<u8> = (0..=127).step_by(4).collect();
        assert_eq!(infer_cc_mode(&sweep), CcMode::Absolute);
    }

    #[test]
    fn infers_relative_from_endpoint_clusters() {
        let ticks = vec![1, 1, 2, 1, 127, 127, 126, 1, 1, 127];
        assert_eq!(infer_cc_mode(&ticks), CcMode::RelativeTwosComplement);
    }

    #[test]
    fn grid_extent_is_derived_from_pad_positions() {
        let model = SurfaceModel {
            id: "test.grid".into(),
            display_name: "Test".into(),
            transport: TransportSpec::Midi { in_port: "x".into(), out_port: "x".into() },
            init: vec![],
            controls: (0..4)
                .flat_map(|r| {
                    (0..16).map(move |c| ControlDescriptor {
                        id: ControlId::new(format!("pad.{r}.{c}")),
                        kind: ControlKind::Pad,
                        position: Some(GridPos { row: r, col: c }),
                        input: InputSpec::Note { channel: 0, number: (r * 16 + c) as u8 },
                        feedback: FeedbackCapability::Rgb,
                    })
                })
                .collect(),
            displays: vec![],
            driver: None,
            feedback_hz: 30,
        };
        assert_eq!(model.grid_extent(), Some((4, 16)));
    }

    #[test]
    fn window_projection_offsets_grid_coordinates() {
        let mut vs = VirtualSurface::new();
        vs.bind(BoundSurface {
            surface: SurfaceId::new("lp-1"),
            model: SurfaceModelId::new("novation.launchpad-mini-mk3"),
            bindings: vec![],
            windows: vec![(RegionId::new("clip.grid"), GridPos { row: 0, col: 8 })],
            takeover: takeover::TakeoverState::default(),
        });
        let projected = vs
            .project(
                &SurfaceId::new("lp-1"),
                &RegionId::new("clip.grid"),
                GridPos { row: 2, col: 3 },
            )
            .unwrap();
        assert_eq!(projected, GridPos { row: 2, col: 11 });
    }
}
