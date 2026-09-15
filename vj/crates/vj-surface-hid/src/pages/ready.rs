//! READY page implementation.
//!
//! Standby / system overview, show information, bank summary, and navigation.

use crate::broker_client::BrokerClient;
use crate::graphics::{Canvas, Rgb};
use crate::guarded::GuardedEngine;
use crate::pages::{Page, PageAction, PageContext, PageId};

pub struct ReadyPage;

impl ReadyPage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadyPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for ReadyPage {
    fn page_id(&self) -> PageId {
        PageId::Ready
    }

    fn render_key(&self, key: u8, ctx: &PageContext, _guarded: &GuardedEngine) -> Canvas {
        let mut c = Canvas::new(Rgb::DARK_GRAY);

        match key {
            // Row 0 (0..7): Banks 0..7 overview
            0..=7 => {
                let bank_idx = key as u32;
                let is_active = ctx.state.active_bank == bank_idx;
                let bg = if is_active {
                    Rgb::new(0, 90, 140)
                } else {
                    Rgb::new(18, 22, 32)
                };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, if is_active { 3 } else { 1 }, if is_active { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("BANK", 0, 12, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{bank_idx}"), 0, 32, 96, 3, if is_active { Rgb::WHITE } else { Rgb::CYAN });
                c.draw_text_centered_bold(if is_active { "ACTIVE" } else { "STANDBY" }, 0, 70, 96, 1, if is_active { Rgb::GREEN } else { Rgb::MID_GRAY });
            }

            // Row 1 (8..15): Status details
            8 | 9 => {
                // Show Name
                c.fill_rect(0, 0, 96, 96, Rgb::new(35, 20, 50));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::PURPLE);
                c.draw_text_centered_bold("SHOW", 0, 12, 96, 1, Rgb::LIGHT_GRAY);
                let show_display = if ctx.state.show_name.is_empty() { "DEFAULT" } else { &ctx.state.show_name };
                c.draw_text_centered_bold(show_display, 0, 38, 96, 1, Rgb::MAGENTA);
                c.draw_text_centered_bold("GEN V3", 0, 68, 96, 1, Rgb::WHITE);
            }
            10 => {
                // Master Opacity
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 28, 42));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::MID_GRAY);
                c.draw_text_centered_bold("MASTER", 0, 12, 96, 1, Rgb::LIGHT_GRAY);
                let pct = (ctx.state.master_opacity * 100.0).round() as u32;
                c.draw_text_centered_bold(&format!("{pct}%"), 0, 35, 96, 2, Rgb::WHITE);
                c.draw_progress_bar(8, 70, 80, 14, ctx.state.master_opacity, Rgb::CYAN, Rgb::DARK_GRAY);
            }
            11 => {
                // Tempo BPM
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 28, 42));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::MID_GRAY);
                c.draw_text_centered_bold("TEMPO", 0, 12, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{:.0}", ctx.state.tempo_bpm), 0, 35, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold("BPM", 0, 70, 96, 1, Rgb::LIGHT_GRAY);
            }
            12 => {
                // Blackout State
                let bo = ctx.state.blackout;
                let bg = if bo { Rgb::DARK_RED } else { Rgb::new(15, 35, 20) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, if bo { Rgb::RED } else { Rgb::GREEN });
                c.draw_text_centered_bold("BLACKOUT", 0, 16, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(if bo { "ON" } else { "OFF" }, 0, 42, 96, 2, if bo { Rgb::RED } else { Rgb::GREEN });
                c.draw_text_centered_bold(if bo { "ENGAGED" } else { "PASSTHRU" }, 0, 70, 96, 1, Rgb::LIGHT_GRAY);
            }
            13 | 14 => {
                // IP & Hostname
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::CYAN);
                c.draw_text_centered_bold("NETWORK", 0, 12, 96, 1, Rgb::LIGHT_GRAY);
                let ip_disp = if ctx.telemetry.ip_address.is_empty() { "OFFLINE" } else { &ctx.telemetry.ip_address };
                c.draw_text_centered_bold(ip_disp, 0, 38, 96, 1, Rgb::WHITE);
                let host_disp = if ctx.telemetry.hostname.is_empty() { "rpi-vj.local" } else { &ctx.telemetry.hostname };
                c.draw_text_centered_bold(host_disp, 0, 65, 96, 1, Rgb::CYAN);
            }
            15 => {
                // Fault / Health Indicator
                let is_degraded = ctx.fault.code != "OK" || ctx.fault.renderer_down;
                let bg = if is_degraded { Rgb::DARK_RED } else { Rgb::new(10, 50, 20) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, if is_degraded { Rgb::RED } else { Rgb::GREEN });
                c.draw_text_centered_bold("HEALTH", 0, 12, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(if is_degraded { "FAULT" } else { "OK" }, 0, 38, 96, 2, if is_degraded { Rgb::YELLOW } else { Rgb::GREEN });
                c.draw_text_centered_bold(if is_degraded { "DIAG >" } else { "100%" }, 0, 68, 96, 1, Rgb::WHITE);
            }

            // Row 2 (16..23): Launchers
            16..=19 => {
                // ARM PERFORMANCE MODE
                c.fill_rect(0, 0, 96, 96, Rgb::new(10, 70, 35));
                c.draw_rect_border(3, 3, 90, 90, 3, Rgb::GREEN);
                c.draw_text_centered_bold("PERF", 0, 14, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("MODE", 0, 42, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold("ARM >", 0, 70, 96, 1, Rgb::WHITE);
            }
            20 | 21 => {
                // INGEST
                let usb = ctx.ingest.usb_inserted;
                let bg = if usb { Rgb::new(80, 50, 10) } else { Rgb::new(25, 30, 45) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, if usb { Rgb::AMBER } else { Rgb::BLUE });
                c.draw_text_centered_bold("INGEST", 0, 14, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold(if usb { "USB DETECT" } else { "MEDIA" }, 0, 45, 96, 1, if usb { Rgb::YELLOW } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered_bold("OPEN >", 0, 70, 96, 1, Rgb::CYAN);
            }
            22 | 23 => {
                // SYSTEM
                c.fill_rect(0, 0, 96, 96, Rgb::new(35, 35, 45));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("SYSTEM", 0, 14, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("SETTINGS", 0, 45, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("VIEW >", 0, 70, 96, 1, Rgb::CYAN);
            }

            // Row 3 (24..31): Navigation & Fast controls
            24 => {
                // Bank -
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::CYAN);
                c.draw_text_centered_bold("BANK", 0, 20, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("PREV", 0, 45, 96, 2, Rgb::WHITE);
            }
            25 => {
                // Bank +
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::CYAN);
                c.draw_text_centered_bold("BANK", 0, 20, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("NEXT", 0, 45, 96, 2, Rgb::WHITE);
            }
            26 => {
                // BPM -
                c.fill_rect(0, 0, 96, 96, Rgb::new(28, 28, 36));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("BPM", 0, 20, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("- 1", 0, 45, 96, 2, Rgb::YELLOW);
            }
            27 => {
                // BPM +
                c.fill_rect(0, 0, 96, 96, Rgb::new(28, 28, 36));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("BPM", 0, 20, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("+ 1", 0, 45, 96, 2, Rgb::YELLOW);
            }
            28 => {
                // Blackout toggle
                let bo = ctx.state.blackout;
                c.fill_rect(0, 0, 96, 96, if bo { Rgb::DARK_RED } else { Rgb::new(28, 28, 36) });
                c.draw_rect_border(2, 2, 92, 92, 2, if bo { Rgb::RED } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered_bold("BLACKOUT", 0, 18, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(if bo { "DARK" } else { "LIVE" }, 0, 45, 96, 2, if bo { Rgb::RED } else { Rgb::GREEN });
                c.draw_text_centered_bold("TOGGLE", 0, 70, 96, 1, Rgb::LIGHT_GRAY);
            }
            29 => {
                // Degraded / Fault view shortcut
                c.fill_rect(0, 0, 96, 96, Rgb::new(28, 28, 36));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::AMBER);
                c.draw_text_centered_bold("FAULTS", 0, 20, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("LOGS", 0, 45, 96, 2, Rgb::AMBER);
            }
            30 => {
                // System Page shortcut
                c.fill_rect(0, 0, 96, 96, Rgb::new(28, 28, 36));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("SYSTEM", 0, 20, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("CONFIG", 0, 45, 96, 2, Rgb::WHITE);
            }
            31 => {
                // Launch Performance
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 110, 40));
                c.draw_rect_border(3, 3, 90, 90, 3, Rgb::GREEN);
                c.draw_text_centered_bold("PERF", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("START", 0, 45, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold("LIVE >", 0, 70, 96, 1, Rgb::WHITE);
            }

            _ => {}
        }

        c
    }

    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, _guarded: &mut GuardedEngine) -> PageAction {
        match key {
            0..=7 => {
                let bank = key as u32;
                broker.select_bank(bank);
                ctx.active_bank = bank;
                ctx.state.active_bank = bank;
                PageAction::None
            }
            12 | 28 => {
                broker.toggle_blackout();
                ctx.state.blackout = !ctx.state.blackout;
                PageAction::None
            }
            15 | 29 => {
                PageAction::SwitchPage(PageId::Degraded)
            }
            16..=19 | 31 => {
                PageAction::SwitchPage(PageId::Performance)
            }
            20 | 21 => {
                PageAction::SwitchPage(PageId::Ingest)
            }
            22 | 23 | 30 => {
                PageAction::SwitchPage(PageId::System)
            }
            24 => {
                if ctx.state.active_bank > 0 {
                    let next_b = ctx.state.active_bank - 1;
                    broker.select_bank(next_b);
                    ctx.active_bank = next_b;
                    ctx.state.active_bank = next_b;
                }
                PageAction::None
            }
            25 => {
                if ctx.state.active_bank < 7 {
                    let next_b = ctx.state.active_bank + 1;
                    broker.select_bank(next_b);
                    ctx.active_bank = next_b;
                    ctx.state.active_bank = next_b;
                }
                PageAction::None
            }
            26 => {
                let new_bpm = (ctx.state.tempo_bpm - 1.0).max(20.0);
                broker.set_tempo_bpm(new_bpm);
                ctx.state.tempo_bpm = new_bpm;
                PageAction::None
            }
            27 => {
                let new_bpm = (ctx.state.tempo_bpm + 1.0).min(300.0);
                broker.set_tempo_bpm(new_bpm);
                ctx.state.tempo_bpm = new_bpm;
                PageAction::None
            }
            _ => PageAction::None,
        }
    }

    fn handle_key_release(&mut self, _key: u8, _ctx: &mut PageContext, _broker: &BrokerClient, _guarded: &mut GuardedEngine) -> PageAction {
        PageAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ready_page_renders_and_handles_bank_select() {
        let mut page = ReadyPage::new();
        let mut ctx = PageContext::default();
        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        let action = page.handle_key_press(3, &mut ctx, &client, &mut guarded);
        assert_eq!(action, PageAction::None);
        assert_eq!(ctx.state.active_bank, 3);

        let canvas = page.render_key(3, &ctx, &guarded);
        let jpeg = canvas.encode_jpeg();
        assert!(!jpeg.is_empty());

        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"bank.select\"") && l.contains("\"index\":3")));

        // Nav to performance
        let nav = page.handle_key_press(31, &mut ctx, &client, &mut guarded);
        assert_eq!(nav, PageAction::SwitchPage(PageId::Performance));
    }
}
