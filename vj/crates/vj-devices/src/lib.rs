//! Device profiles and the drivers for products whose LED/display protocol
//! needs code rather than a declarative TOML profile.
//!
//! This is the only crate that names products. Nothing above it does.

use vj_surface::{SurfaceError, SurfaceModel};

pub mod launchpad_mini_mk3;
pub mod xtouch_mini;

pub use xtouch_mini::{
    decode_relative_encoder, encode_button_led, encode_ring_feedback, parse_midi_message, RingMode,
    XTouchControlEvent, XTouchMini, CC_ENCODER_BASE, CC_MASTER_FADER, NOTE_ENCODER_PUSH_BASE,
    NOTE_LAYER_A_ROW1_BASE, NOTE_LAYER_A_ROW2_BASE, XTOUCH_MINI_BUTTON_COUNT,
    XTOUCH_MINI_ENCODER_COUNT,
};

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("failed to parse profile: {0}")]
    Parse(String),
    #[error("unknown driver: {0}")]
    UnknownDriver(String),
}

/// Load a `SurfaceModel` from a profile TOML.
pub fn load_model(toml_src: &str) -> Result<SurfaceModel, ProfileError> {
    toml::from_str(toml_src).map_err(|e| ProfileError::Parse(e.to_string()))
}

/// Clamp an RGB triple into the 7-bit range MIDI SysEx carries.
pub fn rgb_to_7bit(rgb: [u8; 3]) -> [u8; 3] {
    [rgb[0] >> 1, rgb[1] >> 1, rgb[2] >> 1]
}

pub type DriverResult<T> = Result<T, SurfaceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_is_narrowed_to_seven_bits() {
        assert_eq!(rgb_to_7bit([255, 128, 0]), [127, 64, 0]);
        for c in rgb_to_7bit([255, 255, 255]) {
            assert!(c <= 127);
        }
    }
}
