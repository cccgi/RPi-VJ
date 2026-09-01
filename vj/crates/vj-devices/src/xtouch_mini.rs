//! Behringer X-Touch Mini MIDI driver: relative encoders, buttons, and LED ring feedback.

pub const XTOUCH_MINI_ENCODER_COUNT: usize = 8;
pub const XTOUCH_MINI_BUTTON_COUNT: usize = 24; // 8 encoder push + 8 row1 + 8 row2

pub const CC_ENCODER_BASE: u8 = 16; // CC 16..23
pub const CC_MASTER_FADER: u8 = 9;

pub const NOTE_ENCODER_PUSH_BASE: u8 = 0; // Notes 0..7
pub const NOTE_LAYER_A_ROW1_BASE: u8 = 8; // Notes 8..15
pub const NOTE_LAYER_A_ROW2_BASE: u8 = 16; // Notes 16..23

/// Ring display modes for X-Touch Mini encoder LED rings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingMode {
    SingleDot, // Single LED at value position
    Pan,       // Center-outwards
    Fan,       // 0 to value
    Spread,    // Center outwards both ways
    Trim,      // Single LED with dot at center
}

/// Control event parsed from X-Touch Mini MIDI message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XTouchControlEvent {
    EncoderDelta { encoder: u8, delta: i32 },
    ButtonPress { note: u8 },
    ButtonRelease { note: u8 },
    MasterFader { value: u8, normalized: f32 },
}

/// Decode a 7-bit relative CC two's complement value.
/// 1..63 = clockwise (+1..+63)
/// 65..127 = counter-clockwise (-63..-1, e.g. 127 = -1)
#[inline]
pub fn decode_relative_encoder(value: u8) -> i32 {
    if value == 0 {
        0
    } else if value < 64 {
        value as i32
    } else {
        (value as i32) - 128
    }
}

