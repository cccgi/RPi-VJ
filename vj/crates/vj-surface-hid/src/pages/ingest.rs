//! INGEST page implementation.
//!
//! Full standalone USB & media import workflow on Stream Deck XL:
//! - Drive status indicator (USB plugged / scanned / ready / ejected)
//! - File count & current item display with truncated filename
//! - Real-time progress bar / percentage
//! - Media analysis recommendation display (QUAD_MIRROR, SEAM_GAP, OK, BAD)
//! - Interactive decision actions:
//!   * Accept Recommendation (key 16)
//!   * Keep Full Frame (override quad) (key 17)
//!   * Crop Quadrant (force quad split/crop) (key 18)
//!   * Repair Seam (apply blend loop) (key 19)
//!   * Skip File (key 20)
//!   * Cancel Job (guarded 3s hold, key 25)
//!   * Activate Show Generation (guarded 3s hold, key 27)
//!   * Safe Eject USB Drive (guarded 3s hold, key 28)
//!   * Exit to System (key 31)

use crate::broker_client::BrokerClient;
use crate::graphics::{Canvas, Rgb};
use crate::guarded::{GuardedAction, GuardedEngine, GuardedEngineState};
use crate::pages::{MediaRecommendation, Page, PageAction, PageContext, PageId, UsbDriveStatus};

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

