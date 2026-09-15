//! PERFORMANCE page implementation.
//!
//! 4x8 grid for clips across banks with active clip highlight, bank selection (0..7),
//! blackout toggle, BPM / master controls, and instant non-blocking command dispatch.

use crate::broker_client::BrokerClient;
use crate::graphics::{Canvas, Rgb};
use crate::guarded::GuardedEngine;
use crate::pages::{Page, PageAction, PageContext, PageId};

pub struct PerformancePage {
    bank_modal_open: bool,
}

impl PerformancePage {
    pub fn new() -> Self {
        Self {
            bank_modal_open: false,
        }
    }
}

impl Default for PerformancePage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for PerformancePage {
    fn page_id(&self) -> PageId {
        PageId::Performance
    }

    fn render_key(&self, key: u8, ctx: &PageContext, _guarded: &GuardedEngine) -> Canvas {
        let mut c = Canvas::new(Rgb::BLACK);

        if self.bank_modal_open {
            // Modal mode: Row 0 is Direct Bank Select (Banks 0..7)
            if key < 8 {
                let bank_idx = key as u32;
                let is_active = ctx.state.active_bank == bank_idx;
                let bg = if is_active { Rgb::new(0, 120, 150) } else { Rgb::new(20, 30, 45) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(3, 3, 90, 90, 2, if is_active { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered("SELECT", 0, 15, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered(&format!("BANK {bank_idx}"), 0, 38, 96, 1, if is_active { Rgb::WHITE } else { Rgb::CYAN });
                c.draw_text_centered(if is_active { "ACTIVE" } else { "SET" }, 0, 65, 96, 1, if is_active { Rgb::GREEN } else { Rgb::LIGHT_GRAY });
                return c;
            } else if key == 31 {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_RED);
                c.draw_text_centered("CLOSE", 0, 35, 96, 1, Rgb::WHITE);
                return c;
            }
        }

        // Standard Performance View
        match key {
            // Rows 0..2 (Keys 0..23): 24 Clip Trigger Pads for Active Bank
            0..=23 => {
                let clip_index = key as u32;
                let is_active = ctx.state.active_clip == Some(clip_index);
                let is_blackout = ctx.state.blackout;

                let bg = if is_active {
                    if is_blackout {
                        Rgb::new(100, 20, 20) // Active but blacked out
                    } else {
                        Rgb::new(15, 140, 60) // Bright Active Green
                    }
                } else {
                    // Distinct background per row to aid visual orientation
                    let row = key / 8;
                    match row {
                        0 => Rgb::new(20, 25, 35),
                        1 => Rgb::new(25, 20, 35),
                        _ => Rgb::new(20, 30, 30),
                    }
                };

                c.fill_rect(0, 0, 96, 96, bg);

                let border_color = if is_active {
                    if is_blackout { Rgb::RED } else { Rgb::GREEN }
                } else {
                    Rgb::MID_GRAY
                };
                let border_w = if is_active { 4 } else { 1 };
                c.draw_rect_border(2, 2, 92, 92, border_w, border_color);

                // Clip Pad Label & Status
                let bank = ctx.state.active_bank;
                c.draw_text_centered_bold(&format!("B{bank}-C{clip_index}"), 0, 12, 96, 1, if is_active { Rgb::WHITE } else { Rgb::LIGHT_GRAY });

                let main_label = format!("{clip_index}");
                c.draw_text_centered_bold(&main_label, 0, 32, 96, 3, if is_active { Rgb::WHITE } else { Rgb::CYAN });

                let status_text = if is_active {
                    if is_blackout { "BLCKOUT" } else { "PLAYING" }
                } else {
                    "READY"
                };
                c.draw_text_centered_bold(status_text, 0, 70, 96, 1, if is_active { Rgb::YELLOW } else { Rgb::LIGHT_GRAY });
            }

            // Row 3 (24..31): Controls & Navigation
            24 => {
                // Bank 0 / Modal select
                let is_b0 = ctx.state.active_bank == 0;
                c.fill_rect(0, 0, 96, 96, if is_b0 { Rgb::new(0, 100, 120) } else { Rgb::new(25, 30, 45) });
                c.draw_rect_border(2, 2, 92, 92, if is_b0 { 3 } else { 1 }, if is_b0 { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("BANK 0", 0, 22, 96, 1, if is_b0 { Rgb::WHITE } else { Rgb::CYAN });
                c.draw_text_centered_bold(if is_b0 { "ACTIVE" } else { "SELECT" }, 0, 48, 96, 1, Rgb::LIGHT_GRAY);
            }
            25 => {
                // Bank 1 / Next
                let is_b1 = ctx.state.active_bank == 1;
                c.fill_rect(0, 0, 96, 96, if is_b1 { Rgb::new(0, 100, 120) } else { Rgb::new(25, 30, 45) });
                c.draw_rect_border(2, 2, 92, 92, if is_b1 { 3 } else { 1 }, if is_b1 { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("BANK 1", 0, 22, 96, 1, if is_b1 { Rgb::WHITE } else { Rgb::CYAN });
                c.draw_text_centered_bold(if is_b1 { "ACTIVE" } else { "SELECT" }, 0, 48, 96, 1, Rgb::LIGHT_GRAY);
            }
            26 => {
                // Bank 2 / More Banks Modal
                let curr_b = ctx.state.active_bank;
                c.fill_rect(0, 0, 96, 96, Rgb::new(30, 35, 50));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::CYAN);
                c.draw_text_centered_bold("BANK 0..7", 0, 16, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(&format!("B{curr_b}"), 0, 40, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold("MENU >", 0, 70, 96, 1, Rgb::CYAN);
            }
            27 => {
                // Stop Clip Key
                let has_clip = ctx.state.active_clip.is_some();
                c.fill_rect(0, 0, 96, 96, if has_clip { Rgb::new(90, 20, 20) } else { Rgb::DARK_GRAY });
                c.draw_rect_border(2, 2, 92, 92, 2, if has_clip { Rgb::RED } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("STOP", 0, 20, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("CLIP", 0, 52, 96, 1, Rgb::YELLOW);
            }
            28 => {
                // Blackout Toggle
                let bo = ctx.state.blackout;
                let bg = if bo { Rgb::RED } else { Rgb::new(20, 25, 35) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(3, 3, 90, 90, 3, if bo { Rgb::WHITE } else { Rgb::RED });
                c.draw_text_centered_bold("BLACKOUT", 0, 16, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(if bo { "ENGAGED" } else { "TOGGLE" }, 0, 42, 96, 1, if bo { Rgb::YELLOW } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered_bold(if bo { "DARK" } else { "LIVE" }, 0, 70, 96, 1, if bo { Rgb::WHITE } else { Rgb::GREEN });
            }
            29 => {
                // BPM - / Tap Tempo
                c.fill_rect(0, 0, 96, 96, Rgb::new(25, 30, 40));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("BPM", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{:.0}", ctx.state.tempo_bpm), 0, 36, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold("TAP -", 0, 70, 96, 1, Rgb::WHITE);
            }
            30 => {
                // BPM + / Master Opacity
                c.fill_rect(0, 0, 96, 96, Rgb::new(25, 30, 40));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("OPACITY", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let opacity_pct = (ctx.state.master_opacity * 100.0).round() as u32;
                c.draw_text_centered_bold(&format!("{opacity_pct}%"), 0, 36, 96, 2, Rgb::CYAN);
                c.draw_text_centered_bold("FAST +", 0, 70, 96, 1, Rgb::WHITE);
            }
            31 => {
                // Disarm / Return to System / Ready Page
                c.fill_rect(0, 0, 96, 96, Rgb::new(40, 40, 50));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("EXIT", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("DISARM", 0, 44, 96, 1, Rgb::AMBER);
                c.draw_text_centered_bold("READY >", 0, 70, 96, 1, Rgb::CYAN);
            }

            _ => {}
        }

        c
    }

    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, _guarded: &mut GuardedEngine) -> PageAction {
        if self.bank_modal_open {
            if key < 8 {
                let bank = key as u32;
                broker.select_bank(bank);
                ctx.active_bank = bank;
                ctx.state.active_bank = bank;
                self.bank_modal_open = false;
                return PageAction::None;
            } else if key == 31 {
                self.bank_modal_open = false;
                return PageAction::None;
            }
        }

        match key {
            0..=23 => {
                // Trigger clip immediately over non-blocking broker channel!
                let clip_index = key as u32;
                let bank = ctx.state.active_bank;
                broker.trigger_clip(clip_index, Some(bank));
                ctx.state.active_clip = Some(clip_index);
                PageAction::None
            }
            24 => {
                broker.select_bank(0);
                ctx.active_bank = 0;
                ctx.state.active_bank = 0;
                PageAction::None
            }
            25 => {
                broker.select_bank(1);
                ctx.active_bank = 1;
                ctx.state.active_bank = 1;
                PageAction::None
            }
            26 => {
                self.bank_modal_open = true;
                PageAction::None
            }
            27 => {
                broker.stop_clip();
                ctx.state.active_clip = None;
                PageAction::None
            }
            28 => {
                broker.toggle_blackout();
                ctx.state.blackout = !ctx.state.blackout;
                PageAction::None
            }
            29 => {
                let new_bpm = (ctx.state.tempo_bpm - 2.0).max(20.0);
                broker.set_tempo_bpm(new_bpm);
                ctx.state.tempo_bpm = new_bpm;
                PageAction::None
            }
            30 => {
                let new_bpm = (ctx.state.tempo_bpm + 2.0).min(300.0);
                broker.set_tempo_bpm(new_bpm);
                ctx.state.tempo_bpm = new_bpm;
                PageAction::None
            }
            31 => {
                PageAction::SwitchPage(PageId::Ready)
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
    fn test_performance_page_clip_trigger_and_blackout() {
        let mut page = PerformancePage::new();
        let mut ctx = PageContext::default();
        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        // Trigger clip 7
        page.handle_key_press(7, &mut ctx, &client, &mut guarded);
        assert_eq!(ctx.state.active_clip, Some(7));

        // Render clip 7: should be highlighted
        let canvas = page.render_key(7, &ctx, &guarded);
        let jpeg = canvas.encode_jpeg();
        assert!(!jpeg.is_empty());

        // Toggle blackout
        page.handle_key_press(28, &mut ctx, &client, &mut guarded);
        assert_eq!(ctx.state.blackout, true);

        // Render clip 7 during blackout
        let bo_canvas = page.render_key(7, &ctx, &guarded);
        assert_eq!(bo_canvas.get_pixel(48, 48), Rgb::new(100, 20, 20));

        std::thread::sleep(std::time::Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"clip.trigger\"") && l.contains("\"key\":7")));
        assert!(lines.iter().any(|l| l.contains("\"blackout.toggle\"")));
    }
}
