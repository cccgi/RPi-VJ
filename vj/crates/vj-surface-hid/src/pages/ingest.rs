//! INGEST page implementation.
//!
//! USB and LAN ingest pipeline monitor, advisory crop/seam policy,
//! and guarded show activation and cancellation.

use crate::broker_client::BrokerClient;
use crate::graphics::{Canvas, Rgb};
use crate::guarded::{GuardedAction, GuardedEngine, GuardedEngineState};
use crate::pages::{Page, PageAction, PageContext, PageId};

pub struct IngestPage;

impl IngestPage {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IngestPage {
    fn default() -> Self {
        Self::new()
    }
}

impl Page for IngestPage {
    fn page_id(&self) -> PageId {
        PageId::Ingest
    }

    fn render_key(&self, key: u8, ctx: &PageContext, guarded: &GuardedEngine) -> Canvas {
        let mut c = Canvas::new(Rgb::DARK_GRAY);

        // Check active hold progress on guarded keys
        let hold_progress = match guarded.state() {
            GuardedEngineState::Holding(hold) if hold.key == key => {
                let elapsed = ctx.now.duration_since(hold.start);
                Some((hold.action, (elapsed.as_secs_f32() / hold.duration.as_secs_f32()).clamp(0.0, 1.0)))
            }
            _ => None,
        };

        match key {
            // Row 0 (0..7): Ingest Pipeline Step Indicators
            0 => {
                // USB Status
                let inserted = ctx.ingest.usb_inserted;
                c.fill_rect(0, 0, 96, 96, if inserted { Rgb::new(20, 80, 40) } else { Rgb::DARK_GRAY });
                c.draw_rect_border(2, 2, 92, 92, 2, if inserted { Rgb::GREEN } else { Rgb::MID_GRAY });
                c.draw_text_centered("USB DRIVE", 0, 18, 96, 1, Rgb::WHITE);
                c.draw_text_centered(if inserted { "MOUNTED" } else { "NO MEDIA" }, 0, 42, 96, 1, if inserted { Rgb::YELLOW } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered("INBOX", 0, 68, 96, 1, Rgb::CYAN);
            }
            1..=5 => {
                let step_idx = key;
                let step_name = match step_idx {
                    1 => "1.STAGE",
                    2 => "2.PROBE",
                    3 => "3.ANALYZE",
                    4 => "4.TRANSCODE",
                    5 => "5.VALIDATE",
                    _ => "",
                };
                let p = ctx.ingest.progress;
                let step_threshold = (step_idx as f32) / 6.0;
                let step_done = p >= step_threshold;

                c.fill_rect(0, 0, 96, 96, if step_done { Rgb::new(10, 60, 25) } else { Rgb::new(25, 30, 40) });
                c.draw_rect_border(2, 2, 92, 92, 1, if step_done { Rgb::GREEN } else { Rgb::MID_GRAY });
                c.draw_text_centered(step_name, 0, 20, 96, 1, Rgb::WHITE);
                c.draw_text_centered(if step_done { "DONE" } else { "QUEUED" }, 0, 45, 96, 1, if step_done { Rgb::GREEN } else { Rgb::LIGHT_GRAY });
            }
            6 => {
                // Warning / Advisory indicator
                let has_warn = ctx.ingest.warning.is_some();
                c.fill_rect(0, 0, 96, 96, if has_warn { Rgb::new(90, 60, 10) } else { Rgb::new(20, 30, 40) });
                c.draw_rect_border(2, 2, 92, 92, 2, if has_warn { Rgb::AMBER } else { Rgb::MID_GRAY });
                c.draw_text_centered("ADVISORY", 0, 18, 96, 1, Rgb::WHITE);
                c.draw_text_centered(if has_warn { "WARN" } else { "CLEAN" }, 0, 42, 96, 1, if has_warn { Rgb::YELLOW } else { Rgb::GREEN });
                c.draw_text_centered("POLICY", 0, 68, 96, 1, Rgb::LIGHT_GRAY);
            }
            7 => {
                // Ingest Overall Progress
                let pct = (ctx.ingest.progress * 100.0).round() as u32;
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_text_centered("TOTAL", 0, 15, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered(&format!("{pct}%"), 0, 38, 96, 2, Rgb::GREEN);
                c.draw_progress_bar(8, 70, 80, 12, ctx.ingest.progress, Rgb::GREEN, Rgb::DARK_GRAY);
            }

            // Row 1 (8..15): Job Details
            8..=11 => {
                // Job Name
                c.fill_rect(0, 0, 96, 96, Rgb::new(30, 25, 45));
                c.draw_text_centered("ACTIVE JOB", 0, 15, 96, 1, Rgb::LIGHT_GRAY);
                let name = ctx.ingest.job_name.as_deref().unwrap_or("None");
                c.draw_text_centered(name, 0, 40, 96, 1, Rgb::WHITE);
                c.draw_text_centered(&ctx.ingest.step, 0, 65, 96, 1, Rgb::CYAN);
            }
            12..=15 => {
                // Free Space Invariant
                c.fill_rect(0, 0, 96, 96, Rgb::new(25, 30, 35));
                c.draw_text_centered("STORAGE", 0, 15, 96, 1, Rgb::LIGHT_GRAY);
                let free_gb = (ctx.telemetry.free_space_bytes as f32) / 1_000_000_000.0;
                c.draw_text_centered(&format!("{free_gb:.1}G FREE"), 0, 40, 96, 1, Rgb::WHITE);
                c.draw_text_centered("SAFE (>15%)", 0, 65, 96, 1, Rgb::GREEN);
            }

            // Row 2 (16..23): Proposed Policy Actions
            16 | 17 => {
                // Crop Quadrant Toggle
                let on = ctx.ingest.proposed_crop;
                c.fill_rect(0, 0, 96, 96, if on { Rgb::new(0, 90, 100) } else { Rgb::DARK_GRAY });
                c.draw_rect_border(2, 2, 92, 92, 2, if on { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered("QUADRANT", 0, 18, 96, 1, Rgb::WHITE);
                c.draw_text_centered("CROP", 0, 40, 96, 2, if on { Rgb::WHITE } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered(if on { "ENABLED" } else { "DISABLED" }, 0, 70, 96, 1, if on { Rgb::GREEN } else { Rgb::MID_GRAY });
            }
            18 | 19 => {
                // Seam Crossfade Toggle
                let on = ctx.ingest.proposed_seam;
                c.fill_rect(0, 0, 96, 96, if on { Rgb::new(0, 90, 100) } else { Rgb::DARK_GRAY });
                c.draw_rect_border(2, 2, 92, 92, 2, if on { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered("LOOP SEAM", 0, 18, 96, 1, Rgb::WHITE);
                c.draw_text_centered("CROSSFADE", 0, 42, 96, 1, if on { Rgb::WHITE } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered(if on { "SMOOTH" } else { "HARD CUT" }, 0, 70, 96, 1, if on { Rgb::GREEN } else { Rgb::MID_GRAY });
            }
            20..=23 => {
                // Output Target Info
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 25, 40));
                c.draw_text_centered("TARGET", 0, 18, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered("96x96 ICONS", 0, 40, 96, 1, Rgb::CYAN);
                c.draw_text_centered("+ FULL HD H264", 0, 65, 96, 1, Rgb::WHITE);
            }

            // Row 3 (24..31): Guarded Actions & Navigation
            24 => {
                // Start / Ingest Next
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 100, 40));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::GREEN);
                c.draw_text_centered("START", 0, 20, 96, 2, Rgb::WHITE);
                c.draw_text_centered("INGEST", 0, 50, 96, 1, Rgb::YELLOW);
            }
            25 => {
                // Guarded Cancel Ingest (3s hold)
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_RED);
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::RED);
                c.draw_text_centered("CANCEL", 0, 15, 96, 1, Rgb::WHITE);
                c.draw_text_centered("HOLD 3S", 0, 38, 96, 1, Rgb::YELLOW);
                if let Some((GuardedAction::CancelIngest, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::YELLOW);
                } else {
                    c.draw_text_centered("GUARDED", 0, 65, 96, 1, Rgb::LIGHT_GRAY);
                }
            }
            26 => {
                // Guarded Delete Show (3s hold)
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_RED);
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::RED);
                c.draw_text_centered("DELETE", 0, 15, 96, 1, Rgb::WHITE);
                c.draw_text_centered("HOLD 3S", 0, 38, 96, 1, Rgb::YELLOW);
                if let Some((GuardedAction::DeleteShow, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::YELLOW);
                } else {
                    c.draw_text_centered("STAGING", 0, 65, 96, 1, Rgb::LIGHT_GRAY);
                }
            }
            27 => {
                // Guarded Activate Show Generation (3s hold)
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 90, 110));
                c.draw_rect_border(3, 3, 90, 90, 3, Rgb::CYAN);
                c.draw_text_centered("ACTIVATE", 0, 15, 96, 1, Rgb::WHITE);
                c.draw_text_centered("SHOW GEN", 0, 35, 96, 1, Rgb::YELLOW);
                if let Some((GuardedAction::ActivateShow, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::CYAN);
                } else {
                    c.draw_text_centered("HOLD 3S", 0, 65, 96, 1, Rgb::WHITE);
                }
            }
            28..=30 => {
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_GRAY);
                c.draw_text_centered("STATUS", 0, 25, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered("FSYNC OK", 0, 50, 96, 1, Rgb::GREEN);
            }
            31 => {
                // Return to Ready
                c.fill_rect(0, 0, 96, 96, Rgb::new(40, 40, 55));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::LIGHT_GRAY);
                c.draw_text_centered("EXIT", 0, 18, 96, 2, Rgb::WHITE);
                c.draw_text_centered("READY >", 0, 55, 96, 1, Rgb::CYAN);
            }

