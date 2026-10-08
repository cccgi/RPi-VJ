//! Behringer X-Touch Mini: MIDI events to commands, and state back to rings and LEDs.
//!
//! The numbers were measured on the owner's unit for the browser rig (gig/src/xtouch.js)
//! and the layout is the one used there:
//!
//! ```text
//!   knob 1  clip opacity          click: 100 %
//!   knob 2  shader opacity        click: 100 %
//!   knob 3  blend opacity         click: 50 %
//!   knob 4  clip speed 0.5-1.5x   click: 1.0x
//!   knob 5  video FX amount       click: 50 %
//!   knob 6  shader FX amount      click: 50 %
//!   fader   audio input gain (layer A), shader opacity (layer B)
//!   pads 1-8   clip on/off, shader on/off, opacity 25/50/75/100 %, blend prev/next
//!   pads 9-16  layer order, tap, BPM -, BPM +, -, HUD, reset, blackout
//!
//!   layer B knobs 1-5  how far one band bends the shader layer: bass bulge, low-mid
//!                      twist, mid wave, treble ripple, kick stretch
//!   layer B knobs 6-7  mirror segments, drift (spin; centre is still)
//!   layer B knob 8     shader brightness            click: back to the default
//!   layer B pads       as layer A
//! ```
use crate::control::{BlendMode, Command, Layer, State, MACRO_DEFAULTS};

/// Zero-based MIDI channel the unit was measured on; replaced by whatever it sends.
pub const CHANNEL: u8 = 10;
/// Ring slots: eight per layer.
pub const KNOBS: usize = 16;
const KNOB_CC: u8 = 1;
const FADER_A_CC: u8 = 9;
const FADER_B_CC: u8 = 10;
/// Layer B knobs are CC 11-18, their clicks notes 24-31 and the layer B pads notes 32-47.
const MACRO_CC: u8 = 11;
const MACRO_CLICK: u8 = 24;
const LAYER_B_PADS: u8 = 32;
pub const GAIN_MIN: f32 = 0.02;
pub const GAIN_MAX: f32 = 2.0;
const BLENDS: [BlendMode; 8] = [
    BlendMode::Alpha,
    BlendMode::Additive,
    BlendMode::Screen,
    BlendMode::Multiply,
    BlendMode::Overlay,
    BlendMode::SoftLight,
    BlendMode::LumaKey,
    BlendMode::Difference,
];
/// Pads whose LED mirrors state: clip on, shader on, clip on top, blackout. In standard
/// mode the unit numbers its pad LEDs 0-15 although the pads send notes 8-23, and takes
/// velocity 1 as on (2 would blink).
const LED_NOTES: [u8; 4] = [0, 1, 8, 15];
const LED_ON: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Control { number: u8, value: u8 },
    Note { number: u8, pressed: bool },
}

/// MIDI byte stream to events, with running status; realtime and system bytes are skipped.
#[derive(Debug, Default)]
pub struct Parser {
    status: u8,
    first: Option<u8>,
}
impl Parser {
    pub fn push(&mut self, byte: u8) -> Option<(u8, Event)> {
        if byte >= 0xf8 {
            return None;
        }
        if byte >= 0x80 {
            self.status = if byte < 0xf0 { byte } else { 0 };
            self.first = None;
            return None;
        }
        let kind = self.status & 0xf0;
        if !matches!(kind, 0x80 | 0x90 | 0xb0) {
            return None;
        }
        let Some(first) = self.first.take() else {
            self.first = Some(byte);
            return None;
        };
        let event = match kind {
            0xb0 => Event::Control { number: first, value: byte },
            0x90 => Event::Note { number: first, pressed: byte > 0 },
            _ => Event::Note { number: first, pressed: false },
        };
        Some((self.status & 0x0f, event))
    }
}

/// Ring slot of a knob CC: 0-7 on layer A, 8-15 on layer B.
pub fn knob_index(control: u8) -> Option<usize> {
    if (KNOB_CC..KNOB_CC + 8).contains(&control) {
        Some(usize::from(control - KNOB_CC))
    } else if (MACRO_CC..MACRO_CC + 8).contains(&control) {
        Some(8 + usize::from(control - MACRO_CC))
    } else {
        None
    }
}
/// Fader position to linear gain: off at the bottom, then 0.02x to 2x on a log curve.
pub fn fader_gain(position: f32) -> f32 {
    let position = position.clamp(0.0, 1.0);
    if position < 0.02 {
        return 0.0;
    }
    GAIN_MIN * (GAIN_MAX / GAIN_MIN).powf((position - 0.02) / 0.98)
}

