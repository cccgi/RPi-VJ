//! PERFORMANCE page implementation.
//!
//! Matches the operator's original 32-key convention (mirrored from
//! `gig/index.html`'s own `SYS_COMMON`/nav layout, which reserves the same
//! 4 keys on every page for muscle memory in the dark):
//! - 0..23, 26..29: 28 clip/shader trigger pads for the active bank
//!   (gig leaves 26-29 free for exactly this on most of its real banks --
//!   quad-mirror, 50th, 720p, neon, etc.)
//! - 24/25: instant bank PREV/NEXT (wraps over however many banks the
//!   loaded show defines, no modal; falls back to `FALLBACK_BANK_COUNT`
//!   when no show.json loaded, e.g. mock/test contexts)
//! - 30: tap = STOP CLIP, hold 3s = exit to SYSTEM (health/renderer
//!   restart/reboot/shutdown/ingest)
//! - 31: BLACKOUT toggle (matches gig's fixed BLACK panic key)
//!
//! Master opacity and BPM have no dedicated keys here by design -- the
//! operator already drives those from a physical X-Touch Mini MIDI
//! controller and didn't want deck real estate spent duplicating them.

use std::time::{Duration, Instant};

use crate::broker_client::BrokerClient;
use crate::graphics::{Canvas, Rgb};
use crate::guarded::GuardedEngine;
use crate::pages::{Page, PageAction, PageContext, PageId, PadInfo, ShowManifest};

/// Bank wrap count used only when no show.json manifest is loaded (mock
/// transport, unit tests). A real deployed show reports its own bank count
/// via `ShowManifest::bank_count()` -- the real show has 9, not 8.
const FALLBACK_BANK_COUNT: u32 = 8;
const SYSTEM_HOLD_DURATION: Duration = Duration::from_secs(3);

/// Number of banks to wrap PREV/NEXT over: the loaded show's own bank
/// count, or `FALLBACK_BANK_COUNT` if no show.json loaded or it defines no
/// banks at all (avoids a modulo-by-zero).
fn active_bank_count(manifest: Option<&ShowManifest>) -> u32 {
    match manifest.map(|m| m.bank_count()) {
        Some(0) | None => FALLBACK_BANK_COUNT,
        Some(n) => n,
    }
}

/// True when a pad should show its real name instead of a numeric ID.
/// Any non-"clip" pad type (shader, videofx, shaderfx, blend, system) has
/// no "clip #" muscle memory to preserve, so a labeled one gets its name as
/// the dominant element; clip pads keep their number by design (see module
/// docs) even when a real label is also available.
fn uses_name_label(pad: Option<&PadInfo>) -> bool {
    pad.map(|p| p.use_name_label).unwrap_or(false)
}

pub struct PerformancePage {
    /// Press-timestamp of an in-progress hold on key 30 (STOP/SYSTEM).
    /// Plain local hold tracking, not the shared `GuardedEngine`: this is a
    /// benign navigation/stop action, not a destructive one needing the
    /// 2-key-confirm ceremony guarded actions elsewhere use.
    stop_hold_start: Option<Instant>,
}

