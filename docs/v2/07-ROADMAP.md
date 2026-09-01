# Roadmap — Two Parallel Tracks

Two tracks run at once over a shared middle. **Track A** keeps your gigs supplied from the existing repository. **Track B** builds the portable engine. Three crates are written once and consumed by both, which is what stops the parallelism from being duplicated work.

```
        ┌──────────────────────────── Track A · Pi appliance ───────────────────────────┐
        A0 bring-up ── A1 typed core ── A2 surfaces ── A3 two decks ── A4 freeze
                                          │
                            shared: vj-types · vj-surface · vj-devices
                                          │
        B0 workspace ── B1 GPU spike ── B2 ISF ── B3 decode ── B4 audio+tempo ──┘
                                                                       └── B5 surfaces
                                                                            └── B6 desktop app
                                                                                 └── B7 Pi port
                                                                                      └── B8 macOS
        └──────────────────────────── Track B · portable core ─────────────────────────┘
```

Effort is given in solo-developer working days, as ranges. They are estimates, not commitments.

---

## Track A — Pi appliance

Goal: a Pi that boots into a show and launches clips from all six controllers, with no shaders. It uses mpv and it is deliberately a dead end — its job is to be reliable at a gig while Track B is being built.

### A0 · Hardware bring-up — 3–5 days

The earlier research produced a design and never touched hardware. This phase closes that gap and nothing else.

- Build and run the current binary on the exact Pi 5 Bookworm Lite image.
- Record kernel, Mesa, mpv, PipeWire, GStreamer, and firmware versions into `docs/target-baseline.md` with raw command output.
- Confirm the Stream Deck XL product IDs against your actual units.
- Resolve the HDMI-owning DRM card and connector at runtime; stop hardcoding it in the systemd unit.
- Test the supplied systemd, udev, and PipeWire files end to end.
- Verify an HEVC clip actually decodes on the hardware path and reaches the screen: `ffmpeg -hwaccel drm -c:v hevc -i clip.mp4 -f null -` at many multiples of realtime with near-zero CPU.

**Exit:** clip switching and Stream Deck reconnect survive a reboot with no desktop session, and a 30-minute soak shows no child exit or memory growth.

### A1 · Typed control core — 4–6 days

- Adopt `vj-types` and `vj-core` from Track B's B0. This is the first shared handoff.
- **Fix the press-edge bug.** `src/streamdeck.rs` re-fires `Pressed` on every poll tick a key reads as held; `vj-surface-hid::diff_buttons` is the tested replacement.
- Replace unbounded crossbeam channels with the bounded coalescing queue.
- Wrap the mpv launcher behind `VideoDeck`.
- Add configuration loading, graceful shutdown, and structured worker-health events.

**Exit:** existing behaviour intact, no adapter mutates playback state directly, queue saturation is deterministic and counted.

### A2 · Control surfaces on the Pi — 8–12 days · **shared with B5**

The phase that delivers what you asked for. Everything here is written in `vj-surface`, `vj-surface-midi`, and `vj-devices`, so Track B inherits it complete.

- MIDI transport over ALSA sequencer with enumeration, hot-plug, and stable identity.
- Device profiles for all six controllers, plus SysEx drivers for Launchpad, Fire, and APC mk2.
- Virtual surface: regions, bindings, banking, soft takeover, feedback worker with diffing and priorities.
- MIDI learn with persistence — the mechanism by which the three undocumented devices become correct.
- Configure the X-Touch Mini in X-Touch Editor on the PC **before** it goes near the Pi, and commit the resulting configuration.

**Exit:** Gate H in [`08-VALIDATION.md`](08-VALIDATION.md).

### A3 · Two decks and prepared switching — 5–8 days

- Media index with stable clip IDs, codec/GOP/colour probing, and cached device-ready thumbnails.
- Hot decoder slots — two prerolled mpv contexts, or a move to GStreamer if mpv cannot hold independent ready slots.
- First-frame acknowledgement, so `ClipActivated` reports a real presentation timestamp.
- Crossfade between two live sources.
- Quantized launch from `vj-tempo` (audio tempo only at this stage).

**Exit:** Gate C — prepared trigger to visible activation under 50 ms at p95, and a late preparation never blanks the output.

### A4 · Freeze and harden — 4–6 days

- Metrics, watchdog, atomic show-state writes, safe fallback scene.
- Fault injection: corrupt media, device unplug, command flood, decoder stall.
- Pin the OS and package baseline. Produce a rollbackable release artifact and a clean-image install procedure.

**Exit:** Gate G — eight-hour soak with fault injection, recovery observable without an attached keyboard.

**Track A total: roughly 24–37 days.** After A4, Track A receives no new features. It runs your gigs until B7 lands.

---

## Track B — Portable core

### B0 · Workspace and vocabulary — 2–3 days