/// What one event asks for. `last_clip` is the clip pad 1 brings back; `now_ms` stamps taps.
pub fn commands(event: Event, state: &State, last_clip: Option<usize>, now_ms: u64) -> Vec<Command> {
    let opacity = |opacity| Command::SetOpacity { opacity };
    let amount = |layer, amount| Command::SetEffectAmount { layer, amount };
    let blend = |step: usize| {
        let at = BLENDS.iter().position(|mode| *mode == state.blend).unwrap_or(0);
        Command::SetBlend {
            mode: BLENDS[(at + step) % BLENDS.len()],
            opacity: state.opacity,
        }
    };
    let one = match event {
        Event::Control { number, value } => {
            let v = f32::from(value.min(127)) / 127.0;
            match number {
                1 => Command::SetVideoOpacity { opacity: v },
                2 | 3 | FADER_B_CC => opacity(v),
                4 => Command::SetSpeed { speed: 0.5 + f64::from(v) },
                5 => amount(Layer::Video, v),
                6 => amount(Layer::Shader, v),
                FADER_A_CC => Command::SetInputGain { gain: fader_gain(v) },
                n if (MACRO_CC..MACRO_CC + 8).contains(&n) => Command::SetMacro { index: n - MACRO_CC, value: v },
                _ => return Vec::new(),
            }
        }
        Event::Note { pressed: false, .. } => return Vec::new(),
        Event::Note { number, .. } if (LAYER_B_PADS..LAYER_B_PADS + 16).contains(&number) => {
            let pad = Event::Note { number: number - LAYER_B_PADS + 8, pressed: true };
            return commands(pad, state, last_clip, now_ms);
        }
        Event::Note { number, .. } => match number {
            n if (MACRO_CLICK..MACRO_CLICK + 8).contains(&n) => Command::SetMacro {
                index: n - MACRO_CLICK,
                value: MACRO_DEFAULTS[usize::from(n - MACRO_CLICK)],
            },
            0 => Command::SetVideoOpacity { opacity: 1.0 },
            1 => opacity(1.0),
            2 => opacity(0.5),
            3 => Command::SetSpeed { speed: 1.0 },
            4 => amount(Layer::Video, 0.5),
            5 => amount(Layer::Shader, 0.5),
            8 if state.requested.or(state.playing).is_some() => Command::ClearClips,
            8 => match last_clip {
                Some(clip) => Command::TriggerClip { clip },
                None => return Vec::new(),
            },
            9 if state.selected_shader.is_some() => Command::SelectShader { enabled: !state.shader },
            10 => opacity(0.25),
            11 => opacity(0.5),
            12 => opacity(0.75),
            13 => opacity(1.0),
            14 => blend(BLENDS.len() - 1),
            15 => blend(1),
            16 => Command::ToggleLayerOrder,
            17 => Command::TapTempo { time_ms: now_ms },
            18 => Command::AdjustBpm { delta: -1.0 },
            19 => Command::AdjustBpm { delta: 1.0 },
            21 => Command::ToggleHud,
            22 => Command::Reset,
            23 => Command::Blackout { enabled: !state.blackout },
            _ => return Vec::new(),
        },
    };
    vec![one]
}

