//! Novation Launchpad Mini MK3.
//!
//! The one device in the rig with a proper public Programmer's Reference
//! Manual. Two things matter and both are easy to get wrong:
//!
//!   * the device must be put into **Programmer Mode** before the note layout
//!     and full LED addressing behave as documented, and that must be re-sent
//!     on every reconnect;
//!   * it exposes two USB MIDI ports ("DAW" and "MIDI") — bind the MIDI port.
//!
//! Hardware pulse is worth using: a pulsing "awaiting quantized launch" pad
//! costs one message instead of a 30 Hz animation.

/// SysEx that switches the device into Programmer Mode.
pub const ENTER_PROGRAMMER_MODE: [u8; 9] =
    [0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x0E, 0x01, 0xF7];

/// SysEx that returns the device to its standalone Live layout.
pub const EXIT_PROGRAMMER_MODE: [u8; 9] =
    [0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x0E, 0x00, 0xF7];

const SYSEX_HEADER: [u8; 7] = [0xF0, 0x00, 0x20, 0x29, 0x02, 0x0D, 0x03];

/// One LED instruction inside a colour-spec message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedSpec {
    /// Palette index, 0..127.
    Static { index: u8, colour: u8 },
    /// Alternate between two palette colours at the device's blink rate.
    Flash { index: u8, colour_a: u8, colour_b: u8 },
    /// Hardware pulse — use this for a pending quantized launch.
    Pulse { index: u8, colour: u8 },
    /// Full RGB, 7-bit per channel.
    Rgb { index: u8, r: u8, g: u8, b: u8 },
}

/// Build one SysEx message lighting many pads at once.
///
/// Batching matters: refreshing an 8x8 grid one note-on at a time is 64
/// messages, and this is one.
pub fn colour_spec(specs: &[LedSpec]) -> Vec<u8> {
    let mut out = Vec::with_capacity(SYSEX_HEADER.len() + specs.len() * 6 + 1);
    out.extend_from_slice(&SYSEX_HEADER);
    for spec in specs {
        match *spec {
            LedSpec::Static { index, colour } => out.extend_from_slice(&[0, index, colour & 0x7f]),
            LedSpec::Flash { index, colour_a, colour_b } => {
                out.extend_from_slice(&[1, index, colour_a & 0x7f, colour_b & 0x7f])
            }
            LedSpec::Pulse { index, colour } => out.extend_from_slice(&[2, index, colour & 0x7f]),
            LedSpec::Rgb { index, r, g, b } => {
                out.extend_from_slice(&[3, index, r & 0x7f, g & 0x7f, b & 0x7f])
            }
        }
    }
    out.push(0xF7);
    out
}

/// Programmer-Mode note number for a grid position.
///
/// The grid is addressed as a decimal 10s/1s pair: row 0 is the *bottom*
/// row and is 11..18, row 7 (top) is 81..88.
pub fn note_for(row: u16, col: u16) -> Option<u8> {
    if row > 7 || col > 7 {
        return None;
    }
    let bottom_up = 7 - row;
    Some(((bottom_up + 1) * 10 + (col + 1)) as u8)
}

/// Inverse of [`note_for`].
pub fn position_for(note: u8) -> Option<(u16, u16)> {
    let tens = note / 10;
    let ones = note % 10;
    if !(1..=8).contains(&tens) || !(1..=8).contains(&ones) {
        return None;
    }
    Some((7 - (tens as u16 - 1), ones as u16 - 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programmer_mode_sysex_is_well_formed() {
        assert_eq!(ENTER_PROGRAMMER_MODE[0], 0xF0);
        assert_eq!(*ENTER_PROGRAMMER_MODE.last().unwrap(), 0xF7);
        assert_eq!(&ENTER_PROGRAMMER_MODE[1..4], &[0x00, 0x20, 0x29], "Novation manufacturer ID");
    }

    #[test]
    fn grid_note_mapping_round_trips() {
        for row in 0..8u16 {
            for col in 0..8u16 {
                let note = note_for(row, col).unwrap();
                assert_eq!(position_for(note), Some((row, col)), "row {row} col {col}");
            }
        }
    }

    #[test]
    fn bottom_left_is_eleven_and_top_right_is_eighty_eight() {
        assert_eq!(note_for(7, 0), Some(11));
        assert_eq!(note_for(0, 7), Some(88));
    }

    #[test]
    fn out_of_range_positions_are_rejected() {
        assert_eq!(note_for(8, 0), None);
        assert_eq!(position_for(9), None);
        assert_eq!(position_for(89), None);
    }

    #[test]
    fn a_full_grid_refresh_is_one_message() {
        let specs: Vec<_> = (0..64)
            .map(|i| LedSpec::Rgb { index: i, r: 100, g: 20, b: 5 })
            .collect();
        let msg = colour_spec(&specs);
        assert_eq!(msg[0], 0xF0);
        assert_eq!(*msg.last().unwrap(), 0xF7);
        assert_eq!(msg.iter().filter(|&&b| b == 0xF0).count(), 1, "exactly one message");
        // header + 64 * (type + index + rgb) + terminator
        assert_eq!(msg.len(), 7 + 64 * 5 + 1);
    }

    #[test]
    fn pulse_is_encoded_as_type_two() {
        let msg = colour_spec(&[LedSpec::Pulse { index: 11, colour: 5 }]);
        assert_eq!(&msg[7..10], &[2, 11, 5]);
    }

    #[test]
    fn sysex_payload_never_sets_the_high_bit() {
        let msg = colour_spec(&[LedSpec::Rgb { index: 11, r: 255, g: 255, b: 255 }]);
        for &b in &msg[7..msg.len() - 1] {
            assert!(b < 0x80, "SysEx data bytes must be 7-bit, saw {b:#04x}");
        }
    }
}
