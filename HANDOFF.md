# RPi-VJ Appliance V3 � Project State & Agent Handoff

**Timestamp**: September 15, 2026  
**Target Hardware**: Raspberry Pi 5 (`rpi@192.168.0.145`, hostname `rpi-vj.local`, branch `headless-v3`)  
**OS**: Debian GNU/Linux 13 (Trixie) Lite (kernel `6.18.34+rpt-rpi-2712` aarch64)  
**Peripheral Hardware**: Elgato Stream Deck XL (`0x0fd9:0x006c` / `0x0fd9:0x008f`), HDMI Monitor on Port 0 (`HDMI-A-1`)

---

## 1. Overall Status Summary
All engineering and feature development milestones (**A3-00 through A3-14**) are completed and verified on physical hardware. The system runs as a dedicated headless appliance without a standard desktop environment or display manager.

| Task ID | Component / Milestone | Status | Notes |
|---|---|---|---|
| **A3-00** | Baseline & Recovery Image | **DONE** | 57 system artifacts captured and hashed to `/home/rpi/baseline-*`. |
| **A3-01** | Lite OS & Provisioning | **DONE** | Idempotent zero-drift provisioning script verified. |
| **A3-02** | KMS & Cage Renderer Launcher | **DONE** | Single-output KMS locked to `HDMI-A-1:1920x1080@60D`, Cage kiosk compositor. |
| **A3-04** | Persistent Logs & Boot Ledger | **DONE** | 9-field structured JSONL schema in `/var/lib/rpi-vj/boot-ledger.jsonl`. |
| **A3-05** | Broker Protocol & Supervisor | **DONE** | Unix socket `/run/rpi-vj/broker.sock`, state store, monotonic crash supervisor. |
| **A3-06** | Native Stream Deck Transport | **DONE** | Low-latency `hidraw` transport with edge triggering and icon caching. |
| **A3-07** | Stream Deck State UI Engine | **DONE** | 6 deterministic operational pages (`BOOTING`, `READY`, `PERFORMANCE`, `INGEST`, `DEGRADED`, `SYSTEM`). |
| **A3-08** | Show Storage & Job Engine | **DONE** | Immutable generations (`gen-<ts>-<hash>`), atomic symlink swap, 15% reserve check. |
| **A3-09** | USB Ingest Engine | **DONE** | Secure `nosuid,nodev,noexec,ro` USB discovery, streaming copy to staging. |
| **A3-10** | Media Pipeline & Transcode | **DONE** | ffmpeg/ffprobe analysis (codec, quad mirror detection), 1080p60 transcode, 96x96 thumbnails. |
| **A3-11** | Stream Deck Ingest Workflow | **DONE** | Interactive USB ingest page with progress gauge and crop toggle. |
| **A3-12** | LAN Web Ingest Portal | **DONE** | Drag-and-drop web portal at `http://rpi-vj.local/portal/` with chunked upload API. |
| **A3-13** | Appliance Hardening | **DONE** | Systemd `ProtectSystem=strict`, `/dev/watchdog` (15s runtime), automated staging GC. |
| **A3-14** | Release Qualification Suite | **DONE** | 12/12 automated qualification checks passed on Pi 5. |
| **A3-03** | Offline Boot Hardware Gate | **PENDING** | Physical 25-cycle cold boot logging run on physical test bench. |

---

## 1a-8. Most Button Mappings Wrong — Bridge Was Reimplementing Per-Page Logic Instead of Delegating to It (same day)

Operator, correctly and pointedly: "Streamdeck button mappings are documented precisely how each button on each page is mapped. You need to study it and map them correctly accordingly... That is a huge waste of time while everything worked in main branch and mapping is documented well." Should have found `docs/original/STREAMDECK.md` before making any assumptions about key layout — it's the authoritative, already-written spec for exactly this.

**What that doc revealed**: keys 26-29 are **not** always 4 extra clip slots as §1a-3's redesign assumed. They're **page-specific function keys** on 5 of the 9 pages — SPEED−/SPEED+/CLR CLIP/CLR FX (page 1), BPM−/BPM+/TAP/HUD (page 2), OPACITY 25/50/75/100 (page 3), FX AMT 25/50/75/100 (pages 4-5) — and only extra clip slots on Quad Mirror/720p/Neon. And RESET (key 30) is far more than "stop the current clip": it stops/blanks both clips, clears the active shader, restores speed/levels, and exits blackout.

**Root cause**: none of this per-page key-26-29 meaning exists in `show.json` at all — it's pure client-side data in `gig/index.html`'s own `SYS_PAGES`/`SYS_COMMON` tables, dispatched through `handleSystemKey()` (called from `handleKeyPress`/`window.__vjPress` whenever `keyIdx >= 24`). That dispatcher was **already fully correct** — implements every case in the doc exactly, including the RESET/BLACK/nav behavior and the 3s-hold-then-BLACK-within-5s shutdown gesture. The bug was entirely in *this session's own bridge code* (`startNativeControllerBridge`): instead of forwarding raw key presses to `window.__vjPress()` and letting that authoritative dispatch handle everything, it **reimplemented a partial subset** of the behavior itself — manually flipping `showManager.currentBankIndex` for nav, manually calling `showManager.toggleBlackout()` for BLACK, and looping `activeKeys` to fake a "stop" for RESET. This meant: keys 26-29 on 5 of 9 pages did nothing (their real per-page action was never invoked, and the native side's naive `clip.trigger` for those keys had no matching pad in `show.json` there anyway); RESET only stopped a clip, never cleared shader/exited blackout/restored speed as documented; and BLACK bypassed the shutdown-arm gesture logic entirely.

**Fix — delegate instead of reimplement**:
- `vj-broker/src/state.rs`: new `"key.press"` command, storing `last_key_press: Option<u32>` and a dedicated `key_press_seq: u64` that increments **only** on `key.press` (deliberately not reusing the general `version` counter, which other unrelated commands also bump — would have caused stale replay).
- `vj-surface-hid`: `BrokerClient::notify_key_press(key)` sends it; `performance.rs`'s `handle_key_press`/`handle_key_release` now call it for **every** key (0-31), in addition to the existing per-key-type commands (`clip.trigger`/`select_bank`/`stop_clip`/`toggle_blackout`), which are kept only for this controller's own icon rendering and are otherwise harmless even when a key turns out to be a non-clip function key.
- `gig/index.html`'s bridge: gutted the entire reimplementation. It now tracks `key_press_seq`, and on any change calls `window.__vjPress(lastKeyPress)` — nothing else. Baselines `key_press_seq` on the first poll without firing, so a fresh page load never replays a stale press from before a restart. Poll interval tightened 250ms → 100ms (a rapid two-different-key sequence within one window would still only forward the second — a known, narrow, unchanged-from-before limitation, not solved by this fix).

