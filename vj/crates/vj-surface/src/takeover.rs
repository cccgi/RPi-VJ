//! Soft takeover for absolute controls.
//!
//! An absolute knob (APC, MPK, X-Touch fader) holds a physical position that
//! stops matching the parameter as soon as a bank or page changes. Moving it
//! then jumps the parameter. Each policy trades a different discomfort.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use vj_types::ParameterPath;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeoverMode {
    /// Apply immediately. Fine for faders where a jump is acceptable.
    Jump,
    /// Ignore until the physical value crosses the parameter value.
    /// Safest; feels dead until crossed.
    Pickup,
    /// Move the parameter proportionally toward the endpoint the control is
    /// heading for. The recommended default for knobs.
    #[default]
    Scale,
    /// The control is relative; takeover does not apply.
    Relative,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    /// Last physical value seen from the hardware, 0..1.
    physical: f32,
    /// Whether the control has taken over the parameter.
    engaged: bool,
}

/// Per-binding takeover bookkeeping. Reset on bank change so the next move
/// re-acquires rather than jumping.
#[derive(Clone, Debug, Default)]
pub struct TakeoverState {
    entries: HashMap<String, Entry>,
}

impl TakeoverState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drop all engagement. Call on bank/page/layer change.
    pub fn reset(&mut self) {
        self.entries.clear();
    }

    pub fn reset_path(&mut self, path: &ParameterPath) {
        self.entries.remove(&path.0);
    }

    /// Apply a new physical value.
    ///
    /// `current` is the parameter's present value. Returns `Some(new_value)`
    /// when the parameter should change, `None` when the move is swallowed.
    pub fn apply(
        &mut self,
        path: &ParameterPath,
        mode: TakeoverMode,
        physical: f32,
        current: f32,
    ) -> Option<f32> {
        let physical = physical.clamp(0.0, 1.0);
        let current = current.clamp(0.0, 1.0);

        match mode {
            TakeoverMode::Jump | TakeoverMode::Relative => {
                self.entries
                    .insert(path.0.clone(), Entry { physical, engaged: true });
                Some(physical)
            }

            TakeoverMode::Pickup => {
                let prev = self.entries.get(&path.0).copied();
                let engaged = prev.map(|e| e.engaged).unwrap_or(false);

                if engaged {
                    self.entries
                        .insert(path.0.clone(), Entry { physical, engaged: true });
                    return Some(physical);
                }

                let crossed = match prev {
                    // Crossed the parameter value between the last physical
                    // reading and this one.
                    Some(e) => (e.physical - current).signum() != (physical - current).signum(),
                    None => (physical - current).abs() < 1e-4,
                };

                self.entries
                    .insert(path.0.clone(), Entry { physical, engaged: crossed });
                crossed.then_some(physical)
            }

            TakeoverMode::Scale => {
                let prev = self.entries.get(&path.0).copied();
                self.entries
                    .insert(path.0.clone(), Entry { physical, engaged: true });

                let Some(prev) = prev else {
                    // First touch after a reset: do not move the parameter,
                    // just record where the hardware is.
                    return None;
                };

                let delta = physical - prev.physical;
                if delta.abs() < 1e-6 {
                    return None;
                }

                // Scale the physical movement by the remaining headroom on the
                // side it is heading toward, so the control converges on the
                // endpoint rather than snapping.
                let scaled = if delta > 0.0 {
                    let head_phys = (1.0 - prev.physical).max(1e-6);
                    let head_param = 1.0 - current;
                    current + delta / head_phys * head_param
                } else {
                    let head_phys = prev.physical.max(1e-6);
                    let head_param = current;
                    current + delta / head_phys * head_param
                };

                Some(scaled.clamp(0.0, 1.0))
            }
        }
    }

    /// Fold a relative encoder delta into a parameter value.
    pub fn apply_relative(current: f32, detents: i16, step: f32) -> f32 {
        (current + detents as f32 * step).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> ParameterPath {
        ParameterPath::new("layer.0.opacity")
    }

    #[test]
    fn jump_applies_immediately() {
        let mut st = TakeoverState::new();
        assert_eq!(st.apply(&path(), TakeoverMode::Jump, 0.9, 0.1), Some(0.9));
    }

    #[test]
    fn pickup_swallows_until_crossing() {
        let mut st = TakeoverState::new();
        // Parameter sits at 0.5; hardware starts low and sweeps up.
        assert_eq!(st.apply(&path(), TakeoverMode::Pickup, 0.10, 0.5), None);
        assert_eq!(st.apply(&path(), TakeoverMode::Pickup, 0.30, 0.5), None);
        // Crossing 0.5 engages.
        assert_eq!(st.apply(&path(), TakeoverMode::Pickup, 0.60, 0.5), Some(0.60));
        // Once engaged it tracks directly.
        assert_eq!(st.apply(&path(), TakeoverMode::Pickup, 0.70, 0.60), Some(0.70));
    }

    #[test]
    fn scale_never_jumps_and_converges_upward() {
        let mut st = TakeoverState::new();
        // Hardware is at 0.2, parameter at 0.9. First touch only records.
        assert_eq!(st.apply(&path(), TakeoverMode::Scale, 0.2, 0.9), None);

        // Sweeping the hardware to the top must land on 1.0, not snap.
        let mut current = 0.9_f32;
        for step in 1..=8 {
            let physical = 0.2 + (1.0 - 0.2) * (step as f32 / 8.0);
            if let Some(v) = st.apply(&path(), TakeoverMode::Scale, physical, current) {
                assert!(v >= current - 1e-5, "parameter must not jump backwards");
                current = v;
            }
        }
        assert!((current - 1.0).abs() < 1e-3, "expected convergence to 1.0, got {current}");
    }

    #[test]
    fn scale_converges_downward() {
        let mut st = TakeoverState::new();
        assert_eq!(st.apply(&path(), TakeoverMode::Scale, 0.8, 0.1), None);
        let mut current = 0.1_f32;
        for step in 1..=8 {
            let physical = 0.8 - 0.8 * (step as f32 / 8.0);
            if let Some(v) = st.apply(&path(), TakeoverMode::Scale, physical, current) {
                current = v;
            }
        }
        assert!(current.abs() < 1e-3, "expected convergence to 0.0, got {current}");
    }

    #[test]
    fn reset_forces_reacquisition() {
        let mut st = TakeoverState::new();
        st.apply(&path(), TakeoverMode::Scale, 0.5, 0.5);
        st.reset();
        // After a bank change the first move is swallowed again.
        assert_eq!(st.apply(&path(), TakeoverMode::Scale, 0.9, 0.2), None);
    }

    #[test]
    fn relative_steps_are_bounded() {
        assert_eq!(TakeoverState::apply_relative(0.98, 5, 0.01), 1.0);
        assert_eq!(TakeoverState::apply_relative(0.02, -5, 0.01), 0.0);
    }
}