impl PerformancePage {
    pub fn new() -> Self {
        Self {
            stop_hold_start: None,
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

        match key {
            // 28 clip/shader trigger pads: 0..23 plus 26..29
            0..=23 | 26..=29 => {
                let clip_index = key as u32;
                let is_active = ctx.state.active_clip == Some(clip_index);
                let is_blackout = ctx.state.blackout;
                let bank = ctx.state.active_bank;

                // Real pad data from show.json, if it loaded and defines
                // this bank/key. `None` for either reason falls back to the
                // original numeric-only rendering below, unchanged.
                let pad = ctx.show_manifest.as_ref().and_then(|m| m.pad(bank, key));
                let real_label = pad.map(|p| p.label.as_str());
                let has_thumbnail = pad.and_then(|p| p.thumbnail.as_ref()).is_some();
                // Non-clip pads (shader, videofx, shaderfx, blend, system)
                // never have a clip number worth keeping -- the operator
                // asked for real names to *replace* numbers on every
                // functional button except clip launch pads, so a labeled
                // one gets its name as the dominant center element instead
                // of the generic digit.
                let use_name = uses_name_label(pad);

                if let Some(thumb) = pad.and_then(|p| p.thumbnail.as_ref()) {
                    c.blit(thumb);
                } else {
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
                            2 => Rgb::new(20, 30, 30),
                            _ => Rgb::new(30, 25, 20),
                        }
                    };
                    c.fill_rect(0, 0, 96, 96, bg);
                }

                let border_color = if is_active {
                    if is_blackout { Rgb::RED } else { Rgb::GREEN }
                } else {
                    Rgb::MID_GRAY
                };
                let border_w = if is_active { 4 } else { 1 };
                c.draw_rect_border(2, 2, 92, 92, border_w, border_color);

                if use_name {
                    // Non-clip pad with a real name: the name IS the label,
                    // drawn large and centered -- no numeric ID anywhere,
                    // since there's nothing to replace it with. Pick the
                    // largest scale that still fits within the 96px icon
                    // width so short names (ADD, WARP) read bigger than
                    // long ones (PARTICLE) without ever clipping.
                    let label = real_label.unwrap_or_default();
                    let fits = |s: usize| label.len() * (6 * s + 1) <= 90;
                    let name_scale = if fits(3) { 3 } else if fits(2) { 2 } else { 1 };
                    let name_y = match name_scale {
                        3 => 38,
                        2 => 40,
                        _ => 44,
                    };
                    c.draw_text_centered_bold(label, 0, name_y, 96, name_scale, if is_active { Rgb::WHITE } else { Rgb::CYAN });
                } else {
                    // Top band: the pad's real name (clip) when show.json
                    // defines one; otherwise the original bank/clip
                    // fallback. A thumbnail gets a solid band behind the
                    // text so it stays legible over a busy video frame.
                    if has_thumbnail {
                        c.fill_rect(3, 3, 90, 16, Rgb::BLACK);
                    }
                    let top_text = real_label.map(str::to_string).unwrap_or_else(|| format!("B{bank}-C{clip_index}"));
                    c.draw_text_centered_bold(&top_text, 0, 12, 96, 1, if is_active { Rgb::WHITE } else { Rgb::LIGHT_GRAY });

                    // Center: the clip number, for quick verbal/muscle-memory
                    // reference ("hit clip 7") -- skipped when a thumbnail is
                    // present so the video frame isn't covered.
                    if !has_thumbnail {
                        let main_label = format!("{clip_index}");
                        c.draw_text_centered_bold(&main_label, 0, 32, 96, 3, if is_active { Rgb::WHITE } else { Rgb::CYAN });
                    }
                }

                // Bottom band: live status, same as before, legibility band
                // added when a thumbnail is behind it.
                if has_thumbnail {
                    c.fill_rect(3, 77, 90, 16, Rgb::BLACK);
                }
                let status_text = if is_active {
                    if is_blackout { "BLCKOUT" } else { "PLAYING" }
                } else {
                    "READY"
                };
                c.draw_text_centered_bold(status_text, 0, 70, 96, 1, if is_active { Rgb::YELLOW } else { Rgb::LIGHT_GRAY });
            }

            24 => {
                // Bank Prev (instant, wraps)
                let bank = ctx.state.active_bank;
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 40, 70));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::new(43, 108, 176));
                c.draw_text_centered_bold("BANK", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("< {bank}"), 0, 36, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("PREV", 0, 70, 96, 1, Rgb::new(43, 108, 176));
            }
            25 => {
                // Bank Next (instant, wraps)
                let bank = ctx.state.active_bank;
                c.fill_rect(0, 0, 96, 96, Rgb::new(20, 40, 70));
                c.draw_rect_border(2, 2, 92, 92, 1, Rgb::new(43, 108, 176));
                c.draw_text_centered_bold("BANK", 0, 14, 96, 1, Rgb::LIGHT_GRAY);
                c.draw_text_centered_bold(&format!("{bank} >"), 0, 36, 96, 2, Rgb::WHITE);
                c.draw_text_centered_bold("NEXT", 0, 70, 96, 1, Rgb::new(43, 108, 176));
            }
            30 => {
                // Tap: Stop Clip. Hold 3s: exit to System.
                let holding = self.stop_hold_start.is_some();
                let has_clip = ctx.state.active_clip.is_some();
                c.fill_rect(0, 0, 96, 96, if has_clip { Rgb::new(90, 20, 20) } else { Rgb::DARK_GRAY });
                c.draw_rect_border(2, 2, 92, 92, 2, if has_clip { Rgb::RED } else { Rgb::MID_GRAY });
                c.draw_text_centered_bold("STOP", 0, 16, 96, 2, Rgb::WHITE);
                if holding {
                    let elapsed = ctx.now.duration_since(self.stop_hold_start.unwrap());
                    let progress = (elapsed.as_secs_f32() / SYSTEM_HOLD_DURATION.as_secs_f32()).clamp(0.0, 1.0);
                    c.draw_hold_ring(48, 68, 16, progress, Rgb::CYAN);
                } else {
                    c.draw_text_centered_bold("HOLD 3S", 0, 68, 96, 1, Rgb::CYAN);
                }
            }
            31 => {
                // Blackout Toggle (fixed panic key, matches gig's BLACK)
                let bo = ctx.state.blackout;
                let bg = if bo { Rgb::RED } else { Rgb::new(20, 25, 35) };
                c.fill_rect(0, 0, 96, 96, bg);
                c.draw_rect_border(3, 3, 90, 90, 3, if bo { Rgb::WHITE } else { Rgb::RED });
                c.draw_text_centered_bold("BLACKOUT", 0, 16, 96, 1, Rgb::WHITE);
                c.draw_text_centered_bold(if bo { "ENGAGED" } else { "TOGGLE" }, 0, 42, 96, 1, if bo { Rgb::YELLOW } else { Rgb::LIGHT_GRAY });
                c.draw_text_centered_bold(if bo { "DARK" } else { "LIVE" }, 0, 70, 96, 1, if bo { Rgb::WHITE } else { Rgb::GREEN });
            }

            _ => {}
        }

        c
    }

    fn handle_key_press(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, _guarded: &mut GuardedEngine) -> PageAction {
        match key {
            0..=23 | 26..=29 => {
                // Forward the raw key so gig's own per-page dispatch can
                // decide what it actually means (real clip/shader/effect
                // pad on most pages, or a SPEED/BPM/OPACITY/FX-AMOUNT
                // preset on keys 26-29 of pages 1-5 -- see
                // docs/original/STREAMDECK.md). trigger_clip/ctx.state
                // below are kept for this controller's OWN icon rendering
                // (active-clip highlight) and are harmless even when the
                // pressed key turns out to be a non-clip function key.
                let clip_index = key as u32;
                let bank = ctx.state.active_bank;
                broker.notify_key_press(clip_index);
                broker.trigger_clip(clip_index, Some(bank));
                ctx.state.active_clip = Some(clip_index);
                PageAction::None
            }
            24 => {
                broker.notify_key_press(24);
                let count = active_bank_count(ctx.show_manifest.as_deref());
                let bank = (ctx.state.active_bank + count - 1) % count;
                broker.select_bank(bank);
                ctx.active_bank = bank;
                ctx.state.active_bank = bank;
                PageAction::None
            }
            25 => {
                broker.notify_key_press(25);
                let count = active_bank_count(ctx.show_manifest.as_deref());
                let bank = (ctx.state.active_bank + 1) % count;
                broker.select_bank(bank);
                ctx.active_bank = bank;
                ctx.state.active_bank = bank;
                PageAction::None
            }
            30 => {
                self.stop_hold_start = Some(ctx.now);
                PageAction::None
            }
            31 => {
                broker.notify_key_press(31);
                broker.toggle_blackout();
                ctx.state.blackout = !ctx.state.blackout;
                PageAction::None
            }
            _ => PageAction::None,
        }
    }

    fn handle_key_release(&mut self, key: u8, ctx: &mut PageContext, broker: &BrokerClient, _guarded: &mut GuardedEngine) -> PageAction {
        if key != 30 {
            return PageAction::None;
        }
        let Some(start) = self.stop_hold_start.take() else {
            return PageAction::None;
        };
        if ctx.now.duration_since(start) >= SYSTEM_HOLD_DURATION {
            PageAction::SwitchPage(PageId::System)
        } else {
            // Short tap: forward key 30 so gig runs its real RESET (stops
            // clips, clears shader, restores speed/levels, exits blackout
            // -- see docs/original/STREAMDECK.md), not just this
            // controller's own local clip.stop.
            broker.notify_key_press(30);
            broker.stop_clip();
            ctx.state.active_clip = None;
            PageAction::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::ShowManifest;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Each call gets its own directory (pid + a monotonic counter) so
    /// tests that run in parallel within the same process never race on
    /// the same "show.json" path.
    fn unique_temp_dir(prefix: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nonce = COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), nonce))
    }

    fn manifest_with_shader_label() -> ShowManifest {
        let json = r#"{"banks":[{"pads":[
            {"key": 7, "type": "clip", "label": "Laser"},
            {"key": 17, "type": "shader", "label": "TUNNEL"}
        ]}]}"#;
        let dir = unique_temp_dir("perf-page-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("show.json");
        std::fs::File::create(&path).unwrap().write_all(json.as_bytes()).unwrap();
        ShowManifest::load(&path, &dir).expect("test manifest should parse")
    }

    #[test]
    fn test_performance_page_uses_real_label_when_manifest_present() {
        let page = PerformancePage::new();
        let mut ctx = PageContext::default();
        ctx.show_manifest = Some(Arc::new(manifest_with_shader_label()));
        let guarded = GuardedEngine::default();

        // Shader pad (key 17): real name shown, no clip number drawn.
        let canvas = page.render_key(17, &ctx, &guarded);
        assert!(!canvas.encode_jpeg().is_empty());

        // Key with no manifest entry (e.g. 12) falls back to numeric B{bank}-C{n}.
        let fallback_canvas = page.render_key(12, &ctx, &guarded);
        assert!(!fallback_canvas.encode_jpeg().is_empty());
    }

    #[test]
    fn test_uses_name_label_true_only_for_non_clip_pads() {
        let manifest = manifest_with_shader_label();

        // key 17 is a labeled shader -> name replaces the numeric ID.
        assert!(uses_name_label(manifest.pad(0, 17)));

        // key 7 is a labeled clip -> keeps its number, per module docs.
        assert!(!uses_name_label(manifest.pad(0, 7)));

        // No pad at all (key 12 undefined in this test manifest) -> numeric fallback.
        assert!(!uses_name_label(manifest.pad(0, 12)));
        assert!(!uses_name_label(None));
    }

    #[test]
    fn test_bank_prev_next_wraps_over_manifest_bank_count_not_fallback() {
        // A 3-bank manifest should wrap PREV/NEXT over 3 banks, not the
        // 8-bank fallback used when no show.json is loaded.
        let json = r#"{"banks":[
            {"name":"A","pads":[]},{"name":"B","pads":[]},{"name":"C","pads":[]}
        ]}"#;
        let dir = unique_temp_dir("perf-page-bankcount-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("show.json");
        std::fs::File::create(&path).unwrap().write_all(json.as_bytes()).unwrap();
        let manifest = ShowManifest::load(&path, &dir).expect("test manifest should parse");

        let mut page = PerformancePage::new();
        let mut ctx = PageContext::default();
        ctx.show_manifest = Some(Arc::new(manifest));
        let (client, _mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        // From bank 0, PREV must wrap to bank 2 (last of 3), not bank 7.
        page.handle_key_press(24, &mut ctx, &client, &mut guarded);
        assert_eq!(ctx.state.active_bank, 2);

        // NEXT from 2 wraps back to 0.
        page.handle_key_press(25, &mut ctx, &client, &mut guarded);
        assert_eq!(ctx.state.active_bank, 0);
    }

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

        // Toggle blackout (key 31, the fixed panic key)
        page.handle_key_press(31, &mut ctx, &client, &mut guarded);
        assert!(ctx.state.blackout);

        // Render clip 7 during blackout
        let bo_canvas = page.render_key(7, &ctx, &guarded);
        assert_eq!(bo_canvas.get_pixel(48, 48), Rgb::new(100, 20, 20));

        std::thread::sleep(std::time::Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"clip.trigger\"") && l.contains("\"key\":7")));
        assert!(lines.iter().any(|l| l.contains("\"blackout.toggle\"")));
    }

    #[test]
    fn test_performance_page_extra_clip_slots_26_to_29() {
        let mut page = PerformancePage::new();
        let mut ctx = PageContext::default();
        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        for key in 26..=29u8 {
            page.handle_key_press(key, &mut ctx, &client, &mut guarded);
            assert_eq!(ctx.state.active_clip, Some(key as u32));
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"clip.trigger\"") && l.contains("\"key\":29")));
    }

    #[test]
    fn test_performance_page_bank_prev_next_wraps() {
        let mut page = PerformancePage::new();
        let mut ctx = PageContext::default();
        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        // From bank 0, PREV must wrap to bank 7.
        page.handle_key_press(24, &mut ctx, &client, &mut guarded);
        assert_eq!(ctx.state.active_bank, 7);

        // NEXT from 7 wraps back to 0.
        page.handle_key_press(25, &mut ctx, &client, &mut guarded);
        assert_eq!(ctx.state.active_bank, 0);

        std::thread::sleep(std::time::Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"bank.select\"") && l.contains("\"index\":7")));
        assert!(lines.iter().any(|l| l.contains("\"bank.select\"") && l.contains("\"index\":0")));
    }

    #[test]
    fn test_performance_page_key30_tap_stops_clip_hold_exits_to_system() {
        let mut page = PerformancePage::new();
        let mut ctx = PageContext::default();
        let (client, mock) = BrokerClient::with_mock();
        let mut guarded = GuardedEngine::default();

        // Short tap: stops the active clip, stays on Performance.
        page.handle_key_press(7, &mut ctx, &client, &mut guarded);
        assert_eq!(ctx.state.active_clip, Some(7));

        let t0 = Instant::now();
        ctx.now = t0;
        page.handle_key_press(30, &mut ctx, &client, &mut guarded);
        ctx.now = t0 + Duration::from_millis(200);
        let action = page.handle_key_release(30, &mut ctx, &client, &mut guarded);
        assert_eq!(action, PageAction::None);
        assert_eq!(ctx.state.active_clip, None);

        // Long hold (>= 3s): exits to System instead of stopping the clip.
        let t1 = Instant::now();
        ctx.now = t1;
        page.handle_key_press(30, &mut ctx, &client, &mut guarded);
        ctx.now = t1 + Duration::from_millis(3100);
        let action = page.handle_key_release(30, &mut ctx, &client, &mut guarded);
        assert_eq!(action, PageAction::SwitchPage(PageId::System));

        std::thread::sleep(std::time::Duration::from_millis(50));
        let lines = mock.drain_outgoing();
        assert!(lines.iter().any(|l| l.contains("\"clip.stop\"")));
    }
}