/// Parse a raw 3-byte MIDI channel message into an XTouchControlEvent without heap allocation.
pub fn parse_midi_message(bytes: &[u8]) -> Option<XTouchControlEvent> {
    if bytes.len() < 3 {
        return None;
    }
    let status = bytes[0] & 0xF0;
    let channel = bytes[0] & 0x0F;
    if channel != 0 {
        return None;
    }
    let d1 = bytes[1] & 0x7F;
    let d2 = bytes[2] & 0x7F;

    match status {
        // Note On (0x90)
        0x90 => {
            if d2 > 0 {
                Some(XTouchControlEvent::ButtonPress { note: d1 })
            } else {
                Some(XTouchControlEvent::ButtonRelease { note: d1 })
            }
        }
        // Note Off (0x80)
        0x80 => Some(XTouchControlEvent::ButtonRelease { note: d1 }),
        // Control Change (0xB0)
        0xB0 => {
            if (CC_ENCODER_BASE..CC_ENCODER_BASE + 8).contains(&d1) {
                let encoder = d1 - CC_ENCODER_BASE;
                let delta = decode_relative_encoder(d2);
                Some(XTouchControlEvent::EncoderDelta { encoder, delta })
            } else if d1 == CC_MASTER_FADER {
                let normalized = d2 as f32 / 127.0;
                Some(XTouchControlEvent::MasterFader {
                    value: d2,
                    normalized,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Format an LED ring CC feedback message.
///
/// CC 16..23 on Channel 0 (0xB0).
/// `val` is normalized 0.0..1.0, mapped to segment levels 0..11.
pub fn encode_ring_feedback(encoder: u8, _mode: RingMode, val: f32) -> [u8; 3] {
    let cc = CC_ENCODER_BASE + (encoder % 8);
    let clamped = val.clamp(0.0, 1.0);
    // Segment level 0..11 (center at 6 for 0.5 in pan mode)
    let level = (clamped * 11.0).round() as u8;
    [0xB0, cc, level.min(11)]
}

/// Format a button LED feedback message.
/// Note On on Channel 0 with velocity 127 (on) or 0 (off).
pub fn encode_button_led(note: u8, on: bool) -> [u8; 3] {
    let vel = if on { 127 } else { 0 };
    [0x90, note & 0x7F, vel]
}

/// State tracker and driver for Behringer X-Touch Mini.
#[derive(Debug, Clone)]
pub struct XTouchMini {
    encoder_values: [f32; 8],
    ring_modes: [RingMode; 8],
    button_states: [bool; 24],
    fader_value: f32,
}

impl Default for XTouchMini {
    fn default() -> Self {
        Self::new()
    }
}

impl XTouchMini {
    pub fn new() -> Self {
        Self {
            encoder_values: [0.5; 8],
            ring_modes: [RingMode::Pan; 8],
            button_states: [false; 24],
            fader_value: 0.0,
        }
    }

    /// Process a raw MIDI byte slice without allocating heap memory.
    #[inline]
    pub fn process_midi(&mut self, bytes: &[u8]) -> Option<XTouchControlEvent> {
        let event = parse_midi_message(bytes)?;
        match event {
            XTouchControlEvent::EncoderDelta { encoder, delta } => {
                let idx = (encoder as usize).min(7);
                let current = self.encoder_values[idx];
                let step = delta as f32 * 0.02;
                self.encoder_values[idx] = (current + step).clamp(0.0, 1.0);
            }
            XTouchControlEvent::ButtonPress { note } => {
                if (note as usize) < 24 {
                    self.button_states[note as usize] = true;
                }
            }
            XTouchControlEvent::ButtonRelease { note } => {
                if (note as usize) < 24 {
                    self.button_states[note as usize] = false;
                }
            }
            XTouchControlEvent::MasterFader { normalized, .. } => {
                self.fader_value = normalized;
            }
        }
        Some(event)
    }

    pub fn set_ring_mode(&mut self, encoder: u8, mode: RingMode) {
        if (encoder as usize) < 8 {
            self.ring_modes[encoder as usize] = mode;
        }
    }

    pub fn ring_mode(&self, encoder: u8) -> RingMode {
        if (encoder as usize) < 8 {
            self.ring_modes[encoder as usize]
        } else {
            RingMode::SingleDot
        }
    }

    pub fn encoder_value(&self, encoder: u8) -> f32 {
        if (encoder as usize) < 8 {
            self.encoder_values[encoder as usize]
        } else {
            0.0
        }
    }

    pub fn button_pressed(&self, note: u8) -> bool {
        if (note as usize) < 24 {
            self.button_states[note as usize]
        } else {
            false
        }
    }

    pub fn master_fader(&self) -> f32 {
        self.fader_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xtouch_relative_encoder_delta() {
        // CC 16 (Encoder 1), value 1 (Clockwise)
        let cw_msg = [0xB0, 16, 1];
        let cw_event = parse_midi_message(&cw_msg).expect("valid CC message");
        assert_eq!(
            cw_event,
            XTouchControlEvent::EncoderDelta {
                encoder: 0,
                delta: 1
            }
        );
        assert_eq!(decode_relative_encoder(1), 1);

        // CC 16 (Encoder 1), value 127 (Counter-clockwise)
        let ccw_msg = [0xB0, 16, 127];
        let ccw_event = parse_midi_message(&ccw_msg).expect("valid CC message");
        assert_eq!(
            ccw_event,
            XTouchControlEvent::EncoderDelta {
                encoder: 0,
                delta: -1
            }
        );
        assert_eq!(decode_relative_encoder(127), -1);

        // CC 23 (Encoder 8), value 2 (Clockwise +2)
        let cw2_msg = [0xB0, 23, 2];
        let cw2_event = parse_midi_message(&cw2_msg).expect("valid CC message");
        assert_eq!(
            cw2_event,
            XTouchControlEvent::EncoderDelta {
                encoder: 7,
                delta: 2
            }
        );
        assert_eq!(decode_relative_encoder(2), 2);

        // CC 23 (Encoder 8), value 126 (Counter-clockwise -2)
        let ccw2_msg = [0xB0, 23, 126];
        let ccw2_event = parse_midi_message(&ccw2_msg).expect("valid CC message");
        assert_eq!(
            ccw2_event,
            XTouchControlEvent::EncoderDelta {
                encoder: 7,
                delta: -2
            }
        );
        assert_eq!(decode_relative_encoder(126), -2);
    }

    #[test]
    fn test_xtouch_ring_feedback_encoding() {
        // Float value 0.5 in Pan mode formats valid CC byte sequence
        let bytes = encode_ring_feedback(0, RingMode::Pan, 0.5);
        // Channel 0 CC -> status 0xB0, CC 16 (for encoder 0)
        assert_eq!(bytes[0], 0xB0);
        assert_eq!(bytes[1], 16);
        // Value for 0.5 should be 6 in 0..11 range
        assert_eq!(bytes[2], 6);

        // Test encoder 7 (CC 23)
        let bytes7 = encode_ring_feedback(7, RingMode::Pan, 0.5);
        assert_eq!(bytes7[0], 0xB0);
        assert_eq!(bytes7[1], 23);
        assert_eq!(bytes7[2], 6);
    }

    #[test]
    fn test_xtouch_button_press_release() {
        // Note 8 on (velocity 127) -> Press
        let msg_on = [0x90, 8, 127];
        let ev_on = parse_midi_message(&msg_on).expect("valid note on");
        assert_eq!(ev_on, XTouchControlEvent::ButtonPress { note: 8 });

        // Note 8 off (0x80) -> Release
        let msg_off = [0x80, 8, 0];
        let ev_off = parse_midi_message(&msg_off).expect("valid note off");
        assert_eq!(ev_off, XTouchControlEvent::ButtonRelease { note: 8 });

        // Note 8 on with velocity 0 -> Release
        let msg_on_zero = [0x90, 8, 0];
        let ev_zero = parse_midi_message(&msg_on_zero).expect("valid note on zero vel");
        assert_eq!(ev_zero, XTouchControlEvent::ButtonRelease { note: 8 });

        // Encoders push (Notes 0..7)
        let enc_push = [0x90, 3, 127];
        assert_eq!(
            parse_midi_message(&enc_push),
            Some(XTouchControlEvent::ButtonPress { note: 3 })
        );
        let enc_rel = [0x80, 3, 64];
        assert_eq!(
            parse_midi_message(&enc_rel),
            Some(XTouchControlEvent::ButtonRelease { note: 3 })
        );
    }
}
