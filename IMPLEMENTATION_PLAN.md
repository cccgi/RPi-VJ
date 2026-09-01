# RPi-VJ — Implementation Plan (handoff doc)

**Audience:** whoever picks this up next to carry it from scaffold to a
running system on real hardware. Written to be readable without the
conversation that produced it.

**Read order:** this file for the plan and priorities → `docs/RESEARCH.md`
for the "why" behind every design choice → `docs/STATUS.md` for exactly
what's verified vs. not → the code itself, which is heavily commented with
the same reasoning inline.

**What exists right now:** a Rust Cargo workspace that compiles clean
(`cargo check`/`cargo build --workspace` both pass) against the real
`elgato-streamdeck`, `pipewire`, `tokio`, and `image` crate APIs, structured
into four crates plus systemd/provisioning scripts. It has **not** run on a
Raspberry Pi, a Stream Deck XL, or a USB audio interface — none of that
hardware was available in the environment this was built in. Treat it as a
correctly-shaped starting point, not a finished product.

---

## 1. System architecture

```
                         ┌─────────────────────────────┐
                         │         vj-core (bin)        │
                         │  orchestration / main loop    │
                         └───────────┬───────────────────┘
                    ┌────────────────┼────────────────────┐
                    │                │                     │
            ┌───────▼──────┐ ┌───────▼───────┐   ┌─────────▼─────────┐
            │ vj-streamdeck │ │   vj-video     │   │      vj-audio      │
            │  (HID input)  │ │ (mpv/DRM out)  │   │ (PipeWire capture) │
            └───────┬───────┘ └───────┬────────┘   └─────────┬──────────┘
                    │                 │                       │
              USB HID (32 keys)  mpv subprocess          PipeWire graph
              elgato-streamdeck  + JSON IPC socket        (own thread)
                                 --gpu-context=drm
                                        │
                                        ▼
                                  HDMI via DRM/KMS
                                (no compositor)
```

Single-daemon, four-crate workspace. `vj-core` is the only binary; the
other three are libraries it links against. There is no IPC between
crates — they're linked into one process and talk through Rust function
calls / channels, *except* `vj-video`, which itself spawns mpv as a
subprocess and talks to it over a Unix-socket JSON IPC protocol (see
§2.1 for why that specific boundary exists).

### 1.1 Non-negotiable hardware constraint driving every decision

**The Pi 5 (BCM2712) has no H.264 hardware decoder — only HEVC.** This one
fact is why clips must be HEVC-encoded, why `vj-video` defaults to
`--hwdec=drm` for HEVC and treats H.264 as a software-decode fallback, and
why `scripts/encode_clip.sh` exists. Do not "simplify" the design by
assuming H.264 hardware decode works on this SoC — it doesn't, full stop.
Full citations and reasoning: `docs/RESEARCH.md` §1.1.

### 1.2 Why mpv-as-subprocess, not libmpv-in-process

Crash isolation. mpv occasionally faults at the DRM/EGL driver level on
embedded GPUs; a subprocess boundary means that fault kills mpv, not the
Stream Deck/audio pipeline, and `vj-core` can respawn it in under a second
(`respawn_player()` in `main.rs`, triggered by `player.is_alive()` == false
after a failed command). The IPC round-trip cost (sub-millisecond over a
Unix socket) is irrelevant against the 50ms trigger budget, which is
dominated by decoder/pageflip latency, not control-plane overhead. Full
comparison table: `docs/RESEARCH.md` §1.2.

### 1.3 Why PipeWire's own mainloop runs on a dedicated OS thread

PipeWire's C mainloop is not tokio-reactor-compatible; `vj-audio::capture`
spawns it on its own `std::thread` and bridges to the async world with a
plain `std::sync::mpsc` channel drained via `spawn_blocking`. This is the
documented supported integration pattern for `pipewire-rs`, not a scaffold
shortcut — keep it this way rather than trying to force PipeWire's
callbacks onto the tokio runtime.

---

## 2. Module-by-module implementation status and next steps

### 2.1 `vj-video` — mpv/DRM-KMS player + clip preload

**Files:** `lib.rs`, `ipc.rs`, `player.rs`, `preload.rs`, `raw_drm.rs`

