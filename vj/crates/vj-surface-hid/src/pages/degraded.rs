//! DEGRADED page implementation.
//!
//! Fault diagnostic and recovery view displaying error codes,
//! affected subsystems, and one-touch/guarded recovery triggers.

use crate::broker_client::BrokerClient;
use crate::graphics::{Canvas, Rgb};
use crate::guarded::{GuardedAction, GuardedEngine, GuardedEngineState};
use crate::pages::{Page, PageAction, PageContext, PageId};

pub struct DegradedPage;

impl DegradedPage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DegradedPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for DegradedPage {
    fn page_id(&self) -> PageId {
        PageId::Degraded
    }

    fn render_key(&self, key: u8, ctx: &PageContext, guarded: &GuardedEngine) -> Canvas {
        let mut c = Canvas::new(Rgb::DARK_RED);

        let hold_progress = match guarded.state() {
            GuardedEngineState::Holding(hold) if hold.key == key => {
                let elapsed = ctx.now.duration_since(hold.start);
                Some((hold.action, (elapsed.as_secs_f32() / hold.duration.as_secs_f32()).clamp(0.0, 1.0)))
            }
            _ => None,
        };

        match key {
            // Row 0 (0..7): Fault Code & Severity
            0 | 1 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_RED);
                c.draw_rect_border(3, 3, 90, 90, 3, Rgb::RED);
                c.draw_text_centered_bold("FAULT", 0, 14, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold(&ctx.fault.code, 0, 44, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("ACTIVE", 0, 68, 96, 1, Rgb::RED);
            }
            2 | 3 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(60, 15, 15));
                c.draw_text_centered_bold("SUBSYSTEM", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&ctx.fault.subsystem, 0, 38, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold(if ctx.fault.renderer_down { "DOWN" } else { "DEGRADED" }, 0, 65, 96, 1, Rgb::RED);
            }
            4..=7 => {
                c.fill_rect(0, 0, 96, 96, Rgb::new(50, 10, 10));
                c.draw_text_centered_bold("DIAGNOSTIC", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&ctx.fault.message, 0, 38, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(&ctx.fault.recovery_hint, 0, 65, 96, 1, Rgb::CYAN);
            }

            // Row 1 (8..15): Live Telemetry during fault
            8 | 9 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_GRAY);
                c.draw_text_centered_bold("RENDERER", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(if ctx.fault.renderer_down { "STOPPED" } else { "ALIVE" }, 0, 38, 96, 1, if ctx.fault.renderer_down { Rgb::RED } else { Rgb::GREEN });
                c.draw_text_centered_bold("CAGE/KMS", 0, 65, 96, 1, Rgb::WHITE);
            }
            10 | 11 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_GRAY);
                c.draw_text_centered_bold("TEMP / CPU", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{:.0}C", ctx.telemetry.temperature_c), 0, 38, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold(if ctx.telemetry.throttled { "THROTTLED" } else { "NORMAL" }, 0, 68, 96, 1, if ctx.telemetry.throttled { Rgb::RED } else { Rgb::GREEN });
            }
            12 | 13 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_GRAY);
                c.draw_text_centered_bold("STORAGE", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let free_gb = (ctx.telemetry.free_space_bytes as f32) / 1_000_000_000.0;
                c.draw_text_centered_bold(&format!("{free_gb:.1}G"), 0, 38, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("SPACE", 0, 68, 96, 1, Rgb::CYAN);
            }
            14 | 15 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_GRAY);
                c.draw_text_centered_bold("BROKER", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("SOCKET", 0, 38, 96, 1, Rgb::CYAN);
                c.draw_text_centered_bold("RETRYING", 0, 65, 96, 1, Rgb::YELLOW);
            }

            // Row 2 (16..23): Recovery Action Buttons
            16..=19 => {
                // Restart Renderer Action
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 70, 120));
                c.draw_rect_border(3, 3, 90, 90, 2, Rgb::CYAN);
                c.draw_text_centered_bold("RESTART", 0, 14, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("RENDERER", 0, 44, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("TAP TO RUN", 0, 68, 96, 1, Rgb::CYAN);
            }
            20..=23 => {
                // Replay State & Reload Show
                c.fill_rect(0, 0, 96, 96, Rgb::new(30, 80, 50));
                c.draw_rect_border(3, 3, 90, 90, 2, Rgb::GREEN);
                c.draw_text_centered_bold("RESTORE", 0, 14, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("STATE", 0, 44, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("REPLAY >", 0, 68, 96, 1, Rgb::GREEN);
            }

            // Row 3 (24..31): Guarded Reboot & Exit
            24..=27 => {
                // Guarded System Reboot
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_RED);
                c.draw_rect_border(3, 3, 90, 90, 3, Rgb::RED);
                c.draw_text_centered_bold("FORCE", 0, 12, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("REBOOT", 0, 32, 96, 2, Rgb::YELLOW);
                if let Some((GuardedAction::Reboot, p)) = hold_progress {
                    c.draw_hold_ring(48, 68, 16, p, Rgb::YELLOW);
                } else {
                    c.draw_text_centered_bold("HOLD 3S", 0, 68, 96, 1, Rgb::WHITE);
                }
            }
            28..=30 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_GRAY);
                c.draw_text_centered_bold("DIAG", 0, 22, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("LOGGED", 0, 50, 96, 1, Rgb::GREEN);
            }
            31 => {
                // Return to Ready
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
        match key {
            16..=19 => {
                // Restart renderer immediately and replay state
                broker.restart_renderer();
                broker.replay_state();
                PageAction::None
            }
            20..=23 => {
                broker.replay_state();
                PageAction::None
            }
            24..=27 => {
                guarded.on_press(GuardedAction::Reboot, key, ctx.now);
                PageAction::None
            }
            31 => {
                PageAction::SwitchPage(PageId::Ready)
            }
            _ => PageAction::None,
        }
    }

    fn handle_key_release(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, guarded: &mut GuardedEngine) -> PageAction {
        if let Some(action) = guarded.on_release(key, ctx.now) {
            if action == GuardedAction::Reboot {
                broker.send_command("system.reboot", serde_json::json!({}));
                PageAction::TriggerGuarded(action)
            } else {
                PageAction::None
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
    fn test_degraded_page_restart_renderer_action() {
        let mut page = DegradedPage::new();
        let mut ctx = PageContext::default();
        ctx.fault.code = "ERR-RENDERER-CRASH".to_string();
        ctx.fault.renderer_down = true;

        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        let action = page.handle_key_press(16, &mut ctx, &client, &mut guarded);
        assert_eq!(action, PageAction::None);

        let canvas = page.render_key(0, &ctx, &guarded);
        let jpeg = canvas.encode_jpeg();
        assert!(!jpeg.is_empty());

        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"renderer.restart\"")));
    }
}