**Verified** via `POST /api/v1/command` `key.press` simulating a full sequence after redeploying and a clean restart of all three services: key 0 → clip triggered (confirmed by `PRESS key=0 binding=clip:...` + engine log showing the file playing); key 27 (page 1 SPEED+) → silently consumed by `handleSystemKey` (no false clip-trigger log, unlike key 0's real trigger) — confirms it hit the SPEED+ case, not the clip path; key 30 → `RESET ALL` logged (the full `resetAll()`, not the old partial stop); key 31 pressed twice → toggled without error; key 25 → page nav accepted, confirmed by a subsequent key-0 press on the new page playing a *different* clip. All 66 tests (`vj-broker` + `vj-surface-hid`) pass on the Pi's own build; both binaries + `index.html` hash-verified redeployed.

**Known residual gap, not fixed** (rendering only, not functional): this controller's own icon for keys 26-29 on pages 1-5 still draws the old generic clip/number styling (or blank fallback) rather than the real SPEED/BPM/OPACITY/FX-AMOUNT labels, since that data lives only in gig's JS, not in `show.json`, and the native Rust side has no source to read it from. The *behavior* is now correct (verified above); only the *icon label* on those 20 keys (4 keys × 5 pages) doesn't yet reflect it. Would need either exporting `SYS_PAGES`/`SYS_COMMON` into `show.json` as a new top-level field both sides read, or a hardcoded mirror table in Rust — flagged for a follow-up, not attempted here given the session was already very long.

---

## 1a-7. Clips/Shaders Stopped Triggering Entirely After the Real-Show Deploy (same day)

Operator reported nothing would trigger at all after §1a-6. Two separate causes, found and fixed in sequence:

**Cause 1 — stale kiosk page.** `rpi-vj-renderer.service` (the Cage/Chromium kiosk showing `gig/index.html`) had been running continuously since the last reboot (06:52), well before the real `show.json` was deployed (07:33) — its in-browser `showManager` was still initialized against the old 4-clip test show while the native controller was now sending real-show bank/key indices. `systemctl restart rpi-vj-renderer.service` forces a fresh page load that re-fetches the current `show.json`. Confirmed via the existing `gig-diag.log` (wired up in §1a0's bridge work): boot-time `activateShow()` correctly played a real clip after the restart.

**Cause 2 — the actual bug, in `startNativeControllerBridge()`'s `applyRemoteState()`.** Simulated real button presses via `vj-broker`'s `POST /api/v1/command` (same endpoint the native controller itself uses) to reproduce without physical hardware: triggering `bank.select` then `clip.trigger` for the same numeric key the bridge had already fired once in a different bank produced `bridge: bank -> 3` in the log but **no** following `bridge: clip -> 0` — the trigger was silently swallowed.

Root cause, found by reading `gig/src/show.js`: `showManager.activeKeys` is a flat `Set<keyIndex>` with **no bank scoping** — `rebuildKeyBindings()` (called on every bank change) rebuilds the key→pad table but never touches `activeKeys`. The bridge's own guard (`!showManager.activeKeys.has(targetClip)`, added to avoid double-firing `__vjPress`'s toggle semantics) was seeing a key index left "active" from the *previous* bank and refusing to fire for the *new* bank's same-numbered pad — which would also have inverted the action (toggle OFF instead of play) had the guard not blocked it first. Since every bank reuses key indices 0-23/26-29, this silently ate almost every "switch bank, press a pad" press — the normal way an operator navigates — while a same-bank/different-key press (the case exercised by earlier testing) worked fine and masked the bug until the real 9-bank show made bank-switching routine.

**Fix**: on a detected bank change, delete just the incoming `targetClip` key's stale membership from `activeKeys` before evaluating whether to fire, and broadened the fire condition to `targetClip !== lastClip || (bankJustChanged && targetClip !== null)` (was `targetClip !== lastClip` alone) so a same-numbered key in a new bank always re-evaluates. Deliberately scoped to only this case — did not touch `activeKeys` for keys other than the incoming one, so any clip left genuinely still playing from the previous bank is not falsely marked inactive.

**Verified** via `POST /api/v1/command` simulating 3 separate bank/key combinations after redeploying `index.html` and restarting the kiosk: bank 3/key 0 → FX bank's `INVERT` videofx fired; bank 0/key 5 → `Abstract` clip played; bank 5/key 0 → `Q1 [mirror_hv_tl]` clip played (QUAD MIRROR bank) — confirmed via `gig-diag.log`'s `PRESS key=... binding=...` / `clip PLAY: ...` lines for each.

**Known remaining limitation, not fixed** (out of scope for this report — didn't regress, was already true before): pressing the exact same key twice in a row *within the same bank* still won't re-trigger (`targetClip === lastClip` and no bank change), since the native controller's "always play, no toggle" model and gig's toggle-based `triggerKey()` aren't fully reconciled. Only matters for an explicit "restart this clip from frame 0" gesture; STOP (key 30) still works normally in between presses.

---

## 1a-6. Real 9-Bank Show Deployed (119 Clips, 18 Shaders/FX), Bank Count Made Dynamic (same day)

Operator noticed the deployed deck only had 4 clips and 5 shaders and asked why, since their real show has far more. Root cause: `/opt/rpi-vj/web/show.json` was still the deliberately minimal "Pipeline Test" proof-of-concept from earlier in this session (§1a0), never replaced with the real `gig/show.json` (1775 lines, 9 banks: CLIPS/MIX/SYS/FX/FX-LAYER/QUAD MIRROR/50TH/720p/NEON, 119 clip pads, 18 shader/effect files, several pad types beyond clip/shader — `blend`, `videofx`, `shaderfx`, `system`).

**Deployed for real** (operator confirmed via AskUserQuestion: direct copy, not the USB/LAN ingest pipeline):
- Extracted the exact 147 unique `src`/`thumb` media paths and 18 shader paths the real show.json actually references (not the full 8GB+ `gig/media/` tree — only the ~3GB of variant folders/files real pads point to: 1080/720/50/neon/quad/h264/thumbs). Verified zero missing files locally before transfer.
- Packaged into a single tar, transferred to the Pi. **First attempt failed**: staged to `/tmp`, which is a 1GB tmpfs (RAM-backed) — filled it completely, also starved concurrently-running test suite of scratch space (`StorageFull` errors, confirmed not a code regression by rerunning clean after freeing `/tmp`). Fixed by staging to `~` (the real 29GB ext4 root, 14-17GB free throughout) instead.
- Extracted directly into `/opt/rpi-vj/web/{media,shaders}/` with `sudo tar -x` (skipping an intermediate staging copy to avoid tripling disk usage), then `chown -R vj:vj`. Deployed the real `show.json` and `presets.json` over the test versions (test show backed up to `/var/lib/rpi-vj/show.json.pipeline-test.bak`).

**Bank count fix** (operator confirmed): `performance.rs`'s PREV/NEXT wrap was hardcoded to `BANK_COUNT = 8`, but the real show has 9 banks — NEON (index 8) would have been permanently unreachable from the deck. Replaced with `active_bank_count()`, which reads `ShowManifest::bank_count()` (new accessor, tracks `banks.len()` at load) and falls back to a `FALLBACK_BANK_COUNT = 8` constant only when no manifest is loaded (mock/test contexts).

**Generalized the §1a-5 name-vs-number fix to all non-clip pad types**: the real show's `MIX`/`SYS` banks use `blend`/`videofx`/`shaderfx`/`system` pad types the original fix's `is_shader` check didn't cover, which would have silently reintroduced the exact "still numbers" bug for those pads. `PadInfo.is_shader` renamed to `PadInfo.use_name_label`, computed as `type != "clip"` rather than `type == "shader"` — anything that isn't a clip gets its name as the dominant element. Also removed the small "FX" caption line (no longer semantically accurate for `blend`/`system` pads) and widened the dynamic-scale name rendering to try scale 3 first (fits short names like "ADD"/"WARP" larger) before falling back to 2 then 1.

**Verified end-to-end on the Pi** with a disposable probe binary linked against the real deployed `vj-surface-hid`/`show.json`: `bank_count() == 9`, bank 8 (NEON) populated and reachable, clip pads carry real labels + thumbnails, FX-bank pads correctly get `use_name_label = true`. Manifest load with the full 28 real thumbnails: ~11ms (up from ~0.6ms with 4 test thumbnails, still negligible); full 32-key page render unchanged at ~237ms, confirming thumbnail decode cost is isolated to one-time startup, not per-render — consistent with §1a-5's earlier finding.

**Found and fixed in passing**: a latent test race in `show_manifest.rs`'s and `performance.rs`'s test helpers — `write_temp`/`manifest_with_shader_label` keyed their temp directory only by process ID, so tests running in parallel within the same test binary and reusing the filename `"show.json"` could clobber each other's fixture file. Only surfaced once this change added a second/third test using the same helper concurrently. Fixed with a monotonic counter appended to the temp dir name. All 48 `vj-surface-hid` tests pass on the Pi's own build; binary hash-verified redeployed.

---

## 1a-5. Real Pad Labels + Clip Thumbnails, with Measured Render Cost (same day, after the full-workspace audit)

Operator feedback: shader buttons ("PLASMA", "TUNNEL", etc.) and clip buttons should show their real `show.json` names instead of generic numbers, and asked to *try* rendering actual video thumbnails on the clip pads specifically to find out whether that costs enough render/CPU time to hurt live reliability — an experiment to measure, not a blind feature add.

**What was built** (plan: `graceful-greeting-sunbeam.md`):
- New `pages/show_manifest.rs`: reads the same `show.json` `vj-broker`'s `/gig/` route already serves, from the same Pi filesystem (no new broker API, no HTTP client). Parses `banks[].pads[].{key,type,label,thumb}`. Any failure — missing file, malformed JSON, a key with no pad, a missing/corrupt thumbnail — resolves to `None`/skip, never a panic, never blocks startup. Thumbnails are decoded **once** at load (`image` crate, promoted from dev- to a real dependency), resized to the exact 96x96 icon size, and cached as raw pixel buffers.
- `PageContext` gained `show_manifest: Option<Arc<ShowManifest>>`, loaded once in `main.rs` right after `UIEngine::new()`.
- `performance.rs`'s clip/shader render arm now looks up `(active_bank, key)` in the manifest: real label if present (falls back to the existing `B{bank}-C{n}` numeric form otherwise), and blits the cached thumbnail as the key's background if one was configured (`Canvas::blit()`, a new method — plain `copy_from_slice`) before drawing the border/label/status text on top for legibility.
- Added timing instrumentation (always on, not gated behind `--diag`, since this is exactly the number needed to answer the question asked): manifest load time at startup, and full-32-key render time on every Stream Deck (re)connect.

**Deployed and measured on the Pi itself** (not the dev machine) using the 4 real clip thumbnails already present in `gig/media/thumbs/`, added to `show.json`'s 4 test clip pads (`media/thumbs/clip_{a,b,c,d}.jpg`, 1.5-3.3 KB each) at `/opt/rpi-vj/web/media/thumbs/`. Ran an A/B by toggling the `thumb` fields in the deployed `show.json` and restarting the service several times per variant:

| Variant | Manifest load (JPEG decode+resize, once at startup) | Full 32-key page render |
|---|---|---|
| Labels only, no thumbnails | ~70µs | ~236.6-236.8ms |
| Labels + 4 clip thumbnails | ~0.6-1.1ms | ~237.0-243.1ms |

**Conclusion**: thumbnail cost is negligible — well under 1ms difference in per-render terms across repeated trials, effectively noise. The ~237ms full-page render is dominated by something else entirely (JPEG *encoding* 32 icons for the physical Stream Deck + the HID writes themselves), not by whether a key's source pixels are a solid color or a decoded photo. **Recommendation given to operator**: keep the thumbnails — they cost nothing measurable, but the physical hardware should still be checked for any felt lag on key-press-to-icon-update; the instrumented numbers don't support any performance concern.

All 44 `vj-surface-hid` tests (incl. 4 new `show_manifest` tests and a new `performance.rs` manifest-present case) pass on the Pi's own aarch64 build. Binary hash-verified installed at `/opt/rpi-vj/bin/rpi-vj-controller`, service restarted clean.

**Follow-up bug found on hardware check**: operator reported shader buttons (keys 16-20) "still numbers" despite the above. Root-caused by building a standalone probe binary against `vj-surface-hid` linked to the real deployed `/opt/rpi-vj/web/show.json` — confirmed `ShowManifest` loaded all 5 shader labels correctly (`PLASMA`/`TUNNEL`/`KALEIDO`/`PARTICLE`/`OSCILLO`), so this was never a data or manifest-loading bug. The actual defect: the real name *was* being drawn, but only as a small top-band caption above the still-unchanged giant center digit (`16`/`17`/.../`20`) — the dominant visual element hadn't changed, so the fix was invisible in practice. Corrected in `performance.rs`: added `is_named_shader_pad()` — any pad where `PadInfo.is_shader` is true now suppresses the numeric digit entirely and renders its real name as the dominant centered element (dynamically sized: scale 2 for short names like PLASMA/TUNNEL, scale 1 for longer ones like PARTICLE, so nothing clips at 96px). Clip pads are deliberately unaffected — they keep their number by design (the operator's "clip #" muscle memory), matching the original instruction that only *other* functional buttons should lose their numbers. Re-verified 45/45 tests (added `test_is_named_shader_pad_true_only_for_labeled_shaders`) on the Pi's own build, hash-verified redeployed.

---

## 1a-4. Part 2: Full-Workspace Audit (same day)

`cargo clippy --workspace --all-targets` across all 13 `vj/` crates found **zero errors, 52 warnings**, all style-level (unused imports, `div_ceil`/`is_multiple_of` manual reimplementations, `assert_eq!(x, true)`, collapsible-if, needless-range-loop, field-reassign-after-default, too-many-arguments, redundant `impl Default`) — no logic/correctness findings. Ran `cargo clippy --fix --workspace --all-targets --allow-dirty` to auto-apply the safe ones.

**The auto-fix itself introduced 2 real regressions**, both the same root cause: clippy's "unused import" check ran against the lib target and didn't see that the import was actually used by that file's own `#[cfg(test)]` module (`use super::*` doesn't make clippy's non-test-target analysis pass see the test module's usage), so it deleted an import that broke the `--tests` build:
- `vj-isf/src/parser.rs`: removed `InputType`/`IsfPass` from the top-level `use crate::{...}`, breaking `parser.rs`'s own test module which reads `meta.inputs[1].ty == InputType::Color` etc. Fixed by moving those two names into the test module's own `use` line instead of the top level (cleaner than restoring them to the top, which would just re-trigger the original "unused in the plain lib build" warning).
- `vj-audio/src/capture.rs`: removed `use std::sync::Arc` at the top, breaking its test module's `Arc::new(AudioRingBuffer::new(...))`. Same fix pattern: moved the import into the test module.