| Piece | Status | Next step |
|---|---|---|
| mpv process spawn + flag set | Written, matches documented mpv `--gpu-context=drm` usage | **Validate on real Pi 5 hardware first** — confirm `--drm-device`/`--drm-connector` autodetection actually finds the HDMI-owning card node (do not assume `/dev/dri/card0`, see RESEARCH.md §1.2) |
| JSON IPC client (`ipc.rs`) | Compiles clean, connect-with-retry + request/reply correlation implemented | Should work as-is; if mpv's IPC protocol version drifts, re-check against `mpv --input-ipc-server=... ` live before assuming a bug here |
| Clip preload / page-cache warming (`preload.rs`) | Implemented (`posix_fadvise` + forced sequential read) | Works as designed; consider adding a size cap / LRU eviction if the clip bank grows past what comfortably fits in RAM (currently warms the *entire* bank unconditionally — fine for ~32 clips at ~20MB, will need rework for a much larger library) |
| Two-plane crossfade | **Not implemented, deliberately** | If crossfades become a requirement, this is the biggest single piece of new work in the project — see RESEARCH.md §1.3 for the design sketch (two DRM overlay planes or an offscreen-FBO compositing step). Budget real time for this; it's not a small addition. |
| `raw_drm.rs` (Alternative B) | Typed skeleton only, **not implemented** | Only build this out if profiling on real hardware shows the mpv-subprocess path can't hit the latency budget, or if a future requirement (e.g. in-process shader compositing of decoded frames) needs it. Don't build it speculatively — see RESEARCH.md §1.4 for the cost/benefit. |

**Immediate priority for next implementer:** get mpv actually running on a
Pi 5 with `--vo=gpu-next --gpu-context=drm` and a real HEVC clip, confirm a
frame appears on HDMI, and measure actual trigger-to-frame latency with a
stopwatch/camera against the <50ms target before touching anything else in
this crate. Everything else here is design-for-a-target-we-haven't-hit-yet
until that one measurement exists.

### 2.2 `vj-streamdeck` — Stream Deck XL driver

**Files:** `lib.rs`, `device.rs`, `icons.rs`

| Piece | Status | Next step |
|---|---|---|
| Device discovery/connect (`open_first_xl`) | Implemented against real `elgato-streamdeck` 0.7.x API (`Kind::Xl`/`Kind::XlV2`) | Should work as-is once a real device is plugged in; verify the udev rule (`systemd/60-streamdeck.rules`) actually grants access without `sudo` |
| Auto-reconnect | Implemented (backoff loop + read-error-triggered reconnect) | Validate real unplug/replug behavior — the scaffold's assumption about what hidapi returns on unplug (`is_timeout` heuristic) is untested against a real disconnect |
| **Button press/release diffing** | **Known incomplete — flagged in code** | `device.rs`'s `poll_once()` currently emits a `Pressed` event for every `true` in the raw state array on every poll tick, not just on the 0→1 transition. **This will double/triple-fire clip triggers as written.** Fix: track a `[bool; 32]` (or `elgato-streamdeck`'s own previous-state helper if the crate has one) across calls and only emit on actual edges. This is the single most important correctness fix before this crate is usable. |
| Dynamic icon rendering (`icons.rs`) | Implemented, compiles against real `set_button_image(DynamicImage)` signature | Works structurally; the composited icon (thumbnail + status border) has not been visually verified on a real key's LCD — check aspect ratio/rotation looks right on hardware, `elgato-streamdeck` is supposed to handle per-model rotation internally but verify |
| Icon updates wired into main loop | **Not wired up** — flagged with a `TODO` in `vj-core/src/main.rs` | After a clip triggers, push an updated icon (`ClipStatus::Playing`) for the newly-active key and `ClipStatus::Idle` for whichever key was previously active. The plumbing (`current_status` HashMap) is already in `main.rs`; the actual `deck.set_key_image()` calls need to be added. |

### 2.3 `vj-audio` — PipeWire capture + FFT features

**Files:** `lib.rs`, `capture.rs`, `features.rs`

| Piece | Status | Next step |
|---|---|---|
| PipeWire stream setup (`capture.rs`) | Compiles clean against real `pipewire` crate 0.8 API (this took several real API-mismatch fixes during development — see STATUS.md) | **Has never run against a live PipeWire graph.** First real-hardware task: confirm the stream actually connects to the USB audio node and buffers arrive in the `process` callback at all. |
| Buffer format handling (`bytemuck_cast_slice`) | Implemented but flagged in its own doc comment as an alignment assumption | Validate the negotiated buffer format matches F32LE as configured before trusting this cast; add a runtime check/fallback copy path if misaligned buffers show up in practice |
| Quantum/rate system config (`systemd/pipewire/*.conf`) | Written per documented PipeWire tuning guidance | Needs on-device validation with `pw-top`/`pw-dump` that the graph actually locks to quantum=256 and doesn't silently renegotiate under load |
| FFT feature extraction (`features.rs`) | Implemented (Hann-windowed FFT via `rustfft`, band-averaged into bass/mid/high) | Logic is self-contained and doesn't depend on PipeWire specifics, so this can be unit-tested with synthetic sine-wave input independent of real hardware — **do this first**, before debugging it live against real audio |
| Shader uniform bus | **Not implemented at all** — flagged with a `TODO` in `vj-core/src/main.rs` | This is a genuine open design question, not just missing code: how do audio features actually reach whatever renders the shaders? Options: shared-memory ring buffer, a second mpv `--script-opts` channel, an OSC socket to an external shader host, or a Rust-native shader host built into `vj-core` itself. **This needs a decision from whoever owns the shader/visual side of the project** — it wasn't specified in the original requirements beyond "leave GPU compute for shaders," so no shader host or rendering technology was chosen or built. |

