//! Guarded action manager providing 3-second long-hold timer with visual feedback
//! and 2-key confirmation protection for dangerous system and ingest operations.

use std::time::{Duration, Instant};

pub const DEFAULT_HOLD_DURATION: Duration = Duration::from_millis(3000);
pub const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GuardedAction {
    Shutdown,
    Reboot,
    RestartRenderer,
    ActivateShow,
    CancelIngest,
    DeleteShow,
}

impl GuardedAction {
    pub fn label(&self) -> &'static str {
        match self {
            GuardedAction::Shutdown => "SHUTDOWN",
            GuardedAction::Reboot => "REBOOT",
            GuardedAction::RestartRenderer => "RESTART RENDERER",
            GuardedAction::ActivateShow => "ACTIVATE SHOW",
            GuardedAction::CancelIngest => "CANCEL INGEST",
            GuardedAction::DeleteShow => "DELETE SHOW",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ActiveHold {
    pub action: GuardedAction,
    pub key: u8,
    pub start: Instant,
    pub duration: Duration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoKeyPrompt {
    pub action: GuardedAction,
    pub initiated_key: u8,
    pub confirm_key: u8,
    pub cancel_key: u8,
}

#[derive(Clone, Debug)]
pub enum GuardedEngineState {
    Idle,
    Holding(ActiveHold),
    TwoKeyPending {
        prompt: TwoKeyPrompt,
        deadline: Instant,
    },
}

pub struct GuardedEngine {
    state: GuardedEngineState,
    hold_duration: Duration,
}

impl Default for GuardedEngine {
    fn default() -> Self {
        Self::new(DEFAULT_HOLD_DURATION)
    }
}

impl GuardedEngine {
    pub fn new(hold_duration: Duration) -> Self {
        Self {
            state: GuardedEngineState::Idle,
            hold_duration,
        }
    }

    /// Call on key press of a guarded action key.
    pub fn on_press(&mut self, action: GuardedAction, key: u8, now: Instant) {
        match &self.state {
            GuardedEngineState::TwoKeyPending { prompt, .. } => {
                if key == prompt.confirm_key {
                    // Handled in handle_two_key_press
                }
            }
            _ => {
                self.state = GuardedEngineState::Holding(ActiveHold {
                    action,
                    key,
                    start: now,
                    duration: self.hold_duration,
                });
            }
        }
    }

    /// Call on key release. If hold was completed, returns the confirmed action.
    /// Otherwise cancels active hold.
    pub fn on_release(&mut self, key: u8, now: Instant) -> Option<GuardedAction> {
        let (action, duration, start) = match &self.state {
            GuardedEngineState::Holding(hold) if hold.key == key => {
                (hold.action, hold.duration, hold.start)
            }
            _ => return None,
        };
        self.state = GuardedEngineState::Idle;
        let elapsed = now.duration_since(start);
        if elapsed >= duration {
            Some(action)
        } else {
            None
        }
    }

    /// Arm a 2-key confirmation prompt modal.
    pub fn arm_two_key(&mut self, action: GuardedAction, initiated_key: u8, confirm_key: u8, cancel_key: u8, now: Instant) {
        self.state = GuardedEngineState::TwoKeyPending {
            prompt: TwoKeyPrompt {
                action,
                initiated_key,
                confirm_key,
                cancel_key,
            },
            deadline: now + CONFIRMATION_TIMEOUT,
        };
    }

    /// Check key press during 2-key confirmation mode.
    pub fn handle_two_key_press(&mut self, key: u8, now: Instant) -> Option<GuardedAction> {
        if let GuardedEngineState::TwoKeyPending { prompt, deadline } = &self.state {
            if now > *deadline {
                self.state = GuardedEngineState::Idle;
                return None;
            }

            if key == prompt.confirm_key {
                let action = prompt.action;
                self.state = GuardedEngineState::Idle;
                return Some(action);
            } else if key == prompt.cancel_key || key == prompt.initiated_key {
                self.state = GuardedEngineState::Idle;
                return None;
            }
        }
        None
    }

    /// Periodic poll to check hold progress [0.0..1.0] and auto-trigger on 100% completion.
    pub fn poll_hold(&mut self, now: Instant) -> (Option<(GuardedAction, f32)>, Option<GuardedAction>) {
        match &self.state {
            GuardedEngineState::Holding(hold) => {
                let elapsed = now.duration_since(hold.start);
                let progress = (elapsed.as_secs_f32() / hold.duration.as_secs_f32()).clamp(0.0, 1.0);
                let action = hold.action;
                if progress >= 1.0 {
                    self.state = GuardedEngineState::Idle;
                    (Some((action, 1.0)), Some(action))
                } else {
                    (Some((action, progress)), None)
                }
            }
            GuardedEngineState::TwoKeyPending { deadline, .. } => {
                if now > *deadline {
                    self.state = GuardedEngineState::Idle;
                }
                (None, None)
            }
            GuardedEngineState::Idle => (None, None),
        }
    }

    pub fn cancel(&mut self) {
        self.state = GuardedEngineState::Idle;
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.state, GuardedEngineState::Idle)
    }

    pub fn state(&self) -> &GuardedEngineState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hold_progress_and_completion() {
        let mut engine = GuardedEngine::new(Duration::from_millis(300));
        let start = Instant::now();

        engine.on_press(GuardedAction::Shutdown, 31, start);
        assert!(engine.is_active());

        let t1 = start + Duration::from_millis(100);
        let (prog, triggered) = engine.poll_hold(t1);
        assert!(triggered.is_none());
        assert!(prog.is_some());
        let (action, p) = prog.unwrap();
        assert_eq!(action, GuardedAction::Shutdown);
        assert!((p - 0.333).abs() < 0.05);

        let t2 = start + Duration::from_millis(350);
        let (prog2, triggered2) = engine.poll_hold(t2);
        assert_eq!(triggered2, Some(GuardedAction::Shutdown));
        assert_eq!(prog2, Some((GuardedAction::Shutdown, 1.0)));
        assert!(!engine.is_active());
    }

    #[test]
    fn test_hold_release_early_cancels() {
        let mut engine = GuardedEngine::new(Duration::from_millis(500));
        let start = Instant::now();

        engine.on_press(GuardedAction::Reboot, 30, start);
        let release_time = start + Duration::from_millis(200);
        let action = engine.on_release(30, release_time);
        assert_eq!(action, None);
        assert!(!engine.is_active());
    }

    #[test]
    fn test_two_key_confirmation() {
        let mut engine = GuardedEngine::new(Duration::from_secs(3));
        let start = Instant::now();

        engine.arm_two_key(GuardedAction::Shutdown, 31, 24, 25, start);
        assert!(matches!(engine.state(), GuardedEngineState::TwoKeyPending { .. }));

        assert_eq!(engine.handle_two_key_press(10, start), None);
        assert!(engine.is_active());

        let res = engine.handle_two_key_press(24, start);
        assert_eq!(res, Some(GuardedAction::Shutdown));
        assert!(!engine.is_active());
    }
}