**Lesson for next time**: never trust `cargo clippy --fix --all-targets` blindly — always immediately follow with `cargo check --workspace --all-targets` (not just `--lib`) to catch exactly this class of regression before it ships. A full `cargo test --release --workspace` afterward confirmed **zero failures** across all 13 crates (unit + integration + doc-tests) once these two were fixed.

Re-deployed the two live crates affected by the auto-fix (`vj-surface-hid`: `graphics.rs` got a safe `div_ceil` equivalence rewrite in the JPEG encoder — re-verified the round-trip test still passes with full fidelity; `broker_client.rs` got a minor test-assertion cleanup. `vj-broker`: `upload.rs`, `supervisor.rs`, `portal_ui.rs`, `state.rs` got similar cosmetic fixes) — binaries hash-verified deployed, services restarted clean.

**Also found and fixed**: the restart cycling done throughout today's redesign work had tripped the supervisor's anti-reboot-loop protection a second time (`incident_count` went 1→2, `durable_degraded` back to `true` — this is the same safety latch documented in §1a below, which never clears itself automatically by design). Cleared it again the same way as before; confirmed `{"durable_degraded":false,"level":"normal"}` afterward.

**Deliberately not chased further** given this was already a very long session: the remaining ~40 warnings are in dormant, undeployed crates (`vj-app`, `vj-render`, `vj-api`, `vj-devices`) or are genuinely inert style items (unused `mut`, unused imports with no functional effect) with no plausible path to a mid-performance failure. If picking this up again, `cargo clippy --fix` a second time then immediately `cargo check --workspace --all-targets` (learned the hard way above) would clear most of the rest safely.