fn raw(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 127.0).round() as u8
}
/// What the unit was last told, or last reported; only differences are written.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Feedback {
    pub knobs: [Option<u8>; KNOBS],
    leds: [Option<bool>; LED_NOTES.len()],
}
impl Feedback {
    /// MIDI messages that bring rings and LEDs in line with `state`; the second item is
    /// the knob a message moves, if any.
    pub fn update(&mut self, state: &State, channel: u8) -> Vec<([u8; 3], Option<usize>)> {
        let m = state.macros;
        let rings = [
            (1, state.video_opacity),
            (2, state.opacity),
            (3, state.opacity),
            (4, (state.speed - 0.5) as f32),
            (5, state.video_effect_amount),
            (6, state.shader_effect_amount),
            (11, m[0]),
            (12, m[1]),
            (13, m[2]),
            (14, m[3]),
            (15, m[4]),
            (16, m[5]),
            (17, m[6]),
            (18, m[7]),
        ];
        let lit = [
            state.requested.or(state.playing).is_some(),
            state.shader,
            state.invert_order,
            state.blackout,
        ];
        let mut out = Vec::new();
        for (control, value) in rings {
            let value = raw(value);
            let Some(knob) = knob_index(control) else { continue };
            if self.knobs[knob] != Some(value) {
                self.knobs[knob] = Some(value);
                out.push(([0xb0 | channel, control, value], Some(knob)));
            }
        }
        for (index, on) in lit.into_iter().enumerate() {
            if self.leds[index] != Some(on) {
                self.leds[index] = Some(on);
                let velocity = if on { LED_ON } else { 0 };
                out.push(([0x90 | channel, LED_NOTES[index], velocity], None));
                // The unit lights pads addressed on channel 1 as well; the browser rig sent both.
                if channel != 0 {
                    out.push(([0x90, LED_NOTES[index], velocity], None));
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn apply(state: &mut State, event: Event) {
        for command in commands(event, state, Some(0), 1000) {
            state.apply(command).unwrap();
        }
    }
    #[test]
    fn parser_handles_running_status_and_note_off_forms() {
        let mut parser = Parser::default();
        let bytes = [0xba, 9, 28, 10, 64, 0xf8, 0x9a, 23, 127, 23, 0, 0x8a, 8, 64, 0xca, 5, 0xba, 1];
        let events: Vec<_> = bytes.iter().filter_map(|b| parser.push(*b)).collect();
        assert_eq!(
            events,
            vec![
                (10, Event::Control { number: 9, value: 28 }),
                (10, Event::Control { number: 10, value: 64 }),
                (10, Event::Note { number: 23, pressed: true }),
                (10, Event::Note { number: 23, pressed: false }),
                (10, Event::Note { number: 8, pressed: false }),
            ]
        );
    }
    #[test]
    fn fader_is_off_at_the_bottom_and_spans_the_original_gain_range() {
        assert_eq!(fader_gain(0.0), 0.0);
        assert!((fader_gain(0.02) - GAIN_MIN).abs() < 1e-6);
        assert!((fader_gain(1.0) - GAIN_MAX).abs() < 1e-5);
        assert!(fader_gain(0.6) > fader_gain(0.5));
    }
    #[test]
    fn knobs_fader_and_pads_follow_the_original_layout() {
        let mut state = State::new(2);
        state.apply(Command::TriggerClip { clip: 1 }).unwrap();
        apply(&mut state, Event::Control { number: 1, value: 0 });
        apply(&mut state, Event::Control { number: 3, value: 127 });
        apply(&mut state, Event::Control { number: 4, value: 127 });
        apply(&mut state, Event::Control { number: 9, value: 127 });
        assert_eq!((state.video_opacity, state.opacity, state.speed), (0.0, 1.0, 1.5));
        assert!((state.input_gain - GAIN_MAX).abs() < 1e-5);
        apply(&mut state, Event::Note { number: 0, pressed: true });
        apply(&mut state, Event::Note { number: 3, pressed: true });
        apply(&mut state, Event::Note { number: 10, pressed: true });
        assert_eq!((state.video_opacity, state.opacity, state.speed), (1.0, 0.25, 1.0));
        // Blend steps wrap in both directions and keep the opacity.
        apply(&mut state, Event::Note { number: 14, pressed: true });
        assert_eq!((state.blend, state.opacity), (BlendMode::Difference, 0.25));
        apply(&mut state, Event::Note { number: 15, pressed: true });
        assert_eq!(state.blend, BlendMode::Alpha);
        // Pad 1 clears the clip, then brings the last one back; releases do nothing.
        apply(&mut state, Event::Note { number: 8, pressed: true });
        assert_eq!(state.requested, None);
        apply(&mut state, Event::Note { number: 8, pressed: false });
        assert_eq!(state.requested, None);
        apply(&mut state, Event::Note { number: 8, pressed: true });
        assert_eq!(state.requested, Some(0));
        apply(&mut state, Event::Note { number: 16, pressed: true });
        apply(&mut state, Event::Note { number: 23, pressed: true });
        assert!(state.invert_order && state.blackout);
        // Reset restores the look but leaves the input trim alone.
        apply(&mut state, Event::Note { number: 22, pressed: true });
        assert!(!state.invert_order && !state.blackout && state.video_opacity == 1.0);
        assert!((state.input_gain - GAIN_MAX).abs() < 1e-5);
    }
    #[test]
    fn feedback_writes_only_what_changed() {
        let mut state = State::new(1);
        let mut feedback = Feedback::default();
        assert_eq!(feedback.update(&state, 10).len(), 6 + 8 + 4 * 2);
        assert!(feedback.update(&state, 10).is_empty());
        state.apply(Command::SetOpacity { opacity: 1.0 }).unwrap();
        // Knobs 2 and 3 both show the shader opacity.
        assert_eq!(
            feedback.update(&state, 10),
            vec![([0xba, 2, 127], Some(1)), ([0xba, 3, 127], Some(2))]
        );
        // A knob the operator just turned is already where the state is.
        feedback.knobs[0] = Some(64);
        state.apply(Command::SetVideoOpacity { opacity: 64.0 / 127.0 }).unwrap();
        assert!(feedback.update(&state, 10).is_empty());
        state.apply(Command::Blackout { enabled: true }).unwrap();
        assert_eq!(
            feedback.update(&state, 10),
            vec![([0x9a, 15, 1], None), ([0x90, 15, 1], None)]
        );
        // Layer B: a macro knob moves its own ring only, and its click restores the default.
        apply(&mut state, Event::Control { number: 18, value: 127 });
        apply(&mut state, Event::Control { number: 11, value: 127 });
        assert_eq!((state.macros[7], state.macros[0]), (1.0, 1.0));
        assert_eq!(feedback.update(&state, 10).len(), 2);
        apply(&mut state, Event::Note { number: 31, pressed: true });
        assert_eq!(state.macros[7], 0.5);
        apply(&mut state, Event::Note { number: 47, pressed: true });
        assert!(!state.blackout);
    }
}
