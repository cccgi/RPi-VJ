//! BOOTING page implementation.
//!
//! Shows boot animation, storage readiness, renderer readiness, media readiness, and overall progress.

use crate::broker_client::BrokerClient;
use crate::graphics::{Canvas, Rgb};
use crate::guarded::GuardedEngine;
use crate::pages::{Page, PageAction, PageContext, PageId};

pub struct BootingPage {
    anim_frame: u32,
}

impl BootingPage {
    pub fn new() -> Self {
        Self { anim_frame: 0 }
    }

    pub fn tick_anim(&mut self) {
        self.anim_frame = (self.anim_frame + 1) % 1000;
    }
}

impl Default for BootingPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for BootingPage {
    fn page_id(&self) -> PageId {
        PageId::Booting
    }

    fn render_key(&self, key: u8, ctx: &PageContext, _guarded: &GuardedEngine) -> Canvas {
        let mut c = Canvas::new(Rgb::DARK_BLUE);

        match key {
            // Row 0 (0..7): Appliance Title & Boot Banner
            0 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_text_centered_bold("RPi-VJ", 0, 16, 96, 2, Rgb::CYAN);
                c.draw_text_centered_bold("BOOT", 0, 48, 96, 2, Rgb::YELLOW);
            }
            1..=6 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(4, 4, 88, 88, 1, Rgb::MID_GRAY);
                let step_num = key;
                let step_text = format!("STEP {step_num}");
                c.draw_text_centered_bold(&step_text, 0, 18, 96, 1, Rgb::LIGHT_GRAY);
                let p = ctx.boot_progress;
                let threshold = (key as f32) / 7.0;
                let color = if p >= threshold { Rgb::GREEN } else { Rgb::AMBER };
                c.fill_rect(20, 50, 56, 15, color);
            }
            7 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                let pct = (ctx.boot_progress * 100.0).round() as u32;
                c.draw_text_centered_bold("INIT", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(&format!("{pct}%"), 0, 38, 96, 2, Rgb::GREEN);
            }

            // Row 1 (8..15): Subsystem Readiness Status
            8 | 9 => {
                // Storage Readiness
                let ok = ctx.boot_storage_ok;
                let bg = if ok { Rgb::new(10, 50, 20) } else { Rgb::new(60, 40, 10) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, if ok { Rgb::GREEN } else { Rgb::AMBER });
                c.draw_text_centered_bold("STORAGE", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(if ok { "READY" } else { "CHECK" }, 0, 38, 96, 2, if ok { Rgb::GREEN } else { Rgb::YELLOW });
            }
            10 | 11 => {
                // Renderer Readiness (Cage/DRM)
                let ok = ctx.boot_renderer_ok;
                let bg = if ok { Rgb::new(10, 50, 20) } else { Rgb::new(60, 40, 10) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, if ok { Rgb::GREEN } else { Rgb::AMBER });
                c.draw_text_centered_bold("RENDERER", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(if ok { "KMS OK" } else { "START" }, 0, 38, 96, 2, if ok { Rgb::GREEN } else { Rgb::YELLOW });
            }
            12 | 13 => {
                // Media Readiness
                let ok = ctx.boot_media_ok;
                let bg = if ok { Rgb::new(10, 50, 20) } else { Rgb::new(60, 40, 10) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, if ok { Rgb::GREEN } else { Rgb::AMBER });
                c.draw_text_centered_bold("MEDIA", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(if ok { "LOADED" } else { "VERIFY" }, 0, 38, 96, 2, if ok { Rgb::GREEN } else { Rgb::YELLOW });
            }
            14 | 15 => {
                // Network / Broker IPC
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::CYAN);
                c.draw_text_centered_bold("BROKER", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("SOCKET", 0, 36, 96, 1, Rgb::CYAN);
                c.draw_text_centered_bold("ONLINE", 0, 58, 96, 1, Rgb::GREEN);
            }

            // Row 2 (16..23): Telemetry preview
            16..=19 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_GRAY);
                c.draw_text_centered_bold("IP / HOST", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&ctx.telemetry.ip_address, 0, 38, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(&ctx.telemetry.hostname, 0, 60, 96, 1, Rgb::CYAN);
            }
            20..=23 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_GRAY);
                c.draw_text_centered_bold("DISK / TEMP", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let free_gb = (ctx.telemetry.free_space_bytes as f32) / 1_000_000_000.0;
                c.draw_text_centered_bold(&format!("{free_gb:.1}G FREE"), 0, 38, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(&format!("{:.0}C", ctx.telemetry.temperature_c), 0, 60, 96, 1, Rgb::YELLOW);
            }

            // Row 3 (24..31): Continuous Progress Bar & Skip/Arm
            24..=30 => {
                let col = key - 24;
                let slot_min = (col as f32) / 7.0;
                let slot_max = ((col + 1) as f32) / 7.0;
                let slot_p = ((ctx.boot_progress - slot_min) / (slot_max - slot_min)).clamp(0.0, 1.0);

                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_progress_bar(8, 30, 80, 36, slot_p, Rgb::GREEN, Rgb::DARK_GRAY);
            }
            31 => {
                // Ready Button / Skip to Ready
                let is_ready = ctx.boot_progress >= 1.0 && ctx.boot_storage_ok && ctx.boot_renderer_ok && ctx.boot_media_ok;
                let bg = if is_ready { Rgb::new(20, 100, 30) } else { Rgb::MID_GRAY };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(3, 3, 90, 90, 2, if is_ready { Rgb::GREEN } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered_bold("ENTER", 0, 18, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("READY", 0, 50, 96, 2, if is_ready { Rgb::GREEN } else { Rgb::LIGHT_GRAY });
            }

            _ => {}
        }

        c
    }

    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, _broker: &BrokerClient, _guarded: &mut GuardedEngine) -> PageAction {
        if key == 31 || ctx.boot_progress >= 1.0 {
            PageAction::SwitchPage(PageId::Ready)
        } else {
            PageAction::None
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
    fn test_booting_page_renders_all_keys() {
        let page = BootingPage::new();
        let ctx = PageContext::default();
        let guarded = GuardedEngine::default();

        for key in 0..32u8 {
            let canvas = page.render_key(key, &ctx, &guarded);
            let jpeg = canvas.encode_jpeg();
            assert!(!jpeg.is_empty(), "Key {key} JPEG must not be empty");
        }
    }
}