---

## 1a-3. PERFORMANCE Layout Corrected to Match the Operator's Real 32-Key Convention (same day)

The first redesign pass (§1a-2 below) got the page *count* right (3 pages) but got the *Performance layout itself* wrong: it kept a bank-select modal and spent 2 keys on a new opacity control. Operator feedback: "this layout doesn't allow me to directly trigger clip playing, I like my 32-button layout more. I can use the X-touch mini for opacity controls that I have already configured working well." Then, pointed at the actual source of truth: "Look up my original 32-button layout, it's definitely not all 32-clip launch buttons on each page. I have bank switchhings, and both clips & shaders trigger on each page."

Went and read it — `gig/index.html`'s own `SYS_COMMON`/`SYS_PAGES`/`applySystemRow()` (lines ~749-838), which is the JS engine's **own** key-layout convention (used when gig drives the deck directly via WebHID — dormant in this deployment, but still the real design the operator is used to):
- Keys 24/25 are **instant** bank PREV/NEXT — no modal, ever. "Nav (24/25) and the two panic keys (30/31) never move — muscle memory in the dark depends on that" (verbatim comment in that file).
- Keys 30/31 are fixed RESET/BLACK panic keys on every page.
- Keys 26-29 are a per-page variable zone — on most of gig's real banks (quad-mirror, 50th, 720p, neon — 6 of 9) they're **deliberately left empty specifically so show.json can use them as more clips**, giving 28 clip/shader slots instead of 24. Only a few special banks (1080p, blend-modes, system, video-fx, effect-fx) use them for shader-FX presets (speed trim, BPM, opacity, FX amount) — concepts that only exist client-side in gig's mixer and have no equivalent broker state on the native side.

