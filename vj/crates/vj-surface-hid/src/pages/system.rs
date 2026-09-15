//! SYSTEM page implementation.
//!
//! Fixed system page containing health, IP/hostname, free space,
//! temperature/throttle state, ingest entry, performance arm/disarm,
//! renderer restart, and guarded shutdown / reboot (2-key or 3-second hold confirmation).

use crate::broker_client::BrokerClient;
use crate::graphics::{Canvas, Rgb};
use crate::guarded::{GuardedAction, GuardedEngine, GuardedEngineState};
use crate::pages::{Page, PageAction, PageContext, PageId};

pub struct SystemPage;

impl SystemPage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for SystemPage {
    fn page_id(&self) -> PageId {
        PageId::System
    }

    fn render_key(&self, key: u8, ctx: &PageContext, guarded: &GuardedEngine) -> Canvas {
        let mut c = Canvas::new(Rgb::DARK_GRAY);

        let hold_progress = match guarded.state() {
            GuardedEngineState::Holding(hold) if hold.key == key => {
                let elapsed = ctx.now.duration_since(hold.start);
                Some((hold.action, (elapsed.as_secs_f32() / hold.duration.as_secs_f32()).clamp(0.0, 1.0)))
            }
            _ => None,
        };

        // If in 2-key confirmation mode:
        if let GuardedEngineState::TwoKeyPending { prompt, .. } = guarded.state() {
            if key == prompt.confirm_key {
                c.fill_rect(0, 0, 96, 96, Rgb::new(140, 20, 20));
                c.draw_rect_border(3, 3, 90, 90, 3, Rgb::YELLOW);
                c.draw_text_centered_bold("CONFIRM", 0, 16, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(prompt.action.label(), 0, 42, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("KEY 2 >", 0, 68, 96, 1, Rgb::WHITE);
                return c;
            } else if key == prompt.cancel_key {
                c.fill_rect(0, 0, 96, 96, Rgb::new(30, 40, 60));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::CYAN);
                c.draw_text_centered_bold("CANCEL", 0, 22, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("ABORT", 0, 50, 96, 1, Rgb::LIGHT_GRAY);
                return c;
            }
        }

        match key {
            // Row 0 (0..7): Telemetry & Identity Display
            0 | 1 => {
                // IP Address
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::CYAN);
                c.draw_text_centered_bold("IP ADDRESS", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let ip_disp = if ctx.telemetry.ip_address.is_empty() { "OFFLINE" } else { &ctx.telemetry.ip_address };
                c.draw_text_centered_bold(ip_disp, 0, 38, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("PORT 8080", 0, 68, 96, 1, Rgb::CYAN);
            }
            2 | 3 => {
                // Hostname
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::CYAN);
                c.draw_text_centered_bold("HOSTNAME", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let host_disp = if ctx.telemetry.hostname.is_empty() { "rpi-vj.local" } else { &ctx.telemetry.hostname };
                c.draw_text_centered_bold(host_disp, 0, 38, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("mDNS ACTIVE", 0, 68, 96, 1, Rgb::GREEN);
            }
            4 | 5 => {
                // Storage Free Space
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 30, 40));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("STORAGE", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let free_gb = (ctx.telemetry.free_space_bytes as f32) / 1_000_000_000.0;
                c.draw_text_centered_bold(&format!("{free_gb:.1}G FREE"), 0, 38, 96, 1, Rgb::WHITE);
                let p = if ctx.telemetry.total_space_bytes > 0 {
                    (ctx.telemetry.free_space_bytes as f32 / ctx.telemetry.total_space_bytes as f32).clamp(0.0, 1.0)
                } else {
                    0.5
                };
                c.draw_progress_bar(8, 68, 80, 14, p, Rgb::GREEN, Rgb::DARK_GRAY);
            }
            6 | 7 => {
                // Temperature & Throttle
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 30, 40));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("CPU TEMP", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{:.1}C", ctx.telemetry.temperature_c), 0, 36, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold(if ctx.telemetry.throttled { "THROTTLED" } else { "THROTTLE: NO" }, 0, 68, 96, 1, if ctx.telemetry.throttled { Rgb::RED } else { Rgb::GREEN });
            }

            // Row 1 (8..15): Subsystem Controls & Navigation
            8 | 9 => {
                // Ingest Entry
                c.fill_rect(0, 0, 96, 96, Rgb::new(25, 35, 55));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::BLUE);
                c.draw_text_centered_bold("INGEST", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("USB / LAN", 0, 45, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("OPEN >", 0, 70, 96, 1, Rgb::CYAN);
            }
            10 | 11 => {
                // Performance Arm / Disarm
                c.fill_rect(0, 0, 96, 96, Rgb::new(15, 70, 35));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::GREEN);
                c.draw_text_centered_bold("PERF", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("SHOW CTRL", 0, 45, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("ARM >", 0, 70, 96, 1, Rgb::GREEN);
            }
            12 | 13 => {
                // Renderer Restart Action
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 60, 100));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::CYAN);
                c.draw_text_centered_bold("RESTART", 0, 16, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("RENDERER", 0, 38, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("CAGE/KMS", 0, 66, 96, 1, Rgb::CYAN);
            }
            14 | 15 => {
                // Show Generation Info
                c.fill_rect(0, 0, 96, 96, Rgb::new(35, 25, 45));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::MID_GRAY);
                c.draw_text_centered_bold("CURRENT", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let show_disp = if ctx.state.show_name.is_empty() { "DEFAULT" } else { &ctx.state.show_name };
                c.draw_text_centered_bold(show_disp, 0, 38, 96, 1, Rgb::MAGENTA);
                c.draw_text_centered_bold("IMMUTABLE", 0, 68, 96, 1, Rgb::GREEN);
            }

            // Row 2 (16..23): Maintenance & Brightness
            16 | 17 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(28, 28, 36));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::MID_GRAY);
                c.draw_text_centered_bold("UPTIME", 0, 18, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{}m", ctx.telemetry.uptime_secs / 60), 0, 42, 96, 2, Rgb::WHITE);
            }
            18 | 19 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(28, 28, 36));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::MID_GRAY);
                c.draw_text_centered_bold("BOOT", 0, 16, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("LEDGER", 0, 38, 96, 1, Rgb::CYAN);
                c.draw_text_centered_bold("ACTIVE", 0, 66, 96, 1, Rgb::GREEN);
            }
            20..=23 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(30, 30, 40));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::MID_GRAY);
                c.draw_text_centered_bold("APPLIANCE", 0, 16, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("VERSION 3", 0, 40, 96, 1, Rgb::CYAN);
                c.draw_text_centered_bold("STANDALONE", 0, 66, 96, 1, Rgb::WHITE);
            }

            // Row 3 (24..31): Guarded Shutdown, Guarded Reboot & Exit
            24 | 25 => {
                // Guarded Reboot (Hold 3s or 2-Key)
                c.fill_rect(0, 0, 96, 96, Rgb::new(90, 40, 10));
                c.draw_rect_border(3, 3, 90, 90, 2, Rgb::AMBER);
                c.draw_text_centered_bold("REBOOT", 0, 14, 96, 2, Rgb::YELLOW);
                if let Some((GuardedAction::Reboot, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::YELLOW);
                } else {
                    c.draw_text_centered_bold("HOLD 3S /", 0, 44, 96, 1, Rgb::WHITE);
                    c.draw_text_centered_bold("TAP 2-KEY", 0, 68, 96, 1, Rgb::LIGHT_GRAY);
                }
            }
            26 | 27 => {
                // Guarded Shutdown (Hold 3s or 2-Key)
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_RED);
                c.draw_rect_border(3, 3, 90, 90, 3, Rgb::RED);
                c.draw_text_centered_bold("SHUTDOWN", 0, 14, 96, 1, Rgb::WHITE);
                if let Some((GuardedAction::Shutdown, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::RED);
                } else {
                    c.draw_text_centered_bold("HOLD 3S /", 0, 42, 96, 1, Rgb::YELLOW);
                    c.draw_text_centered_bold("TAP 2-KEY", 0, 68, 96, 1, Rgb::LIGHT_GRAY);
                }
            }
            28..=30 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(28, 28, 36));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::MID_GRAY);
                c.draw_text_centered_bold("SYSTEM", 0, 22, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("ARMED", 0, 50, 96, 1, Rgb::GREEN);
            }
            31 => {
                // Exit / Return to Ready
                c.fill_rect(0, 0, 96, 96, Rgb::new(40, 40, 55));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("EXIT", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("READY >", 0, 55, 96, 1, Rgb::CYAN);
            }

            _ => {}
        }

        c
    }

    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, guarded: &mut GuardedEngine) -> PageAction {
        // If two-key mode is pending:
        if let Some(action) = guarded.handle_two_key_press(key, ctx.now) {
            match action {
                GuardedAction::Shutdown => {
                    broker.send_command("system.shutdown", serde_json::json!({}));
                    return PageAction::TriggerGuarded(action);
                }
                GuardedAction::Reboot => {
                    broker.send_command("system.reboot", serde_json::json!({}));
                    return PageAction::TriggerGuarded(action);
                }
                _ => return PageAction::None,
            }
        }

        match key {
            8 | 9 => PageAction::SwitchPage(PageId::Ingest),
            10 | 11 => PageAction::SwitchPage(PageId::Performance),
            12 | 13 => {
                broker.restart_renderer();
                broker.replay_state();
                PageAction::None
            }
            24 | 25 => {
                // Start hold or arm 2-key
                guarded.on_press(GuardedAction::Reboot, key, ctx.now);
                // Also arm 2-key confirmation: key 24 initiated, confirm on 0, cancel on 31
                guarded.arm_two_key(GuardedAction::Reboot, key, 0, 31, ctx.now);
                PageAction::None
            }
            26 | 27 => {
                // Start hold or arm 2-key
                guarded.on_press(GuardedAction::Shutdown, key, ctx.now);
                // Arm 2-key confirmation: key 26 initiated, confirm on 0, cancel on 31
                guarded.arm_two_key(GuardedAction::Shutdown, key, 0, 31, ctx.now);
                PageAction::None
            }
            31 => PageAction::SwitchPage(PageId::Ready),
            _ => PageAction::None,
        }
    }

    fn handle_key_release(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, guarded: &mut GuardedEngine) -> PageAction {
        if let Some(action) = guarded.on_release(key, ctx.now) {
            match action {
                GuardedAction::Shutdown => {
                    broker.send_command("system.shutdown", serde_json::json!({}));
                    PageAction::TriggerGuarded(action)
                }
                GuardedAction::Reboot => {
                    broker.send_command("system.reboot", serde_json::json!({}));
                    PageAction::TriggerGuarded(action)
                }
                _ => PageAction::None,
            }
        } else {
            PageAction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_page_telemetry_rendering_and_guarded_shutdown() {
        let mut page = SystemPage::new();
        let mut ctx = PageContext::default();
        ctx.telemetry.ip_address = "10.0.0.55".to_string();
        ctx.telemetry.hostname = "stage-vj.local".to_string();

        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        let canvas = page.render_key(0, &ctx, &guarded);
        let jpeg = canvas.encode_jpeg();
        assert!(!jpeg.is_empty());

        // Press shutdown key: arms 2-key confirm and hold
        page.handle_key_press(26, &mut ctx, &client, &mut guarded);
        assert!(guarded.is_active());

        // Press confirm key (key 0)
        let action = page.handle_key_press(0, &mut ctx, &client, &mut guarded);
        assert_eq!(action, PageAction::TriggerGuarded(GuardedAction::Shutdown));

        std::thread::sleep(std::time::Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"system.shutdown\"")));
    }
}
