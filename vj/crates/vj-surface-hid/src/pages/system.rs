//! SYSTEM page implementation.
//!
//! Utility page reached from PERFORMANCE (key 31 EXIT) and returned to via
//! key 31 here. Contains only telemetry that is actually kept live by
//! `UIEngine::refresh_telemetry()` (hostname, CPU temp) plus real broker
//! actions: renderer restart, state restore, guarded reboot/shutdown, and
//! ingest entry. A prior version of this page (and a separate DEGRADED
//! page) also displayed IP address, free space, uptime, and a throttle/
//! fault indicator that were never updated past their `Default` — i.e.
//! permanently fake regardless of the real system state. Those were
//! removed rather than left to silently lie on stage; see HANDOFF.md.

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

        // 2-key confirmation overlay takes priority over normal rendering,
        // same pattern proven in the pre-consolidation SYSTEM page.
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
            // Row 0 (0..7): real telemetry only
            0 | 1 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::CYAN);
                c.draw_text_centered_bold("HOSTNAME", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&ctx.telemetry.hostname, 0, 38, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("PORT 8080", 0, 68, 96, 1, Rgb::CYAN);
            }
            2 | 3 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 30, 40));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("CPU TEMP", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{:.1}C", ctx.telemetry.temperature_c), 0, 36, 96, 2, Rgb::YELLOW);
            }
            4 | 5 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(35, 25, 45));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::MID_GRAY);
                c.draw_text_centered_bold("SHOW", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let show_disp = if ctx.state.show_name.is_empty() { "DEFAULT" } else { &ctx.state.show_name };
                c.draw_text_centered_bold(show_disp, 0, 38, 96, 1, Rgb::MAGENTA);
            }

            // Row 1 (8..15): real actions
            8 | 9 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(25, 35, 55));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::BLUE);
                c.draw_text_centered_bold("INGEST", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("USB / LAN", 0, 45, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("OPEN >", 0, 70, 96, 1, Rgb::CYAN);
            }
            12 | 13 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 60, 100));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::CYAN);
                c.draw_text_centered_bold("RESTART", 0, 16, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("RENDERER", 0, 38, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("CAGE/KMS", 0, 66, 96, 1, Rgb::CYAN);
            }
            14 | 15 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(30, 80, 50));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::GREEN);
                c.draw_text_centered_bold("RESTORE", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("STATE", 0, 44, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("REPLAY >", 0, 68, 96, 1, Rgb::GREEN);
            }

            // Row 3 (24..31): guarded reboot / shutdown / exit
            24..=26 => {
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
            27..=29 => {
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
            31 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(40, 40, 55));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("EXIT", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("PERF >", 0, 55, 96, 1, Rgb::CYAN);
            }

            _ => {}
        }

        c
    }

    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, guarded: &mut GuardedEngine) -> PageAction {
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
            12 | 13 => {
                broker.restart_renderer();
                PageAction::None
            }
            14 | 15 => {
                broker.replay_state();
                PageAction::None
            }
            24..=26 => {
                guarded.on_press(GuardedAction::Reboot, key, ctx.now);
                guarded.arm_two_key(GuardedAction::Reboot, key, 0, 31, ctx.now);
                PageAction::None
            }
            27..=29 => {
                guarded.on_press(GuardedAction::Shutdown, key, ctx.now);
                guarded.arm_two_key(GuardedAction::Shutdown, key, 0, 31, ctx.now);
                PageAction::None
            }
            31 => PageAction::SwitchPage(PageId::Performance),
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
    fn test_system_page_telemetry_and_guarded_shutdown() {
        let mut page = SystemPage::new();
        let mut ctx = PageContext::default();
        ctx.telemetry.hostname = "stage-vj.local".to_string();

        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        let canvas = page.render_key(0, &ctx, &guarded);
        let jpeg = canvas.encode_jpeg();
        assert!(!jpeg.is_empty());

        // Press shutdown key: arms 2-key confirm and hold.
        page.handle_key_press(27, &mut ctx, &client, &mut guarded);
        assert!(guarded.is_active());

        // Press confirm key (key 0).
        let action = page.handle_key_press(0, &mut ctx, &client, &mut guarded);
        assert_eq!(action, PageAction::TriggerGuarded(GuardedAction::Shutdown));

        std::thread::sleep(std::time::Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"system.shutdown\"")));
    }

    #[test]
    fn test_system_page_restart_renderer_and_restore_state() {
        let mut page = SystemPage::new();
        let mut ctx = PageContext::default();
        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        page.handle_key_press(12, &mut ctx, &client, &mut guarded);
        page.handle_key_press(14, &mut ctx, &client, &mut guarded);

        std::thread::sleep(std::time::Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"renderer.restart\"")));
    }

    #[test]
    fn test_system_page_ingest_navigation() {
        let mut page = SystemPage::new();
        let mut ctx = PageContext::default();
        let (client, _mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        let action = page.handle_key_press(8, &mut ctx, &client, &mut guarded);
        assert_eq!(action, PageAction::SwitchPage(PageId::Ingest));
    }
}
