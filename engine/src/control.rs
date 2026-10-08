use crate::show::Show;
use serde::{Deserialize, Serialize};
pub const CLIPS_PER_PAGE: usize = 28;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlendMode {
    Alpha,
    Additive,
    Screen,
    Multiply,
    Overlay,
    SoftLight,
    LumaKey,
    Difference,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layer {
    Video,
    Shader,
}
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum Command {
    TriggerClip { clip: usize },
    TriggerShader { shader: usize },
    Page { delta: i32 },
    SelectShader { enabled: bool },
    SetBlend { mode: BlendMode, opacity: f32 },
    Blackout { enabled: bool },
    ClearClips,
    ClearShader,
    Reset,
    SetOpacity { opacity: f32 },
    SetVideoEffect { effect: u8 },
    SetShaderEffect { effect: u8 },
    SetEffectAmount { layer: Layer, amount: f32 },
    AdjustSpeed { delta: f64 },
    AdjustBpm { delta: f32 },
    TapTempo { time_ms: u64 },
    ToggleHud,
    SetVideoOpacity { opacity: f32 },
    /// Linear gain on the audio input before analysis; 0 mutes the reaction.
    SetInputGain { gain: f32 },
    SetSpeed { speed: f64 },
    ToggleLayerOrder,
    /// Change shader by itself whenever the music moves to a new section.
    ToggleAutoShuffle,
    /// One of the eight performance macros (X-Touch layer B knobs), 0..=1.
    SetMacro { index: u8, value: f32 },
    CrateShader { crate_id: u8 },
    DeleteShader,
}
/// Authoritative control state is independent of the render process and HID feedback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    clips: usize,
    shaders: usize,
    pages: usize,
    default_bpm: f32,
    last_tap: Option<u64>,
    #[serde(default)]
    shader_presets: Vec<(BlendMode, f32)>,
    pub page: usize,
    pub requested: Option<usize>,
    pub playing: Option<usize>,
    pub failed: Option<usize>,
    pub shader: bool,
    pub selected_shader: Option<usize>,
    pub blackout: bool,
    pub blend: BlendMode,
    pub opacity: f32,
    pub generation: u64,
    pub video_effect: u8,
    pub shader_effect: u8,
    pub video_effect_amount: f32,
    pub shader_effect_amount: f32,
    pub speed: f64,
    pub bpm: f32,
    pub hud: bool,
    #[serde(default = "one")]
    pub video_opacity: f32,
    #[serde(default = "one")]
    pub input_gain: f32,
    /// Clip drawn over the shader instead of under it.
    #[serde(default)]
    pub invert_order: bool,
    /// Shader-layer geometry: bass bulge, low-mid twist, mid wave, treble ripple, scroll
    /// up/down, scroll left/right, drift (these three: 0.5 is still) and mirror segments.
    #[serde(default = "default_macros")]
    pub macros: [f32; 8],
    #[serde(default)]
    pub auto_shuffle: bool,
    #[serde(default)]
    pub failed_shaders: Vec<usize>,
    #[serde(default)]
    pub shader_crates: Vec<Option<u8>>,
    #[serde(default)]
    pub deleted_shaders: Vec<bool>,
}
fn one() -> f32 {
    1.0
}
/// Deals every shader once, in random order, before any comes round again.
#[derive(Debug, Clone)]
pub struct ShuffleBag {
    left: Vec<usize>,
    seed: u64,
}
impl ShuffleBag {
    pub fn new(seed: u64) -> Self {
        Self { left: Vec::new(), seed: seed | 1 }
    }
    /// The next shader to show: never the live one, nor one that failed or was deleted.
    pub fn next(&mut self, state: &State) -> Option<usize> {
        for _ in 0..2 {
            while !self.left.is_empty() {
                self.seed ^= self.seed << 13;
                self.seed ^= self.seed >> 7;
                self.seed ^= self.seed << 17;
                let shader = self.left.swap_remove((self.seed % self.left.len() as u64) as usize);
                if shader < state.shaders
                    && Some(shader) != state.selected_shader
                    && !state.failed_shaders.contains(&shader)
                    && !state.deleted_shaders.get(shader).copied().unwrap_or(false)
                {
                    return Some(shader);
                }
            }
            self.left = (0..state.shaders).collect();
        }
        None
    }
}
pub const MAX_INPUT_GAIN: f32 = 4.0;
pub const MACRO_DEFAULTS: [f32; 8] = [0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 0.0];
fn default_macros() -> [f32; 8] {
    MACRO_DEFAULTS
}
impl State {
    pub fn new(clips: usize) -> Self {
        Self {
            clips,
            shaders: 0,
            shader_presets: Vec::new(),
            pages: clips.div_ceil(CLIPS_PER_PAGE).max(1),
            default_bpm: 128.0,
            last_tap: None,
            page: 0,
            requested: None,
            playing: None,
            failed: None,
            shader: false,
            selected_shader: None,
            blackout: false,
            blend: BlendMode::Alpha,
            opacity: 0.5,
            generation: 0,
            video_effect: 0,
            shader_effect: 0,
            video_effect_amount: 1.0,
            shader_effect_amount: 1.0,
            speed: 1.0,
            bpm: 128.0,
            hud: false,
            video_opacity: 1.0,
            input_gain: 1.0,
            invert_order: false,
            macros: MACRO_DEFAULTS,
            auto_shuffle: false,
            failed_shaders: Vec::new(),
            shader_crates: Vec::new(),
            deleted_shaders: Vec::new(),
        }
    }
    pub fn from_show(show: &Show) -> Self {
        let mut s = Self::new(show.clips.len());
        s.shaders = show.shaders.len();
        s.shader_presets = show
            .shaders
            .iter()
            .map(|shader| (shader.blend, shader.opacity))
            .collect();
        s.shader_crates = vec![None; show.shaders.len()];
        s.deleted_shaders = vec![false; show.shaders.len()];
        if !show.pages.is_empty() {
            s.pages = show.pages.len();
        }
        s.default_bpm = show.bpm;
        s.bpm = show.bpm;
        s
    }
    pub fn page_count(&self) -> usize {
        self.pages
    }
    pub fn validate_for(&self, show: &Show) -> bool {
        let expected = Self::from_show(show);
        self.clips == expected.clips
            && self.shaders == expected.shaders
            && self.pages == expected.pages
            && self.default_bpm == expected.default_bpm
            && self.shader_presets == expected.shader_presets
            && self.page < self.pages
            && self.requested.is_none_or(|i| i < self.clips)
            && self.playing.is_none_or(|i| i < self.clips)
            && self.failed.is_none_or(|i| i < self.clips)
            && self.selected_shader.is_none_or(|i| i < self.shaders)
            && self.video_effect <= 24
            && self.shader_effect <= 24
            && [
                self.opacity,
                self.video_opacity,
                self.video_effect_amount,
                self.shader_effect_amount,
            ]
            .iter()
            .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && self.macros.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            && self.input_gain.is_finite()
            && (0.0..=MAX_INPUT_GAIN).contains(&self.input_gain)
            && self.speed.is_finite()
            && (0.25..=4.0).contains(&self.speed)
            && self.bpm.is_finite()
            && (30.0..=300.0).contains(&self.bpm)
            && self.failed_shaders.iter().all(|i| *i < self.shaders)
            && self.shader_crates.len() == self.shaders
            && self.shader_crates.iter().flatten().all(|c| *c < 4)
            && self.deleted_shaders.len() == self.shaders
    }
    /// Recover desired state, but never retain an unverified presentation claim or old tap clock.
    pub fn recover(&mut self) {
        if self.requested.is_none() {
            self.requested = self.playing;
        }
        self.playing = None;
        self.last_tap = None;
        self.failed = None;
        self.failed_shaders.clear();
        self.generation = self.generation.wrapping_add(1);
    }
    pub fn apply(&mut self, command: Command) -> Result<(), &'static str> {
        fn unit(v: f32) -> Result<(), &'static str> {
            if v.is_finite() && (0.0..=1.0).contains(&v) {
                Ok(())
            } else {
                Err("amount must be finite in 0..=1")
            }
        }
        match command {
            Command::TriggerClip { clip } => {
                if clip >= self.clips {
                    return Err("unknown clip");
                }
                self.requested = Some(clip);
                self.failed = None;
                self.generation = self.generation.wrapping_add(1);
            }
            Command::TriggerShader { shader } => {
                if shader >= self.shaders {
                    return Err("unknown shader");
                }
                self.selected_shader = Some(shader);
                self.shader = true;
                if let Some((blend, opacity)) = self.shader_presets.get(shader) {
                    self.blend = *blend;
                    self.opacity = *opacity;
                }
            }
            Command::Page { delta } => {
                self.page =
                    (self.page as i64 + i64::from(delta)).rem_euclid(self.pages as i64) as usize
            }
            Command::SelectShader { enabled } => self.shader = enabled,
            Command::Blackout { enabled } => self.blackout = enabled,
            Command::SetBlend { mode, opacity } => {
                unit(opacity)?;
                self.blend = mode;
                self.opacity = opacity;
            }
            Command::SetOpacity { opacity } => {
                unit(opacity)?;
                self.opacity = opacity;
            }
            Command::SetVideoEffect { effect } | Command::SetShaderEffect { effect } => {
                if effect > 24 {
                    return Err("unknown effect");
                }
                if matches!(command, Command::SetVideoEffect { .. }) {
                    self.video_effect = effect;
                } else {
                    self.shader_effect = effect;
                }
            }
            Command::SetEffectAmount { layer, amount } => {
                unit(amount)?;
                match layer {
                    Layer::Video => self.video_effect_amount = amount,
                    Layer::Shader => self.shader_effect_amount = amount,
                }
            }
            Command::AdjustSpeed { delta } => {
                if !delta.is_finite() {
                    return Err("nonfinite speed");
                }
                self.speed = (self.speed * (1.0 + delta)).clamp(0.25, 4.0);
            }
            Command::AdjustBpm { delta } => {
                if !delta.is_finite() {
                    return Err("nonfinite tempo");
                }
                self.bpm = (self.bpm + delta).clamp(30.0, 300.0);
            }
            Command::TapTempo { time_ms } => {
                if let Some(last) = self.last_tap {
                    if time_ms <= last {
                        return Err("non-monotonic tap");
                    }
                    let interval = time_ms - last;
                    if (200..=2000).contains(&interval) {
                        self.bpm = 60000.0 / interval as f32;
                    }
                }
                self.last_tap = Some(time_ms);
            }
            Command::ToggleHud => self.hud = !self.hud,
            Command::SetVideoOpacity { opacity } => {
                unit(opacity)?;
                self.video_opacity = opacity;
            }
            Command::SetInputGain { gain } => {
                if !(gain.is_finite() && (0.0..=MAX_INPUT_GAIN).contains(&gain)) {
                    return Err("input gain out of range");
                }
                self.input_gain = gain;
            }
            Command::SetSpeed { speed } => {
                if !speed.is_finite() {
                    return Err("nonfinite speed");
                }
                self.speed = speed.clamp(0.25, 4.0);
            }
            Command::ToggleLayerOrder => self.invert_order = !self.invert_order,
            Command::ToggleAutoShuffle => self.auto_shuffle = !self.auto_shuffle,
            Command::SetMacro { index, value } => {
                unit(value)?;
                *self.macros.get_mut(usize::from(index)).ok_or("unknown macro")? = value;
            }
            Command::CrateShader { crate_id } => {
                if crate_id >= 4 { return Err("unknown shader crate"); }
                if let Some(shader) = self.selected_shader {
                    if let Some(slot) = self.shader_crates.get_mut(shader) { *slot = Some(crate_id); }
                }
            }
            Command::DeleteShader => {
                if let Some(shader) = self.selected_shader {
                    if let Some(slot) = self.deleted_shaders.get_mut(shader) { *slot = true; }
                }
            }
            Command::ClearClips => {
                self.requested = None;
                self.playing = None;
                self.failed = None;
                self.generation = self.generation.wrapping_add(1);
            }
            Command::ClearShader => {
                self.shader = false;
                self.selected_shader = None;
            }
            Command::Reset => {
                let mut s = Self::new(self.clips);
                s.shaders = self.shaders;
                s.pages = self.pages;
                s.page = self.page;
                s.shader_presets = self.shader_presets.clone();
                s.failed_shaders = self.failed_shaders.clone();
                s.shader_crates = self.shader_crates.clone();
                s.deleted_shaders = self.deleted_shaders.clone();
                s.default_bpm = self.default_bpm;
                s.bpm = self.default_bpm;
                // The input trim belongs to the venue, not to the look being reset.
                s.input_gain = self.input_gain;
                s.generation = self.generation.wrapping_add(1);
                *self = s;
            }
        }
        Ok(())
    }
    pub fn presented(&mut self, clip: usize, generation: u64) -> bool {
        if self.requested == Some(clip) && self.generation == generation {
            self.playing = Some(clip);
            true
        } else {
            false
        }
    }
    pub fn rejected(&mut self, generation: u64) -> bool {
        if generation == self.generation {
            self.failed = self.requested;
            self.requested = None;
            true
        } else {
            false
        }
    }
    pub fn key_in_show(&self, show: &Show, key: u8) -> Option<Command> {
        crate::surface::page(show, self)
            .get(usize::from(key))
            .and_then(|k| k.command)
    }
    pub fn key(&self, key: u8) -> Option<Command> {
        match key {
            24 => Some(Command::Page { delta: -1 }),
            25 => Some(Command::Page { delta: 1 }),
            30 => Some(Command::Reset),
            31 => Some(Command::Blackout {
                enabled: !self.blackout,
            }),
            0..=23 | 26..=29 => {
                let slot = if key < 24 { key } else { key - 2 };
                let clip = self.page * CLIPS_PER_PAGE + usize::from(slot);
                (clip < self.clips).then_some(Command::TriggerClip { clip })
            }
            _ => None,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_shuffle_bag_deals_every_usable_shader_before_repeating() {
        let mut state = State::new(0);
        state.shaders = 6;
        state.deleted_shaders = vec![false, false, false, false, false, true];
        state.failed_shaders = vec![4];
        let mut bag = ShuffleBag::new(0);
        let mut round = Vec::new();
        for _ in 0..4 {
            let shader = bag.next(&state).unwrap();
            state.apply(Command::TriggerShader { shader }).unwrap();
            round.push(shader);
        }
        round.sort_unstable();
        assert_eq!(round, [0, 1, 2, 3]);
        // The next round never repeats the shader that is showing.
        for _ in 0..20 {
            let shader = bag.next(&state).unwrap();
            assert!(shader < 4 && Some(shader) != state.selected_shader);
            state.apply(Command::TriggerShader { shader }).unwrap();
        }
        state.shaders = 1;
        state.selected_shader = Some(0);
        assert_eq!(ShuffleBag::new(1).next(&state), None);
    }
    #[test]
    fn wraps_without_changing_playback() {
        let mut s = State::new(60);
        s.apply(Command::TriggerClip { clip: 1 }).unwrap();
        s.presented(1, 1);
        s.apply(Command::Page { delta: -1 }).unwrap();
        assert_eq!(s.page, 2);
        s.apply(Command::Page { delta: 1 }).unwrap();
        assert_eq!(s.page, 0);
        assert_eq!(s.playing, Some(1));
    }
    #[test]
    fn stale_and_failed_candidates_preserve_output() {
        let mut s = State::new(3);
        s.apply(Command::TriggerClip { clip: 0 }).unwrap();
        s.presented(0, 1);
        s.apply(Command::TriggerClip { clip: 1 }).unwrap();
        assert!(!s.presented(0, 1));
        assert!(s.rejected(2));
        assert_eq!(s.playing, Some(0));
        s.apply(Command::TriggerClip { clip: 2 }).unwrap();
        assert!(!s.rejected(2));
    }
    #[test]
    fn invalid_is_atomic() {
        let mut s = State::new(2);
        assert!(s
            .apply(Command::SetBlend {
                mode: BlendMode::Additive,
                opacity: f32::NAN
            })
            .is_err());
        assert!(s.apply(Command::SetVideoEffect { effect: 25 }).is_err());
        assert_eq!(s.blend, BlendMode::Alpha);
        assert_eq!(s.video_effect, 0);
    }
    #[test]
    fn selections_exclusive_and_reset_invalidates_decoder() {
        let mut s = State::new(3);
        s.shaders = 2;
        s.apply(Command::TriggerShader { shader: 0 }).unwrap();
        s.apply(Command::TriggerShader { shader: 1 }).unwrap();
        assert_eq!(s.selected_shader, Some(1));
        s.apply(Command::SetVideoEffect { effect: 4 }).unwrap();
        s.apply(Command::SetVideoEffect { effect: 8 }).unwrap();
        assert_eq!(s.video_effect, 8);
        s.apply(Command::TriggerClip { clip: 0 }).unwrap();
        s.apply(Command::Reset).unwrap();
        assert!(!s.presented(0, 1));
        assert_eq!(s.selected_shader, None);
        assert_eq!(s.video_effect, 0);
        assert_eq!(s.speed, 1.0);
    }
    #[test]
    fn tempo_and_layer_amounts() {
        let mut s = State::new(0);
        s.apply(Command::TapTempo { time_ms: 1000 }).unwrap();
        s.apply(Command::TapTempo { time_ms: 1500 }).unwrap();
        assert_eq!(s.bpm, 120.0);
        s.apply(Command::SetEffectAmount {
            layer: Layer::Shader,
            amount: 0.25,
        })
        .unwrap();
        assert_eq!(s.video_effect_amount, 1.0);
        assert_eq!(s.shader_effect_amount, 0.25);
    }
}