### 2.4 `vj-core` — orchestration daemon

**Files:** `main.rs`, `config.rs`

| Piece | Status | Next step |
|---|---|---|
| Config loading (TOML) | Implemented, simple | Fine as-is |
| Startup sequence (clip cache scan/warm, mpv spawn, Stream Deck connect, audio start) | Implemented | Should work; watch startup ordering on real hardware — if USB enumeration is slow, `connect_with_retry`'s backoff should handle it, but hasn't been timed against real boot behavior |
| Main event loop (button press → clip trigger) | Implemented, includes mpv-died respawn handling | Depends on the button-diffing fix in §2.2 to not misbehave |
| Registered-song / play-state / language remote commands | **Not in scope of this scaffold at all** | Nothing here — this project scaffold covers *local* Stream Deck triggering only. If the SmartKaraokePlayer-style remote-control protocol work from earlier in this session is meant to connect to this project, that integration hasn't been designed and is a substantial separate piece of work. (Flagging this explicitly because those two pieces of context ended up in the same conversation by accident — see the "wrong project" correction earlier in this thread. Do not assume any connection between them unless the user says so.) |

---

## 3. Suggested implementation order

This is a recommendation, not a hard requirement — reorder based on what
hardware arrives first.

1. **Get *something* on screen.** Manually run mpv on the target Pi 5 with
   the exact flag set in `player.rs` and a test HEVC clip, outside of
   `vj-core` entirely. Confirm DRM/KMS output works at all before writing
   more Rust. This validates or invalidates the single riskiest assumption
   in the whole project.
2. **Fix the button-press diffing bug** in `vj-streamdeck/src/device.rs`
   (§2.2) — small, mechanical, and blocks anything downstream from being
   usable.
3. **Wire `vj-core` end-to-end on real hardware**: Stream Deck press →
   `play_now()` → visible clip change, with real trigger-latency
   measurement against the <50ms target. This is the core value
   proposition of the whole system; get it working before polishing
   anything else.
4. **Validate PipeWire capture** in isolation (`pw-top` showing the stream,
   `features.rs` unit-tested against synthetic input, then live against
   real line-in audio).
5. **Decide and build the shader uniform bus** (§2.3) — this requires a
   decision on shader/rendering technology that wasn't part of the original
   scope handed to this scaffold.
6. **Wire icon updates** into the main loop (§2.2) — cosmetic/UX polish,
   correctly ordered last since it depends on nothing being broken
   upstream.
7. Only after all of the above: revisit whether crossfades
   (`raw_drm.rs`/two-plane compositing) are actually needed, given real
   usage.

---

## 4. Things a fresh implementer should NOT do

- **Don't assume H.264 hardware decode is available or "probably fine to
  add later."** It isn't there on this SoC. Re-read RESEARCH.md §1.1 if
  tempted.
- **Don't switch to libmpv in-process "for simplicity."** The subprocess
  boundary is a deliberate crash-isolation decision, not scaffolding
  laziness — see §1.2.
- **Don't trust any of this crate's cargo-check-clean status as proof it
  works.** It proves the Rust compiles against the real crate APIs (which
  is non-trivial and did catch real bugs during development — see
  STATUS.md) — it proves nothing about runtime behavior on hardware this
  environment didn't have.
- **Don't build the raw ffmpeg/DRM path speculatively.** It's a multi-week
  investment; only justified by a measured need the mpv-subprocess path
  can't meet.
- **Don't conflate this project with the SmartKaraokePlayer protocol-probe
  work from elsewhere in this session's history.** They are unrelated
  projects that ended up in the same chat by accident (see the
  conversation's own correction on this). If a future requirement
  genuinely does want the RPi-VJ box to speak that remote-control
  protocol, that's new scope requiring its own design pass, not something
  implied by anything in this codebase today.