/// Helper to truncate filename for 96x96 key display (max ~10-12 chars)
fn abbreviate_filename(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        let ext_idx = name.rfind('.');
        match ext_idx {
            Some(idx) if idx < name.len() => {
                let ext = &name[idx..];
                let prefix_len = max_len.saturating_sub(ext.len() + 1);
                format!("{}~{}", &name[..prefix_len], ext)
            }
            _ => format!("{}...", &name[..max_len.saturating_sub(3)]),
        }
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
            // Row 0 (0..7): Ingest Pipeline Step Indicators & Summary
            0 => {
                // USB Status (Plugged / Scanning / Ready / Ejected / No Media)
                let status = ctx.ingest.usb_status;
                let bg = match status {
                    UsbDriveStatus::Ready => Rgb::new(20, 80, 40),
                    UsbDriveStatus::Scanning | UsbDriveStatus::Plugged => Rgb::new(80, 60, 10),
                    UsbDriveStatus::Ejected => Rgb::new(40, 40, 50),
                    UsbDriveStatus::NoMedia => Rgb::DARK_GRAY,
                };
                let border_color = match status {
                    UsbDriveStatus::Ready => Rgb::GREEN,
                    UsbDriveStatus::Scanning | UsbDriveStatus::Plugged => Rgb::YELLOW,
                    _ => Rgb::MID_GRAY,
                };

                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, border_color);
                c.draw_text_centered_bold("USB DRIVE", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(status.label(), 0, 38, 96, 1, if status == UsbDriveStatus::Ready { Rgb::GREEN } else { Rgb::YELLOW });
                c.draw_text_centered_bold("INBOX", 0, 66, 96, 1, Rgb::CYAN);
            }
            1..=5 => {
                // Pipeline Steps: STAGE, PROBE, ANALYZE, TRANSCODE, VALIDATE
                let step_idx = key;
                let step_name = match step_idx {
                    1 => "1.STAGE",
                    2 => "2.PROBE",
                    3 => "3.ANALYZE",
                    4 => "4.TRANS",
                    5 => "5.VALID",
                    _ => "",
                };
                let p = ctx.ingest.progress;
                let step_threshold = (step_idx as f32) / 6.0;
                let step_done = p >= step_threshold;
                let is_current = p >= ((step_idx - 1) as f32 / 6.0) && p < step_threshold;

                let bg = if step_done {
                    Rgb::new(10, 60, 25)
                } else if is_current {
                    if ctx.ingest.transcode_paused_for_perf && step_idx == 4 {
                        Rgb::new(80, 40, 10) // Paused for performance
                    } else {
                        Rgb::new(20, 50, 70) // Active processing
                    }
                } else {
                    Rgb::new(25, 30, 40)
                };

                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, if is_current { 2 } else { 1 }, if is_current { Rgb::CYAN } else if step_done { Rgb::GREEN } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold(step_name, 0, 18, 96, 1, Rgb::WHITE);

                let status_lbl = if step_done {
                    "DONE"
                } else if is_current {
                    if ctx.ingest.transcode_paused_for_perf && step_idx == 4 {
                        "PAUSED"
                    } else {
                        "ACTIVE"
                    }
                } else {
                    "QUEUED"
                };
                let status_color = if step_done { Rgb::GREEN } else if is_current { Rgb::YELLOW } else { Rgb::LIGHT_GRAY };
                c.draw_text_centered_bold(status_lbl, 0, 46, 96, 1, status_color);
            }
            6 => {
                // Warning / Advisory indicator
                let has_err = ctx.ingest.error_code.is_some();
                let has_warn = ctx.ingest.warning.is_some();
                let bg = if has_err {
                    Rgb::DARK_RED
                } else if has_warn {
                    Rgb::new(90, 60, 10)
                } else {
                    Rgb::new(20, 30, 40)
                };

                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, if has_err { Rgb::RED } else if has_warn { Rgb::AMBER } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("ADVISORY", 0, 14, 96, 1, Rgb::WHITE);
                if let Some(err) = &ctx.ingest.error_code {
                    c.draw_text_centered_bold("ERROR", 0, 38, 96, 1, Rgb::RED);
                    c.draw_text_centered_bold(err, 0, 62, 96, 1, Rgb::YELLOW);
                } else if has_warn {
                    c.draw_text_centered_bold("WARN", 0, 38, 96, 1, Rgb::YELLOW);
                    c.draw_text_centered_bold("POLICY", 0, 62, 96, 1, Rgb::LIGHT_GRAY);
                } else {
                    c.draw_text_centered_bold("CLEAN", 0, 38, 96, 1, Rgb::GREEN);
                    c.draw_text_centered_bold("POLICY", 0, 62, 96, 1, Rgb::LIGHT_GRAY);
                }
            }
            7 => {
                // Ingest Overall Progress
                let pct = (ctx.ingest.progress * 100.0).round() as u32;
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_BLUE);
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::CYAN);
                c.draw_text_centered_bold("TOTAL", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{pct}%"), 0, 36, 96, 2, Rgb::GREEN);
                c.draw_progress_bar(8, 68, 80, 14, ctx.ingest.progress, Rgb::GREEN, Rgb::DARK_GRAY);
            }

            // Row 1 (8..15): File Count & Current Item Info & Headroom
            8 | 9 => {
                // File Count & Current Index
                c.fill_rect(0, 0, 96, 96, Rgb::new(30, 25, 45));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::PURPLE);
                c.draw_text_centered_bold("FILE QUEUE", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let count_str = format!("{}/{}", ctx.ingest.current_file_index, ctx.ingest.file_count);
                c.draw_text_centered_bold(&count_str, 0, 36, 96, 2, Rgb::YELLOW);
                c.draw_text_centered_bold(if ctx.ingest.file_count > 0 { "IN PROGRESS" } else { "IDLE" }, 0, 68, 96, 1, Rgb::WHITE);
            }
            10 | 11 => {
                // Current Item / Truncated Filename
                c.fill_rect(0, 0, 96, 96, Rgb::new(25, 30, 45));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::CYAN);
                c.draw_text_centered_bold("CURRENT ITEM", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                let raw_name = ctx.ingest.current_file_name.as_deref().or(ctx.ingest.job_name.as_deref()).unwrap_or("None");
                let short_name = abbreviate_filename(raw_name, 12);
                c.draw_text_centered_bold(&short_name, 0, 38, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(&ctx.ingest.step, 0, 65, 96, 1, Rgb::CYAN);
            }
            12 | 13 => {
                // Media Analysis Recommendation
                let rec = ctx.ingest.recommendation;
                let (bg, border) = match rec {
                    MediaRecommendation::QuadMirrorDetected => (Rgb::new(20, 50, 70), Rgb::CYAN),
                    MediaRecommendation::SeamGapDetected => (Rgb::new(60, 40, 10), Rgb::AMBER),
                    MediaRecommendation::QuadAndSeamDetected => (Rgb::new(70, 30, 50), Rgb::MAGENTA),
                    MediaRecommendation::CorruptOrUnsupported => (Rgb::new(60, 10, 10), Rgb::RED),
                    MediaRecommendation::Ok => (Rgb::new(10, 50, 20), Rgb::GREEN),
                    MediaRecommendation::None => (Rgb::new(20, 25, 35), Rgb::MID_GRAY),
                };

                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, border);
                c.draw_text_centered_bold("ANALYSIS", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(rec.label(), 0, 38, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("DETECTED", 0, 65, 96, 1, Rgb::YELLOW);
            }
            // 14|15 (Free Space/Headroom) intentionally left blank: it read
            // telemetry.free_space_bytes/total_space_bytes, which were never
            // updated by refresh_telemetry() -- permanently fake numbers.
            // Removed with the fields themselves rather than left to lie.

            // Row 2 (16..23): Interactive Decision Actions
            16 => {
                // Accept Recommendation
                c.fill_rect(0, 0, 96, 96, Rgb::new(15, 80, 40));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::GREEN);
                c.draw_text_centered_bold("ACCEPT", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("RECOMMEND", 0, 44, 96, 1, Rgb::YELLOW);
                c.draw_text_centered_bold("APPLY >", 0, 68, 96, 1, Rgb::GREEN);
            }
            17 => {
                // Keep Full Frame (Override Quad)
                let on = !ctx.ingest.proposed_crop;
                c.fill_rect(0, 0, 96, 96, if on { Rgb::new(20, 60, 90) } else { Rgb::DARK_GRAY });
                c.draw_rect_border(2, 2, 92, 92, 2, if on { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("KEEP", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("FULL FRAME", 0, 44, 96, 1, if on { Rgb::WHITE } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered_bold(if on { "ACTIVE" } else { "OVERRIDE" }, 0, 68, 96, 1, if on { Rgb::GREEN } else { Rgb::MID_GRAY });
            }
            18 => {
                // Crop Quadrant (Force Quad Split/Crop)
                let on = ctx.ingest.proposed_crop;
                c.fill_rect(0, 0, 96, 96, if on { Rgb::new(0, 90, 100) } else { Rgb::DARK_GRAY });
                c.draw_rect_border(2, 2, 92, 92, 2, if on { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("CROP", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("QUADRANT", 0, 44, 96, 1, if on { Rgb::WHITE } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered_bold(if on { "FORCED" } else { "OFF" }, 0, 68, 96, 1, if on { Rgb::GREEN } else { Rgb::MID_GRAY });
            }
            19 => {
                // Repair Seam (Apply blend loop)
                let on = ctx.ingest.proposed_seam;
                c.fill_rect(0, 0, 96, 96, if on { Rgb::new(0, 90, 100) } else { Rgb::DARK_GRAY });
                c.draw_rect_border(2, 2, 92, 92, 2, if on { Rgb::CYAN } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("REPAIR", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("LOOP SEAM", 0, 44, 96, 1, if on { Rgb::WHITE } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered_bold(if on { "BLEND ON" } else { "HARD CUT" }, 0, 68, 96, 1, if on { Rgb::GREEN } else { Rgb::MID_GRAY });
            }
            20 => {
                // Skip File
                c.fill_rect(0, 0, 96, 96, Rgb::new(60, 30, 20));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::ORANGE);
                c.draw_text_centered_bold("SKIP", 0, 18, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("FILE", 0, 48, 96, 1, Rgb::YELLOW);
            }
            // 21..=23 (Output Target Info) intentionally left blank: was
            // static hardcoded text with no dynamic binding at all.

            // Row 3 (24..31): Guarded Actions & Navigation
            24 => {
                // Start / Ingest Next
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 100, 40));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::GREEN);
                c.draw_text_centered_bold("START", 0, 18, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("INGEST", 0, 48, 96, 1, Rgb::YELLOW);
            }
            25 => {
                // Guarded Cancel Ingest (3s hold)
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_RED);
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::RED);
                c.draw_text_centered_bold("CANCEL", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("JOB", 0, 35, 96, 1, Rgb::YELLOW);
                if let Some((GuardedAction::CancelIngest, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::YELLOW);
                } else {
                    c.draw_text_centered_bold("HOLD 3S", 0, 65, 96, 1, Rgb::LIGHT_GRAY);
                }
            }
            26 => {
                // Guarded Delete Show (3s hold)
                c.fill_rect(0, 0, 96, 96, Rgb::DARK_RED);
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::RED);
                c.draw_text_centered_bold("DELETE", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("STAGING", 0, 35, 96, 1, Rgb::YELLOW);
                if let Some((GuardedAction::DeleteShow, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::YELLOW);
                } else {
                    c.draw_text_centered_bold("HOLD 3S", 0, 65, 96, 1, Rgb::LIGHT_GRAY);
                }
            }
            27 => {
                // Guarded Activate Show Generation (3s hold)
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 90, 110));
                c.draw_rect_border(3, 3, 90, 90, 3, Rgb::CYAN);
                c.draw_text_centered_bold("ACTIVATE", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("SHOW GEN", 0, 35, 96, 1, Rgb::YELLOW);
                if let Some((GuardedAction::ActivateShow, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::CYAN);
                } else {
                    c.draw_text_centered_bold("HOLD 3S", 0, 65, 96, 1, Rgb::WHITE);
                }
            }
            28 => {
                // Guarded Safe Eject USB (3s hold)
                let is_ejected = ctx.ingest.usb_status == UsbDriveStatus::Ejected;
                let bg = if is_ejected { Rgb::new(40, 40, 50) } else { Rgb::new(80, 50, 10) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(2, 2, 92, 92, 2, if is_ejected { Rgb::MID_GRAY } else { Rgb::AMBER });
                c.draw_text_centered_bold("EJECT", 0, 14, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold("USB DRIVE", 0, 35, 96, 1, if is_ejected { Rgb::LIGHT_GRAY } else { Rgb::YELLOW });
                if is_ejected {
                    c.draw_text_centered_bold("SAFE NOW", 0, 65, 96, 1, Rgb::GREEN);
                } else if let Some((GuardedAction::EjectUsb, p)) = hold_progress {
                    c.draw_hold_ring(48, 65, 18, p, Rgb::YELLOW);
                } else {
                    c.draw_text_centered_bold("HOLD 3S", 0, 65, 96, 1, Rgb::LIGHT_GRAY);
                }
            }
            // 29|30 (Status/FSYNC OK) intentionally left blank: was static
            // hardcoded text with no dynamic binding at all.
            31 => {
                // Return to System (where Ingest is entered from)
                c.fill_rect(0, 0, 96, 96, Rgb::new(40, 40, 55));
                c.draw_rect_border(2, 2, 92, 92, 2, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold("EXIT", 0, 16, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("SYSTEM >", 0, 55, 96, 1, Rgb::CYAN);
            }

            _ => {}
        }

        c
    }

    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, guarded: &mut GuardedEngine) -> PageAction {
        match key {
            16 => {
                // Accept Recommendation: Apply recommended settings for active analysis
                match ctx.ingest.recommendation {
                    MediaRecommendation::QuadMirrorDetected => {
                        ctx.ingest.proposed_crop = true;
                        ctx.ingest.proposed_seam = false;
                    }
                    MediaRecommendation::SeamGapDetected => {
                        ctx.ingest.proposed_crop = false;
                        ctx.ingest.proposed_seam = true;
                    }
                    MediaRecommendation::QuadAndSeamDetected => {
                        ctx.ingest.proposed_crop = true;
                        ctx.ingest.proposed_seam = true;
                    }
                    _ => {
                        ctx.ingest.proposed_crop = false;
                        ctx.ingest.proposed_seam = false;
                    }
                }
                broker.send_command("ingest.accept_recommendation", serde_json::json!({
                    "crop": ctx.ingest.proposed_crop,
                    "seam": ctx.ingest.proposed_seam
                }));
                PageAction::None
            }
            17 => {
                // Keep Full Frame: Explicitly disable quadrant crop
                ctx.ingest.proposed_crop = false;
                broker.send_command("ingest.decision_override", serde_json::json!({ "keep_full_frame": true, "crop": false }));
                PageAction::None
            }
            18 => {
                // Crop Quadrant: Force quadrant crop enabled
                ctx.ingest.proposed_crop = true;
                broker.send_command("ingest.decision_override", serde_json::json!({ "force_crop": true, "crop": true }));
                PageAction::None
            }
            19 => {
                // Repair Seam: Force seam blend crossfade
                ctx.ingest.proposed_seam = true;
                broker.send_command("ingest.decision_override", serde_json::json!({ "repair_seam": true, "seam": true }));
                PageAction::None
            }
            20 => {
                // Skip File: Skip current file in queue
                broker.send_command("ingest.skip_file", serde_json::json!({}));
                PageAction::None
            }
            24 => {
                // Start / Ingest Next
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
            28 => {
                guarded.on_press(GuardedAction::EjectUsb, key, ctx.now);
                PageAction::None
            }
            31 => {
                PageAction::SwitchPage(PageId::System)
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
                GuardedAction::EjectUsb => {
                    ctx.ingest.usb_status = UsbDriveStatus::Ejected;
                    ctx.ingest.usb_inserted = false;
                    broker.send_command("usb.eject", serde_json::json!({}));
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
        page.handle_key_press(18, &mut ctx, &client, &mut guarded);
        assert!(ctx.ingest.proposed_crop);

        let canvas = page.render_key(18, &ctx, &guarded);
        let jpeg = canvas.encode_jpeg();
        assert!(!jpeg.is_empty());
    }

    #[test]
    fn test_ingest_page_abbreviate_filename() {
        assert_eq!(abbreviate_filename("clip.mp4", 12), "clip.mp4");
        let truncated = abbreviate_filename("my_very_long_clip_name_festival.mov", 12);
        assert!(truncated.contains(".mov"));
        assert!(truncated.len() <= 12);
    }
}