Already scaffolded: the `vj/` workspace with `vj-types`, `vj-core`, `vj-surface`, `vj-tempo`, and the crate seams, compiling with 62 passing tests and no warnings. Remaining work is CI for all three targets and the platform-tier detection that currently guesses from `cfg!`.

**Exit:** all three targets build in CI on every change to a shared crate.

### B1 · GPU backend spike — 6–10 days · **highest risk in the program**

The smallest program that decodes one target-profile clip, imports it to the GPU, renders one moderate audio-reactive fragment shader, composites both at 1080p, presents at 60 fps, and records CPU, GPU frame time, copies per frame, dropped frames, memory, and temperature.

Run it on the 3090 first (fast iteration), then on the Pi. Compare wgpu/Vulkan against GLES/EGL on the Pi if wgpu's video import is not clearly zero-copy.

**Exit:** Gate B. Backend chosen from measurements and recorded in ADR-100. If GLES wins on the Pi, that is recorded honestly rather than hidden.

### B2 · ISF host — 8–12 days

- ISF 2 parsing, embedded-profile validation, built-in and audio-binding injection.
- shaderc → SPIR-V → naga → backend, with the on-disk pipeline cache.
- Scalar, image, and `audioFFT` inputs; bounded multipass; persistent targets from a pooled allocator.
- Conformance fixtures including a deliberate compile failure and a deliberately over-budget request.

**Exit:** Gate E — valid fixtures render at their declared scale, invalid assets fail before activation, the live shader survives a bad compile, no compilation on the render thread.

### B3 · Video decode backends — 10–15 days

`VideoDeck` implementations behind features: NVDEC on Windows first (fastest to validate), VideoToolbox on macOS, GStreamer V4L2 on the Pi. Plus the clip index, hot slot pool with eviction, and `vj-ingest` producing per-tier renditions from one master.

**Exit:** Gate A on each platform — no sustained dropped frames on the delivery profile, documented import/copy behaviour, loop-boundary strategy chosen from observed decoder behaviour.

### B4 · Audio and tempo — 6–9 days

- `AudioCapture` implementations: PipeWire, CoreAudio, WASAPI.
- The analyzer: dual-rate EMA, noise-floor normalisation, spectral flux with median+MAD threshold, tempo and phase tracking with confidence.
- Ableton Link integration and the tempo arbiter's real wiring.
- WAV fixture tests so the analyzer is verifiable with no audio device attached.

**Exit:** Gates D and J.

### B5 · Control surfaces — **already done in A2**

Track B consumes `vj-surface`, `vj-surface-midi`, `vj-surface-hid`, and `vj-devices` unchanged. Remaining work is the Windows and macOS transports — CoreMIDI is straightforward; WinMM needs the port-exclusivity decision from [`03-CONTROL-SURFACES.md`](03-CONTROL-SURFACES.md) §3. Budget 3–5 days.

### B6 · Desktop application — 10–14 days

- Full compositor: explicit YUV/range/matrix/transfer conversion, all six blend and key modes, layer transforms, opacity ramps.
- Deck pool integrated with transitions.
- Window management, output selection, fullscreen on a second display.
- The admin API and a minimal local UI for show authoring and MIDI learn.

**Exit:** Gate B render targets met at the Workstation tier, colour test vectors within documented tolerance, no steady-state heap allocation on the render thread.

### B7 · Pi port — 5–8 days

The moment the portable engine replaces Track A. Most of the work is already done; this phase is the appliance packaging — DRM/KMS presentation, boot-to-show, tier degradation, and re-running every Track A gate against the new binary.

**Exit:** every Track A gate passes on the new engine. Track A is retired.

### B8 · macOS — 4–6 days

VideoToolbox backend, CoreMIDI transport, `CAMetalLayer` presentation, code signing. Syphon output is explicitly out of scope for the first release.

**Track B total: roughly 54–82 days**, of which A2's 8–12 are already counted in Track A.

---

## Sequencing

The dependency that matters: **A1 needs B0.** Do B0 first — it is 2–3 days and it is already largely written. Then A0 and B1 can run in parallel, since A0 is Pi-side operational work and B1 is desktop GPU work.

A reasonable order for one person:

1. **B0** — the workspace exists (mostly done).
2. **A0** — find out what is actually true about your Pi. Do not skip this; every latency and decode claim in the corpus is currently unmeasured.
3. **A1 + A2** — this is the block that makes your rig usable with all six controllers. It is also the single largest shared investment.
4. **B1** — the GPU spike, on the 3090. This is where the program's biggest unknown gets resolved, and it can be interleaved with A3 because it is a separate machine and a separate head-space.
5. **A3 + A4** — freeze the appliance. Gig with it.
6. **B2 → B8** — build the real engine without gig pressure.

## Explicit non-goals for the first production release

NDI, SRT, RTMP, Spout/Syphon, recording, projection mapping, multiple output heads, arbitrary desktop GLSL compatibility, loading unvalidated shaders during a performance, network-distributed rendering, decoding the whole library continuously, and Elgato desktop-plugin ecosystem compatibility.