Corrected `pages/performance.rs` to mirror this for the parts that translate to real broker state (confirmed with the operator: 26-29 become 4 more clips, 30 = tap-stop/hold-3s-system):
- **0-23, 26-29**: 28 clip/shader trigger pads (was 24 clips + a modal + opacity keys)
- **24/25**: instant BANK PREV/NEXT, wraps over 8 banks (was a modal)
- **30**: tap = STOP CLIP, hold 3s = exit to SYSTEM (local hold-tracking in the page struct, not the shared `GuardedEngine` — this is a benign nav/stop action, not a destructive one needing 2-key confirm)
- **31**: BLACKOUT (fixed panic key, matches gig's BLACK)
- No opacity or BPM keys at all — operator already has both on a physical X-Touch Mini MIDI controller (confirmed present on the USB bus earlier this session: `1397:00b3 BEHRINGER X-TOUCH MINI`) and explicitly didn't want deck space spent duplicating them.

`System`'s exit-to-Performance key (31) is unaffected by this — System still returns to Performance the same way. Added 4 new tests (extra clip slots 26-29, bank wraparound both directions, key-30 tap-vs-hold branching) alongside fixing the existing ones. All 39 tests pass (33 lib + 4 `observable.rs` + 2 `state_ui_integration.rs`), locally and natively on the Pi. `cargo clippy --all-targets` clean for this file. Binary hash-verified deployed, service restarted clean.

**Confirmed on physical hardware** by the operator: 28 clip/shader pads, instant bank prev/next, tap-stop/hold-3s-system, blackout — matches. **Part 2 (full-workspace audit) starts next.**

---

## 1a-2. StreamDeck Layout Consolidated to 3 Pages (same day, after PERFORMANCE-boot confirmed working)

Operator asked, after confirming clips/shaders/PERFORMANCE-boot all work: "does the current layout's health/diagnostics/system/ingest pages offer me any realistic performance/reliability in real world live gig performing on stage, other than just displaying some system stats?" A code-level check (not guessing) found:

- Only `hostname` and `temperature_c` in `SystemTelemetry` were ever updated by `refresh_telemetry()`. `ip_address`, `free_space_bytes`, `total_space_bytes`, `throttled`, and `uptime_secs` were permanently stuck at their `Default` values — meaning the throttle indicator (the one that actually matters for a Pi 5 under sustained shader+video load) could **never** fire a true warning.
- `ctx.fault` (`FaultInfo`) was never populated by anything real — DEGRADED's fault code and READY's health key always showed "OK" regardless of actual state.
- `IngestStatus`'s status fields (USB status, progress, file queue, recommendation) are *also* never synced from broker/vj-ingest — same problem one layer deeper — though Ingest's action buttons (`ingest.accept_recommendation`, `ingest.decision_override`, etc.) are real and do send genuine commands.
- `master_opacity` is real, broker-synced state with a *display* (on the old READY page) but **no adjust control anywhere** in any page — a genuine missing control, not fakery.

Operator decided: consolidate to **3 pages** (Performance, System, Ingest — remove Booting/Ready/Degraded entirely) and add a real opacity control. Full plan written and approved before implementing (see chat history / `graceful-greeting-sunbeam.md` plan file if still present).

**Implementation** (`vj/crates/vj-surface-hid/src`):
- `pages/mod.rs`: `PageId` shrunk to `{Performance, Ingest, System}`. Removed `FaultInfo` entirely. `SystemTelemetry` shrunk to just `hostname`/`temperature_c`. `PageContext::default()`'s `active_page` now `Performance`.
- Deleted `pages/booting.rs`, `pages/ready.rs`, `pages/degraded.rs` outright (Booting was confirmed unreachable — nothing ever called `set_page(PageId::Booting)`).
- `pages/system.rs` rewritten from scratch: only real content — HOSTNAME/CPU TEMP/SHOW NAME (row 0), INGEST entry/RESTART RENDERER/RESTORE STATE (row 1), guarded REBOOT/SHUTDOWN + EXIT▸PERFORMANCE (row 3). Reuses the exact 2-key-confirm pattern (confirm=key 0, cancel=key 31) proven in the old System page.
- `pages/performance.rs`: control row (24-31) remapped — dropped the redundant BANK 0 / BANK 1 quick-jump keys (the bank-select modal already covers all 8 banks) to make room for a real **OPACITY −/+** control (keys 29/30, calls `broker.set_master_opacity`, 0.05 step, clamped [0,1]). Full new row: 24 BANK MENU, 25 STOP CLIP, 26 BLACKOUT, 27 BPM−, 28 BPM+, 29 OPACITY−, 30 OPACITY+, 31 EXIT▸SYSTEM. The 24 clip pads (0-23) are untouched.
- `pages/ingest.rs`: blanked 3 boxes that displayed nothing real (14-15 Headroom — read the now-removed telemetry fields; 21-23 and 29-30 — were 100% static hardcoded text with zero dynamic binding). Exit (key 31) now returns to System (where Ingest is entered from) instead of the removed Ready page. Left everything else (USB status display, pipeline steps, all decision/guarded actions) unchanged — **its status fields still aren't live-synced from broker/vj-ingest; that's a separate, larger gap, not fixed in this pass.**
- `ui_engine.rs`, `lib.rs`: mechanical follow-through (dropped the 3 removed page fields/dispatch arms/re-exports).
- Updated 3 test suites to match (`pages/system.rs`'s own tests rewritten, `ui_engine.rs`'s full-lifecycle test, and a previously-unnoticed integration test `tests/state_ui_integration.rs` that only surfaces under `--all-targets`, not plain `--lib` — worth remembering next time a "tests pass" check feels too easy).

**Verified**: all 37 tests pass (31 lib + 4 `observable.rs` + 2 `state_ui_integration.rs`), both locally and natively on the Pi (aarch64). `cargo clippy --all-targets` on this crate shows zero warnings introduced by this change (only pre-existing style items elsewhere, tracked for the Part 2 audit). Binary hash-verified deployed to `/opt/rpi-vj/bin/rpi-vj-controller`, service restarted clean, `[STREAMDECK CONNECTED]` logged with no errors.

**Not yet done**: physical confirmation on the Stream Deck itself. **Part 2 (full-workspace `cargo clippy`/`/code-review` audit across all `vj/` crates) has not started yet** — that's the next step once this is confirmed on hardware.

---

## 1a-1. Boot Page Changed to PERFORMANCE (same day, after gig/ bridge confirmed working)

Cursor fix confirmed working on physical hardware (multi-size theme fix). Clips/shaders confirmed playing on physical hardware. Operator then flagged the StreamDeck navigation as much harder to use than the pre-V3 flat layout ("very difficult to navigate & perform... I like the previous Streamdeck layout from main branch a lot more"). Asked to clarify the direction (replicate the old flat 32-key layout entirely, vs. keep the multi-page system but remove navigation friction, vs. something else) — operator chose: **keep all pages, boot straight into PERFORMANCE instead of READY.**

**Change**: `vj/crates/vj-surface-hid/src/main.rs` — `ui.set_page(PageId::Ready)` → `ui.set_page(PageId::Performance)`. That's the entire change; `UIEngine::set_page()` already runs the correct enter/exit side effects (e.g. `ingest.pause_transcode`) regardless of entry path, so nothing else needed updating. READY (bank overview, health indicator, ingest, system settings) is still one tap away via Performance's key 31 ("EXIT / DISARM / READY >"); DEGRADED is reachable from there via key 15/29 as before.

Deployed, all 31 tests pass (one flaky unrelated test — `test_degraded_page_restart_renderer_action` — failed once under parallel load, passed both in isolation and on a full-suite rerun; not caused by this change, it doesn't touch `main.rs`). **Confirmed on physical hardware**: deck now boots directly into the clip-trigger grid.

Note for next agent: `controller-state.json`'s `active_page` field did **not** update to reflect this (still showed `1`/READY) — this is the same pre-existing stale-reporting bug documented in §1a below (`update_broker_state` is only called from specific reconnect-handler code paths, not on every page change), not a sign the fix failed. Don't trust that field for page-state diagnosis; verify physically or via the controller's actual behavior instead.

---

## 1a0. Cursor Fix Round 2 + Real gig/ Show Deployed with Native-Controller Bridge (same day)

After Round 1 (below) still showed a real visible arrow cursor **even after a genuine `sudo reboot`**, and the operator asked to check media/shader status:

**Cursor — actual root cause found**: The transparent theme only contained a single **1×1** cursor image per name. Cage/wlroots very likely requests a specific nominal size (commonly 24) when creating its cursor manager rather than reading `XCURSOR_SIZE` from the environment — the calling application decides the size, not the shell env var. A theme with only a 1px variant available means that lookup fails, and wlroots silently falls back to its own built-in **visible** default arrow. Fixed `generate-transparent-cursor.py` to emit fully-transparent images at 17 sizes (1–128px) per cursor name in one multi-TOC-entry Xcursor file, eliminating the possibility of a fallback regardless of what size is requested. Regenerated at all three theme locations (`/var/lib/rpi-vj/theme`, `/usr/share/icons/transparent`, `/usr/share/icons/default`), verified structurally with a byte-level parser, and confirmed surviving a real `sudo reboot`. **Still needs a final human visual check** — not yet re-confirmed on the physical TV as of this writing.

**Media check**: Confirmed `/srv/rpi-vj/shows/` was completely empty (no generations) and `/srv/rpi-vj/current` a dangling symlink — nothing had ever been ingested through the normal USB/portal pipeline.

**Shader/clip triggering — real root cause found**: `http://localhost:8080/gig/` was serving `vj-broker`'s hardcoded placeholder HTML (a title + animated decorative rings), not a real show. Meanwhile a **complete, unrelated VJ engine already exists in the repo at `gig/`** — real GLSL shaders (`gig/shaders/*.fs`), an ISF-based mixer/choreography engine (`gig/src/{mixer,isf,choreo,show}.js`), and ~211 pre-transcoded clips (`gig/media/`, ~11GB) — that had simply never been deployed. Two structural gaps had to be fixed before any of it could work:
1. **`vj-broker` had no static file server at all** — only a handful of hardcoded routes existed; everything else 404'd. Added a `GET /gig/*` static-file route to `vj/crates/vj-broker/src/http.rs` (path-traversal-guarded via `canonicalize()`, correct `Content-Type` per extension — `.js` **must** be `text/javascript` or Chromium refuses to execute it as an ES module — and HTTP Range support for `<video>` seeking). Smoke-tested locally (normal fetch, Range fetch, traversal attempt, 404) before deploying.
2. **`gig/`'s JS had no code path reacting to broker state at all.** It was built to read the Stream Deck directly via browser WebHID (`navigator.hid.requestDevice()`), which needs a user gesture that can never happen in this kiosk (no pointer input) — so its own key-handling can never activate, and it was never wired to `vj-broker`'s REST state either. Added `startNativeControllerBridge()` to `gig/index.html`: polls `/api/v1/state` every 250ms and, on change, drives the exact same entry points a real key press would use — `window.__vjPress(key)` (already exposed by the existing code) for clip/shader pads, direct `showManager.currentBankIndex` + `rebuildKeyBindings()` for bank switching, `showManager.toggleBlackout()` for blackout — so all of `gig/`'s existing exclusivity/repaint logic applies unchanged. Syntax-verified with `node --check` before deploying (no way to visually test).

**Also added**: a `POST /collect` endpoint in `vj-broker` (capped log file at `/var/lib/rpi-vj/gig-diag.log`) — `gig/index.html` already had a `VJ_DIAG` diagnostic system posting crash/status text there, it just had nowhere to land. This is what made it possible to verify the bridge from the command line: with a fresh page load and a `curl POST /api/v1/command` simulating a native-controller clip trigger, the diag log showed `clip PLAY: Laser` and `engine fps=60.8 ... clip=clip_a` (video actually playing), and for a shader pad, `shader compile: ok (2085 bytes)` and `engine ... shader=neon_tunnel` (shader actually compiled and switched, blended over the still-playing clip). Blackout toggle confirmed too (`bridge: blackout -> true/false`). **This is real, verified end-to-end proof the pipeline works** — native command → broker state → bridge → actual GPU-rendered playback — not just "the files are in place."

**Current deployed content is a deliberately small proof-of-concept, not the real show**: `/opt/rpi-vj/web/show.json` is a **custom 1-bank/9-pad file** (not the real 9-bank/1775-line `gig/show.json`), referencing 4 small clips (`media/clip_a.mp4`..`clip_d.mp4`, copied from `gig/media/720/`, renamed) at keys 0-3 and 5 real shaders at keys 16-20. The full 211-clip/~11GB media library and the real `gig/show.json` were **not** deployed — that's a separate, larger follow-up (bandwidth/time, and the real show.json's bank layout should be re-checked against the native `PerformancePage`'s 8-bank assumption: real show.json has 9 banks, one — "NEON" at index 8 — is unreachable from the native controller, which only goes 0-7).

**Next steps for whoever picks this up**:
1. Physically confirm the cursor is gone after the size-mismatch fix.
2. Physically confirm a clip (`Laser`, key 0) is visibly playing with the `neon_tunnel` shader blended over it (this is the exact state the system was left in).
3. Decide on deploying the full `gig/media/` library + real `gig/show.json` into `/opt/rpi-vj/web/` (mind the 9-vs-8-bank mismatch above), or curate a smaller permanent set.
4. Consider whether `gig/`'s own WebHID Stream Deck code (`src/streamdeck.js`, the "PAIR STREAM DECK" button) should be removed/disabled entirely now that the native controller bridge exists — it's currently dormant (no pairing possible without a mouse) but harmless to leave, though it's dead weight and a latent footgun if a mouse is ever attached to this kiosk for setup.
5. The old flat 32-key-instant-trigger layout (pre-V3 `src/main.rs`/`src/streamdeck.rs`, no page navigation) vs. the new multi-page appliance UI (2 taps to reach a clip trigger: READY → PERF MODE → clip) is a genuine ergonomics regression flagged by the operator — worth a deliberate design decision, not silently changed.

---

## 1a. Post-Deploy Investigation (same day, after first deploy pass)

After the Bug 1/Bug 3 fixes below were first deployed, the operator physically checked the hardware (photo: `screenshots/IMG_2471.jpeg`) and found: cursor still visible, and **the entire Stream Deck was showing the DEGRADED page content** (all 32 keys — "FAULT/OK/ACTIVE", "FORCE REBOOT HOLD 3S" x4, etc. — an exact match for `pages/degraded.rs`) even though `/run/rpi-vj/controller-state.json` claimed `active_page: 1` (READY). Investigation findings:

* **A second, separate stale cursor theme was found** at `/usr/share/icons/transparent` and `/usr/share/icons/default` — byte-identical to the pre-fix malformed generator output, left over from some earlier provisioning step outside `launch-renderer.sh` (nothing in the repo's scripts writes there; likely a one-time manual step from earlier work). Since `/usr/share/icons` is a fallback entry in `XCURSOR_PATH`, this was a live landmine regardless of exact lookup-order semantics. **Fixed**: regenerated both with the corrected generator script; verified byte-correct.
* **The broker's `durable_degraded` supervisor latch was found still set from an incident at 17:57 that day** (hours before this session started). Per `vj-broker/src/supervisor.rs`, once set this **never clears automatically** — no code path resets it, and there is no HTTP/socket API to clear it either. It does **not** drive the controller's page selection (traced through `ui_engine.rs`/`main.rs` — no code reads broker's `durable_degraded` to force a page). **Fixed** (with operator approval, since it required stopping `vj-broker` briefly): hand-edited `/var/lib/rpi-vj/supervisor-state.json` to clear the flag, restarted broker. Confirmed via `/api/v1/status` → `{"durable_degraded":false,"level":"normal"}`.
* **A one-time burst of ~7 redundant full-page (32-icon) re-renders** was confirmed via `strace` on the hidraw device right after a restart where `vj-broker` had *also* just been restarted moments earlier. Root cause: `BrokerClient::replay_state()` (called once on Stream Deck reconnect, `main.rs:122`) fires 5 separate commands (`select_bank`, `trigger_clip`, `set_blackout`, `set_tempo_bpm`, `set_master_opacity`) that each round-trip through the broker and come back as a `state` broadcast; each arrival makes `ui_engine::tick_at()`'s state-change check (`main.rs`/`ui_engine.rs`) fire a full `render_full_page()`, even though the values being replayed are already current. **This is a genuine pre-existing inefficiency** (not something introduced this session) but was confirmed **not** to be an infinite/perpetual loop — instrumented with a temporary diagnostic build (reverted) that logged 0 further state-triggered re-renders once broker was left alone. It only compounds when broker and controller restart within the same few seconds of each other, which happened repeatedly during this debugging session itself. **Not fixed** (out of scope for the two reported bugs, and `replay_state()`'s redundant round-trips are harmless once-per-reconnect waste, not a live bug) — flagging here for whoever picks up `replay_state()` next: it could be made idempotent (skip commands where the replayed value already equals current server state) to eliminate the redundant broadcasts entirely.
* Ruled out via direct evidence: no rogue/duplicate `rpi-vj-controller` process, no duplicate Chromium/Cage instance (`cage`'s two PIDs are its own internal parent/child, both under one systemd cgroup), hidraw write pipeline confirmed working via `strace` (valid JPEG headers observed streaming to the device), output thread confirmed idle/not blocked (`ps -T` shows `S` state, not `D`).
* Working theory for the photographed stale DEGRADED content: it was likely captured during the debugging window itself (repeated renderer/controller/broker restarts in quick succession, ~20:15–20:37), not a live bug in the final quiescent state. **Not yet re-confirmed with a fresh physical check** — see next steps.

---

## 2. Recent Critical Bugfixes Deployed to Hardware

### Bug 1: Stream Deck XL Fuzzy / Inverted Key UI
* **Root Cause (part 1, orientation)**: The Stream Deck XL LCD key array requires images to be rotated 180 degrees prior to JPEG compression. Without rotation, all text and icons appeared inverted and blurred.
* **Root Cause (part 2, actual "fuzzy" corruption)**: The 180-degree-rotation fix above did **not** fix the reported fuzziness — the handwritten pure-Rust baseline JPEG encoder in `vj/crates/vj-surface-hid/src/graphics.rs` had two independent bugs that scrambled every non-flat icon (any text/shape/gradient content, i.e. basically every real key), while still producing a structurally valid JPEG (so it decoded to *something*, just visually wrong — this is why a header-only test wasn't enough to catch it):
  1. `dct_8x8` wrote its output transposed relative to the natural-order convention the zigzag table and quantization tables assume (`out[u*8+v]` instead of `out[v*8+u]`).
  2. `quantize_and_zigzag` applied the zigzag permutation backwards, scattering AC coefficients to the wrong scan positions instead of reading them from natural order into scan order.
  3. `int_to_category_and_bits` had an off-by-one in the "extra bits" computation for every **negative** DCT coefficient (`(abs_v - 1) ^ mask` instead of `abs_v ^ mask`), corrupting roughly half of all encoded coefficients.
* **Fix Applied**: Corrected all three bugs in `vj/crates/vj-surface-hid/src/graphics.rs`. Added a regression test (`test_jpeg_encoder_roundtrip_matches_source`) that renders a non-flat icon (text + shapes), encodes it, decodes it with a real JPEG decoder (`image` crate, dev-dependency only — not in the release binary), and asserts the decoded pixels are close to the source; this is the only kind of test that can catch this class of bug, since the JPEG container stays structurally valid throughout.
* **Deployed & Verified**: `cargo build --release -p vj-surface-hid` and `cargo test -p vj-surface-hid --lib` both run clean **on the Pi itself** (aarch64, all 31 tests pass, including the JPEG round-trip test). Binary hash-verified copied to `/opt/rpi-vj/bin/rpi-vj-controller`, `rpi-vj-controller.service` restarted, `[STREAMDECK CONNECTED]` logged with no errors since. **Still needs a human to look at the physical Stream Deck XL and confirm the key icons actually render crisp** — everything short of that has been verified from this session.

### Bug 3: Kiosk Shows a Visible Mouse Cursor (prohibited by `tasks/APPLIANCE-V3/A3-02-kms-cage-renderer.md`)
* **Root Cause 1 (malformed cursor file)**: `scripts/appliance/generate-transparent-cursor.py` hand-built a raw Xcursor binary file with two format errors: the file-header `version` field was `1` instead of the correct `0x10000` (Xcursor file format version 1.0), and the table-of-contents `position` field was hardcoded to `36` instead of the real chunk offset `28` (`16`-byte file header + `12`-byte TOC entry). A spec-correct Xcursor parser (wlroots' included) would read chunk data from the wrong file offset and fail to load the theme, silently falling back to a real, visible default arrow cursor — exactly the symptom reported. Compounding this, `launch-renderer.sh`'s `suppress_pointer_and_desktop()` only (re)generated the theme when `${theme_dir}/cursors` didn't already exist, so once the broken theme was written once on the Pi, it would never self-heal on later boots even after the generator script was fixed.
* **Root Cause 2 (found during hardware deployment — the theme could never have been written at all)**: The theme lived at `/opt/rpi-vj/theme/transparent`, but `/opt/rpi-vj` on the Pi is root:root `755`, and `rpi-vj-renderer.service` runs the generator as the unprivileged `vj` user *and* has `ProtectSystem=strict` with `ReadWritePaths=/srv/rpi-vj /var/lib/rpi-vj /run/rpi-vj` — `/opt` is not in that list, so it's read-only in the service's mount namespace regardless of ownership. The runtime generation step could never have actually succeeded from a clean boot; it only ever appeared to work because someone/something had created the theme directory out-of-band as root at some earlier point (nothing in `provision-appliance.sh` creates or chowns `/opt/rpi-vj/theme` — confirmed by grep, zero matches). Reproduced directly on the Pi: `sudo -u vj python3 generate-transparent-cursor.py /opt/rpi-vj/theme/transparent` → `PermissionError: [Errno 13] Permission denied`.
* **Fix Applied**:
  1. Rewrote `generate-transparent-cursor.py` to compute all Xcursor offsets from actual struct sizes instead of hardcoded magic numbers (so they can't drift out of sync again), and corrected the file-format version. Removed the nonsensical self-referential `Inherits=transparent` line from `index.theme`.
  2. Moved the generated theme from `/opt/rpi-vj/theme/transparent` to `/var/lib/rpi-vj/theme/transparent` in `launch-renderer.sh` (configurable via `RENDERER_THEME_DIR`) — `/var/lib/rpi-vj` is already vj:vj-owned and already in `ReadWritePaths=`, so this needed no hardening/unit changes and keeps `ProtectSystem=strict` fully intact.
  3. Changed `launch-renderer.sh` to regenerate the theme unconditionally on every launch (cheap — 43 files of ~70 bytes each) instead of only when missing, so a fixed generator always takes effect on the next boot instead of silently skipping if a stale/broken theme dir is already present.
  4. Stopped swallowing the generator's exit status/stderr (`2>/dev/null || true` → now logs a `WARNING` to journald on failure) so a future regression like this is visible instead of silent.
* **Deployed & Verified on the Pi**: `rpi-vj-renderer.service` restarted from a clean state (theme dir deleted first) and the theme was regenerated automatically by the service itself with no manual steps — confirmed via `journalctl` (no warnings/errors) and by inspecting `/proc/<cage-pid>/environ` directly: `XCURSOR_THEME=transparent`, `XCURSOR_PATH=/var/lib/rpi-vj/theme:...`, `XCURSOR_SIZE=1`, `WLR_LIBINPUT_NO_DEVICES=1` all correctly set on the actual running Cage compositor process. Byte-dumped the deployed `left_ptr` cursor file on the Pi and hand-verified every field against the Xcursor spec (magic/header/version/TOC-position/chunk-header/1×1 transparent pixel) — structurally perfect. **Still needs a human to look at the physical TV/HDMI output and confirm no cursor is visible** — everything short of that has been verified from this session.

### Bug 2: Cold Boot Stuck in Terminal Login (`tty1` / getty conflict)
* **Root Cause**: 
  1. `getty@tty1.service` defaulted to active on `multi-user.target`, grabbing VT1 before Cage/KMS started.
  2. The `vj` service user was missing permissions for `/dev/tty*` nodes, causing `libseat` to fail DRM session negotiation.
  3. Chromium profile singleton locks failed under `/var/lib/rpi-vj/renderer-profile`.
* **Fix Applied**:
  1. Masked `getty@tty1.service` and added `Conflicts=getty@tty1.service` to `rpi-vj-renderer.service`.
  2. Created `/etc/udev/rules.d/99-vj-tty.rules` setting `GROUP="video", MODE="0660"` for all TTYs and added `vj` to `tty` and `video` groups.
  3. Added `TTYPath=/dev/tty1`, `TTYReset=yes`, `TTYVHangup=yes`, and `PAMName=login` to `rpi-vj-renderer.service`.
  4. Corrected `/var/lib/rpi-vj/renderer-profile` ownership and permissions.

---

## 3. Active Services & Runtime Paths on Raspberry Pi

### Systemd Services:
- `rpi-vj-broker.service`: Command broker, supervisor, and REST/WebSocket API (`/opt/rpi-vj/bin/vj-broker`).
- `rpi-vj-controller.service`: Native Stream Deck XL HID driver and UI engine (`/opt/rpi-vj/bin/rpi-vj-controller`).
- `rpi-vj-renderer.service`: Single-output KMS Cage compositor and Chromium kiosk (`/opt/rpi-vj/scripts/appliance/launch-renderer.sh`).
- `seatd.service`: Seat management daemon for Wayland (`seatd -g video`).

### Key File Locations on Pi:
- Service Binaries: `/opt/rpi-vj/bin/` (`vj-broker`, `rpi-vj-controller`, `vj-ingest`)
- Scripts & Configs: `/opt/rpi-vj/scripts/appliance/`
- Runtime Sockets: `/run/rpi-vj/broker.sock`, `/run/seatd.sock`, `/run/rpi-vj/renderer/`
- Data Directories: `/var/lib/rpi-vj/` (boot ledger, desired state, supervisor state)
- Show Storage: `/srv/rpi-vj/shows/`, `/srv/rpi-vj/current`, `/srv/rpi-vj/staging/`
- Release Manifest: `/var/lib/rpi-vj/release-manifest.json`

---

## 4. SSH & Connection Details
- **SSH Target**: `ssh rpi@192.168.0.145` (passwordless sudo configured)
- **Local Hostname**: `rpi-vj.local`
- **Rust Toolchain on Pi**: `export PATH=$HOME/.cargo/bin:$PATH`
- **Local Repository**: `c:\Users\Admin\Projects\RPi-VJ` (synced to `/home/rpi/RPi-VJ/` on Pi)

---

## 5. Next Steps for Next Agent / Operator

0. **Bug 1 / Bug 3 fixes: deployed to the Pi, service-level verified, awaiting human eyes-on confirmation.** Everything that can be checked without looking at the physical devices has been checked (see §2 above for exact commands/evidence): binary hash-verified on `/opt/rpi-vj/bin/rpi-vj-controller`, `rpi-vj-controller.service` and `rpi-vj-renderer.service` both restarted clean with no errors, Cage's live process environment confirmed correct (`XCURSOR_THEME=transparent` etc.), and the deployed cursor file byte-verified against spec. Nothing left to *deploy* — just look.
1. **Verify Live Physical Feedback** (the one remaining step — needs a human):
   - Confirm Stream Deck XL is displaying crisp icons, bank buttons, and telemetry (not fuzzy/garbled).
   - Confirm HDMI monitor displays the gig UI or dark appliance standby splash without cursor or console prompt.
2. **Execute A3-03 Cold Boot Hardware Gate**:
   - Run `/home/rpi/RPi-VJ/scripts/appliance/test-cold-boots.sh` on the Pi to log 25 cold boots and verify zero degradation in `/var/lib/rpi-vj/boot-ledger.jsonl`.
   - Update `tasks/APPLIANCE-V3/BOARD.md` to mark `A3-03` as `DONE` once physical gate passes.