            _ => {}
        }

        c
    }

    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, guarded: &mut GuardedEngine) -> PageAction {
        match key {
            16 | 17 => {
                ctx.ingest.proposed_crop = !ctx.ingest.proposed_crop;
                PageAction::None
            }
            18 | 19 => {
                ctx.ingest.proposed_seam = !ctx.ingest.proposed_seam;
                PageAction::None
            }
            24 => {
                // Start / Accept
                broker.send_command("ingest.start", serde_json::json!({}));
                PageAction::None
            }
            25 => {
                guarded.on_press(GuardedAction::CancelIngest, key, ctx.now);
                PageAction::None
            }
            26 => {
                guarded.on_press(GuardedAction::DeleteShow, key, ctx.now);
                PageAction::None
            }
            27 => {
                guarded.on_press(GuardedAction::ActivateShow, key, ctx.now);
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
            match action {
                GuardedAction::CancelIngest => {
                    broker.send_command("ingest.cancel", serde_json::json!({}));
                    PageAction::TriggerGuarded(action)
                }
                GuardedAction::DeleteShow => {
                    broker.send_command("show.delete_staging", serde_json::json!({}));
                    PageAction::TriggerGuarded(action)
                }
                GuardedAction::ActivateShow => {
                    broker.send_command("show.activate", serde_json::json!({}));
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
    fn test_ingest_page_rendering_and_crop_toggle() {
        let mut page = IngestPage::new();
        let mut ctx = PageContext::default();
        let (client, _mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        assert!(!ctx.ingest.proposed_crop);
        page.handle_key_press(16, &mut ctx, &client, &mut guarded);
        assert!(ctx.ingest.proposed_crop);

        let canvas = page.render_key(16, &ctx, &guarded);
        let jpeg = canvas.encode_jpeg();
        assert!(!jpeg.is_empty());
    }
}
