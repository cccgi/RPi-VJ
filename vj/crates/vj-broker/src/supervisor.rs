use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const HEARTBEAT_WARN_DURATION: Duration = Duration::from_secs(3);
pub const HEARTBEAT_RESTART_DURATION: Duration = Duration::from_secs(6);
pub const RESTART_WINDOW_DURATION: Duration = Duration::from_secs(60);
pub const MAX_RESTARTS_BEFORE_REBOOT: usize = 3;
pub const ANTI_REBOOT_LOOP_WINDOW_SECS: u64 = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationLevel {
    Normal,
    DegradedWarning,
    RestartRenderer,
    ControlledReboot,
    DurableDegraded,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SupervisorPersistentState {
    pub last_boot_reboot_attempted: bool,
    pub last_reboot_epoch_secs: u64,
    pub durable_degraded: bool,
    pub incident_count: u64,
}

impl Default for SupervisorPersistentState {
    fn default() -> Self {
        Self {
            last_boot_reboot_attempted: false,
            last_reboot_epoch_secs: 0,
            durable_degraded: false,
            incident_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SupervisorAction {
    None,
    LogWarning { reason: String },
    RestartRenderer { count_in_window: usize },
    ControlledReboot { reason: String },
    EnterDurableDegraded { reason: String },
}

pub struct Supervisor {
    level: EscalationLevel,
    first_heartbeat_seen: bool,
    last_heartbeat_instant: Option<Instant>,
    restart_instants: Vec<Instant>,
    persistent_state: SupervisorPersistentState,
    state_file_path: Option<PathBuf>,
    boot_ledger_path: Option<PathBuf>,
    started_instant: Instant,
    last_action_instant: Option<Instant>,
}

impl Supervisor {
    pub fn new(
        state_file_path: Option<PathBuf>,
        boot_ledger_path: Option<PathBuf>,
        now: Instant,
        _current_epoch_secs: u64,
    ) -> Self {
        let mut persistent = SupervisorPersistentState::default();

        if let Some(ref path) = state_file_path {
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(loaded) = serde_json::from_str::<SupervisorPersistentState>(&content) {
                        persistent = loaded;
                    }
                }
            }
        }

        let initial_level = if persistent.durable_degraded {
            EscalationLevel::DurableDegraded
        } else {
            EscalationLevel::Normal
        };

        Self {
            level: initial_level,
            first_heartbeat_seen: false,
            last_heartbeat_instant: None,
            restart_instants: Vec::new(),
            persistent_state: persistent,
            state_file_path,
            boot_ledger_path,
            started_instant: now,
            last_action_instant: None,
        }
    }

    pub fn record_heartbeat(&mut self, now: Instant) {
        self.first_heartbeat_seen = true;
        self.last_heartbeat_instant = Some(now);

        // Recover to normal if not in durable degraded
        if self.level != EscalationLevel::DurableDegraded {
            self.level = EscalationLevel::Normal;
        }
    }

    pub fn get_level(&self) -> EscalationLevel {
        self.level
    }

    pub fn is_durable_degraded(&self) -> bool {
        self.level == EscalationLevel::DurableDegraded
    }

    pub fn check_health(&mut self, now: Instant, current_epoch_secs: u64) -> SupervisorAction {
        if self.level == EscalationLevel::DurableDegraded {
            return SupervisorAction::None;
        }

        // Determine baseline reference instant for heartbeat check
        let last_hb = self.last_heartbeat_instant.unwrap_or(self.started_instant);
        let elapsed_since_hb = now.saturating_duration_since(last_hb);

        // Prune old restart timestamps outside sliding window
        self.restart_instants.retain(|&t| now.saturating_duration_since(t) <= RESTART_WINDOW_DURATION);

        // Escalation check
        if elapsed_since_hb >= HEARTBEAT_RESTART_DURATION {
            // Heartbeat missing > 6s
            if self.restart_instants.len() >= MAX_RESTARTS_BEFORE_REBOOT {
                // We have restarted >= 3 times in the last 60s!
                // Check if we already rebooted recently (anti-reboot loop protection)
                let time_since_last_reboot = current_epoch_secs.saturating_sub(self.persistent_state.last_reboot_epoch_secs);

                if self.persistent_state.last_boot_reboot_attempted && time_since_last_reboot < ANTI_REBOOT_LOOP_WINDOW_SECS {
                    // Anti-reboot trigger: already rebooted within window, enters durable degraded!
                    self.level = EscalationLevel::DurableDegraded;
                    self.persistent_state.durable_degraded = true;
                    let _ = self.save_persistent_state();
                    self.log_to_ledger("DURABLE_DEGRADED", "Renderer failed repeatedly post-reboot; suppressing further reboots");
                    return SupervisorAction::EnterDurableDegraded {
                        reason: "Exceeded max restarts post-reboot; durable degraded mode engaged".to_string(),
                    };
                } else {
                    // Trigger one controlled reboot
                    self.level = EscalationLevel::ControlledReboot;
                    self.persistent_state.last_boot_reboot_attempted = true;
                    self.persistent_state.last_reboot_epoch_secs = current_epoch_secs;
                    self.persistent_state.incident_count += 1;
                    let _ = self.save_persistent_state();
                    self.log_to_ledger("CONTROLLED_REBOOT", "Renderer failed 3 times in 60s; triggering single controlled reboot");
                    return SupervisorAction::ControlledReboot {
                        reason: "Renderer failed 3 times in 60s".to_string(),
                    };
                }
            } else {
                // Rate-limit restart action to at most once every 5 seconds
                if let Some(last_act) = self.last_action_instant {
                    if now.saturating_duration_since(last_act) < Duration::from_secs(5) {
                        return SupervisorAction::None;
                    }
                }

                self.level = EscalationLevel::RestartRenderer;
                self.restart_instants.push(now);
                self.last_action_instant = Some(now);
                self.log_to_ledger("RENDERER_RESTART", "Heartbeat timeout > 6s; restarting renderer service");
                return SupervisorAction::RestartRenderer {
                    count_in_window: self.restart_instants.len(),
                };
            }
        } else if elapsed_since_hb >= HEARTBEAT_WARN_DURATION {
            if self.level == EscalationLevel::Normal {
                self.level = EscalationLevel::DegradedWarning;
                return SupervisorAction::LogWarning {
                    reason: format!("Heartbeat delayed: {:.1}s", elapsed_since_hb.as_secs_f32()),
                };
            }
        }

        SupervisorAction::None
    }

    fn save_persistent_state(&self) -> std::io::Result<()> {
        if let Some(ref path) = self.state_file_path {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let tmp_path = path.with_extension("tmp");
            let json = serde_json::to_string_pretty(&self.persistent_state)?;
            {
                let mut f = File::create(&tmp_path)?;
                f.write_all(json.as_bytes())?;
                f.sync_all()?;
            }
            fs::rename(&tmp_path, path)?;
        }
        Ok(())
    }

    fn log_to_ledger(&self, event: &str, detail: &str) {
        if let Some(ref path) = self.boot_ledger_path {
            let line = serde_json::json!({
                "schema_version": "v3.1",
                "boot_id": "supervisor",
                "boot_count": 0,
                "timestamp_utc": "",
                "event": event,
                "phase": "supervisor",
                "status": "escalation",
                "error_code": 0,
                "details": detail
            });
            if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{}", line);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervisor_escalation_ladder() {
        let t0 = Instant::now();
        let epoch = 1700000000;
        let mut sup = Supervisor::new(None, None, t0, epoch);

        // 1. Initial healthy state
        assert_eq!(sup.get_level(), EscalationLevel::Normal);

        // 2. Health check at t0+2s: healthy
        let act = sup.check_health(t0 + Duration::from_secs(2), epoch);
        assert_eq!(act, SupervisorAction::None);

        // 3. Health check at t0+4s: Warning
        let act = sup.check_health(t0 + Duration::from_secs(4), epoch);
        assert!(matches!(act, SupervisorAction::LogWarning { .. }));
        assert_eq!(sup.get_level(), EscalationLevel::DegradedWarning);

        // 4. Heartbeat arrives at t0+5s: recovers to normal
        sup.record_heartbeat(t0 + Duration::from_secs(5));
        assert_eq!(sup.get_level(), EscalationLevel::Normal);

        // 5. Missing heartbeats up to t0+12s: triggers Restart 1
        let act = sup.check_health(t0 + Duration::from_secs(12), epoch);
        assert!(matches!(act, SupervisorAction::RestartRenderer { count_in_window: 1 }));

        // 6. Next restart 2 at t0+18s
        let act = sup.check_health(t0 + Duration::from_secs(18), epoch);
        assert!(matches!(act, SupervisorAction::RestartRenderer { count_in_window: 2 }));

        // 7. Next restart 3 at t0+24s
        let act = sup.check_health(t0 + Duration::from_secs(24), epoch);
        assert!(matches!(act, SupervisorAction::RestartRenderer { count_in_window: 3 }));

        // 8. Next failure at t0+30s: triggers Controlled Reboot (reboot attempt 1)
        let act = sup.check_health(t0 + Duration::from_secs(30), epoch);
        assert!(matches!(act, SupervisorAction::ControlledReboot { .. }));
        assert_eq!(sup.get_level(), EscalationLevel::ControlledReboot);
    }

    #[test]
    fn test_anti_reboot_loop_protection() {
        let t0 = Instant::now();
        let epoch = 1700000000;
        let mut sup = Supervisor::new(None, None, t0, epoch);
        sup.persistent_state.last_boot_reboot_attempted = true;
        sup.persistent_state.last_reboot_epoch_secs = epoch - 30; // Rebooted 30s ago!

        // Force 3 restarts in window
        sup.restart_instants.push(t0 + Duration::from_secs(1));
        sup.restart_instants.push(t0 + Duration::from_secs(2));
        sup.restart_instants.push(t0 + Duration::from_secs(3));

        // At t0+10s (elapsed since heartbeat > 6s), should NOT reboot again, must engage DurableDegraded!
        let act = sup.check_health(t0 + Duration::from_secs(10), epoch);
        assert!(matches!(act, SupervisorAction::EnterDurableDegraded { .. }));
        assert_eq!(sup.get_level(), EscalationLevel::DurableDegraded);
        assert!(sup.is_durable_degraded());
    }
}
