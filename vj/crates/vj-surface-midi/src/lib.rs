//! MIDI transport: `midir` on all three platforms (ALSA sequencer, CoreMIDI,
//! WinMM/WinRT).
//!
//! Platform note that bites in practice: WinMM opens a MIDI input port
//! **exclusively**. If Resolume or rekordbox holds the device, this cannot
//! open it, and vice versa. Prefer the Windows MIDI Services path where
//! available; otherwise partition devices between applications explicitly and
//! report the failure rather than silently running without the controller.

use vj_surface::{ControlEvent, ControllerAdapter, SurfaceError};
use vj_types::SurfaceId;

/// A device discovered by port enumeration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MidiPort {
    pub name: String,
    pub index: usize,
}

/// Match a port by substring, the way profiles declare them.
pub fn find_port<'a>(ports: &'a [MidiPort], needle: &str) -> Option<&'a MidiPort> {
    let needle = needle.to_lowercase();
    ports.iter().find(|p| p.name.to_lowercase().contains(&needle))
}

/// Parse a MIDI channel-voice message into a `(status, channel, d1, d2)` tuple.
/// Returns `None` for SysEx and real-time bytes, which drivers handle.
pub fn parse_channel_message(bytes: &[u8]) -> Option<(u8, u8, u8, u8)> {
    let &status = bytes.first()?;
    if !(0x80..0xF0).contains(&status) {
        return None;
    }
    let kind = status & 0xF0;
    let channel = status & 0x0F;
    let d1 = *bytes.get(1)? & 0x7f;
    let d2 = bytes.get(2).copied().unwrap_or(0) & 0x7f;
    Some((kind, channel, d1, d2))
}

/// Decode a 7-bit relative CC value (e.g., standard two's complement / offset).
/// 1..63 -> +1..+63, 65..127 -> -63..-1.
#[inline]
pub fn decode_relative_cc(value: u8) -> i32 {
    if value == 0 {
        0
    } else if value < 64 {
        value as i32
    } else {
        (value as i32) - 128
    }
}

/// Encode a 3-byte Channel Voice message.
#[inline]
pub fn encode_channel_message(status_kind: u8, channel: u8, d1: u8, d2: u8) -> [u8; 3] {
    [(status_kind & 0xF0) | (channel & 0x0F), d1 & 0x7F, d2 & 0x7F]
}

/// Encode a Note On message (0x90).
#[inline]
pub fn encode_note_on(channel: u8, note: u8, velocity: u8) -> [u8; 3] {
    encode_channel_message(0x90, channel, note, velocity)
}

/// Encode a Note Off message (0x80).
#[inline]
pub fn encode_note_off(channel: u8, note: u8, velocity: u8) -> [u8; 3] {
    encode_channel_message(0x80, channel, note, velocity)
}

/// Encode a Control Change message (0xB0).
#[inline]
pub fn encode_cc(channel: u8, controller: u8, value: u8) -> [u8; 3] {
    encode_channel_message(0xB0, channel, controller, value)
}

pub struct MidiAdapter {
    pub surface: SurfaceId,
    pub connected: bool,
}

impl MidiAdapter {
    pub fn new(surface: SurfaceId) -> Self {
        Self {
            surface,
            connected: false,
        }
    }
}

impl ControllerAdapter for MidiAdapter {
    fn surface_id(&self) -> SurfaceId {
        self.surface.clone()
    }
    fn poll(&mut self, _out: &mut Vec<ControlEvent>) -> Result<(), SurfaceError> {
        // Wired to a midir input connection in Phase B5.
        Ok(())
    }
    fn apply_feedback(&mut self, _frame: &vj_surface::FeedbackFrame) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ports_match_by_substring_case_insensitively() {
        let ports = vec![
            MidiPort {
                name: "LPMiniMK3 DAW Out".into(),
                index: 0,
            },
            MidiPort {
                name: "LPMiniMK3 MIDI Out".into(),
                index: 1,
            },
        ];
        assert_eq!(find_port(&ports, "midi").unwrap().index, 1);
        assert_eq!(find_port(&ports, "daw").unwrap().index, 0);
        assert!(find_port(&ports, "fire").is_none());
    }

    #[test]
    fn note_on_is_parsed() {
        assert_eq!(
            parse_channel_message(&[0x90, 0x3C, 0x64]),
            Some((0x90, 0, 60, 100))
        );
    }

    #[test]
    fn sysex_and_realtime_are_left_to_drivers() {
        assert_eq!(parse_channel_message(&[0xF0, 0x00, 0x20]), None);
        assert_eq!(parse_channel_message(&[0xF8]), None);
    }

    #[test]
    fn test_decode_relative_cc() {
        assert_eq!(decode_relative_cc(1), 1);
        assert_eq!(decode_relative_cc(63), 63);
        assert_eq!(decode_relative_cc(127), -1);
        assert_eq!(decode_relative_cc(65), -63);
        assert_eq!(decode_relative_cc(0), 0);
    }

    #[test]
    fn test_encode_messages() {
        assert_eq!(encode_note_on(0, 8, 127), [0x90, 8, 127]);
        assert_eq!(encode_note_off(0, 8, 0), [0x80, 8, 0]);
        assert_eq!(encode_cc(0, 16, 6), [0xB0, 16, 6]);
    }
}
