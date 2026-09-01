//! USB HID transport and the Stream Deck family.
//!
//! HID is multi-client-hostile everywhere: exactly one process may claim the
//! interface. On Linux this also needs a udev rule granting the service user
//! access to the Elgato hidraw nodes.

pub mod device;

pub use device::{
    frame_key_image_report, IconCache, StreamDeckXL, STREAM_DECK_XL_COLS, STREAM_DECK_XL_HEADER_SIZE,
    STREAM_DECK_XL_ICON_SIZE, STREAM_DECK_XL_KEY_COUNT, STREAM_DECK_XL_PAYLOAD_SIZE,
    STREAM_DECK_XL_REPORT_SIZE, STREAM_DECK_XL_ROWS,
};

use vj_surface::{ControlEvent, ControllerAdapter, FeedbackFrame, SurfaceError};
use vj_types::SurfaceId;

pub const ELGATO_VENDOR_ID: u16 = 0x0fd9;
/// Stream Deck XL product IDs. Confirm both against your units on target.
pub const STREAM_DECK_XL_PRODUCT_IDS: [u16; 2] = [0x006c, 0x008f];

/// Diff a fresh 32-key button state against the previous one and emit only
/// the transitions.
///
/// The bug this exists to prevent: the current `src/streamdeck.rs` re-fires
/// `Pressed` on every poll tick a key reads as held, which produces repeat
/// triggers. Only 0→1 and 1→0 edges may produce events.
pub fn diff_buttons(prev: &[bool], next: &[bool]) -> Vec<(usize, bool)> {
    prev.iter()
        .zip(next.iter())
        .enumerate()
        .filter_map(|(i, (&p, &n))| (p != n).then_some((i, n)))
        .collect()
}

pub struct HidAdapter {
    pub surface: SurfaceId,
    pub connected: bool,
    prev: Vec<bool>,
}

impl HidAdapter {
    pub fn new(surface: SurfaceId, keys: usize) -> Self {
        Self {
            surface,
            connected: false,
            prev: vec![false; keys],
        }
    }

    /// Feed a raw button-state report; returns the edges.
    pub fn ingest(&mut self, next: &[bool]) -> Vec<(usize, bool)> {
        let edges = diff_buttons(&self.prev, next);
        self.prev = next.to_vec();
        edges
    }
}

impl ControllerAdapter for HidAdapter {
    fn surface_id(&self) -> SurfaceId {
        self.surface.clone()
    }
    fn poll(&mut self, _out: &mut Vec<ControlEvent>) -> Result<(), SurfaceError> {
        Ok(())
    }
    fn apply_feedback(&mut self, _frame: &FeedbackFrame) -> Result<(), SurfaceError> {
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
    fn a_held_key_fires_exactly_once() {
        let mut a = HidAdapter::new(SurfaceId::new("sd"), 4);
        let held = [false, true, false, false];
        assert_eq!(a.ingest(&held), vec![(1, true)]);
        // Held across many polls: no further events.
        for _ in 0..10 {
            assert!(a.ingest(&held).is_empty());
        }
        assert_eq!(a.ingest(&[false; 4]), vec![(1, false)]);
    }

    #[test]
    fn simultaneous_presses_are_all_reported() {
        let mut a = HidAdapter::new(SurfaceId::new("sd"), 4);
        assert_eq!(
            a.ingest(&[true, false, true, false]),
            vec![(0, true), (2, true)]
        );
    }
}
