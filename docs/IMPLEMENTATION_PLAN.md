# Implementation Plan

## Working rules

- Implement and validate one phase at a time on the physical Pi 5.
- Do not add the shader compositor until the baseline video, HID, and audio paths are individually measured.
- Every performance claim needs a timestamped measurement and recorded OS/kernel/package versions.
- Preserve bounded memory and queues. Prefer dropping obsolete UI/audio-analysis updates over blocking a real-time path.

## Phase 0 — Make the current scaffold truthful and reproducible

### Tasks

- Install current Raspberry Pi OS Lite Bookworm and record image, kernel, firmware, Mesa, mpv, PipeWire, WirePlumber, GStreamer, and Rust versions.
- Run `cargo generate-lockfile`, commit `Cargo.lock`, then run formatting, Clippy with warnings denied, tests, and release build.
- Compile `native/pw_capture.c` with `-Wall -Wextra -Werror`.
- Confirm which `/dev/dri/card*` owns the connected HDMI connector and which render node is paired with V3D.
- Confirm Stream Deck USB product ID, hidraw node, report descriptor, input offsets, JPEG orientation, and key numbering.
- Confirm the USB sound card's supported rates/formats and PipeWire node identity.
- Resolve system PipeWire versus user PipeWire. Update the service unit to match exactly one model.
- Add `rust-toolchain.toml` or document the minimum Rust version actually used.

### Required fixes likely to emerge

- Replace hard-coded DRM paths with configuration/discovery.
- Correct mpv flags for the installed mpv version and verify inherited `input-ipc-client=fd://0` behaves as intended.
- Correct Stream Deck protocol constants per the exact hardware revision.
- Make the installer idempotent and fail with actionable diagnostics.

### Acceptance

- Clean build from a fresh image using documented commands.
- mpv displays one HEVC test clip directly on HDMI without a desktop.
- Every Stream Deck key produces exactly one press edge and icon updates survive reconnect.
- PipeWire client receives stable F32 stereo blocks at 48 kHz/256 with zero xruns for 30 minutes.

## Phase 1 — Productionize orchestration

### Tasks

- Add a versioned TOML configuration schema for DRM connector/device, audio target, clip bank, key mapping, cache budgets, logging, and latency thresholds.
- Validate all configuration at startup and provide `--check-config` and `--diagnose` commands.
- Introduce `Player`, `Deck`, and `AudioSource` traits so hardware-independent state logic is testable.
- Replace unbounded crossbeam channels with bounded queues and documented overflow behavior.
- Add Ctrl-C/SIGTERM handling; shut down child mpv and release DRM cleanly.
- Detect mpv death, restart it with exponential backoff, and restore the active clip/state.
- Add button release events and debounce only if measured reports require it.
- Generate thumbnails offline at clip ingest; cache normal, active, missing, and error icons.
- Add structured event/timing logs.

### Tests

- Unit-test clip/key configuration, state transitions, queue overflow, retry backoff, and mpv JSON serialization.
- Add fake-player and fake-deck integration tests for trigger ordering and reconnect restoration.
- Property-test key bounds and malformed configuration.
- Test paths containing spaces and non-ASCII characters.

### Acceptance

- Service survives 100 forced mpv crashes and resumes control.
- Deck can be unplugged for five minutes and reconnect without restarting the service.
- Invalid/missing clip produces an error icon and leaves the prior video running.

## Phase 2 — Measure and implement fast clip readiness

### Tasks

- Build the latency instrumentation first: HID receive, coordinator receive, player command, decoder/first-frame readiness, and DRM page flip.
- Add an ingest tool using `ffprobe` JSON to validate/normalize clip properties and generate device-ready JPEG thumbnails.
- Implement bounded file/GOP warming using `posix_fadvise` plus sequential reads without monopolizing memory.
- Benchmark single-mpv `loadfile` latency across the complete clip bank.
- Prototype hot decoder slots with GStreamer and libmpv. Keep 4–8 clips decoded to a ready first frame.
- Record memory per slot, decoder setup time, copy count, CPU, dropped frames, and recovery behavior.
- Select the backend in an ADR based on measurements.

### Acceptance

- A repeatable test harness reports p50/p95/p99 HID-to-photon latency.
- Hot-set p99 meets the agreed target, ideally <50 ms.
- Cold-path latency is reported separately and never mislabeled as hot-path performance.
- Cache memory remains within a configured hard limit.

## Phase 3 — Integrate PipeWire audio and DSP

### Tasks

- Replace or wrap the standalone C utility with a Rust-owned `AudioSource` implementation. If using FFI, document ownership and every unsafe invariant.
- Add a preallocated SPSC block ring between the PipeWire callback and DSP worker.
- Negotiate device format explicitly and handle reconnect/renegotiation.
- Implement RMS, peak, FFT bands, spectral flux, onset/beat, and attack/release smoothing.
- Publish an immutable latest-feature snapshot with capture timestamps and sequence numbers.
- Add audio calibration and gain controls outside the RT callback.

### Tests

- Deterministic DSP tests with sine waves, impulses, silence, clipping, NaN/Inf input, and known beat patterns.
- Callback-duration histogram and forced DSP slowdown test proving capture never blocks.
- USB unplug/replug and default-node-change tests.

### Acceptance

- `pw-top` reports zero xruns for a one-hour run.
- Capture-to-feature p99 remains below the agreed limit.
- DSP overload drops analysis blocks predictably without disrupting capture or video.

## Phase 4 — EGL shader compositor

### Tasks

- Create a GBM/EGL GLES 3.1 renderer owning the DRM output.
- Import decoder DMABUFs with modifier-aware EGL image import.
- Implement shader compilation, uniform validation, fallback shader, and atomic shader swaps.
- Upload the latest audio feature snapshot once per render frame.
- Add framebuffer/texture pools and explicit ownership/lifetime rules.
- Integrate DRM atomic presentation and page-flip timestamps.
- Decide whether ISF compatibility is required; if so, define the supported subset before writing a loader.

### Acceptance

- Sustained 1080p60 with the representative worst-case shader and HEVC clip.
- No per-frame heap allocations after warm-up.
- No CPU frame conversion/copy in profiles.
- Shader compilation failure falls back without interrupting playback.

## Phase 5 — Deployment hardening

### Tasks

- Make service paths, user/session model, DRM device rules, and PipeWire units image-specific and reproducible.
- Add release packaging, checksums, configuration migration, and rollback.
- Add thermal/throttle monitoring and startup checks for PSU, display mode, storage, devices, and permissions.
- Add watchdog integration, log rotation, and a diagnostic bundle command.
- Run fault injection: player kill, USB loss, HDMI hotplug, disk read failure, corrupt clip, audio format change, thermal throttling.
- Document operator startup, shutdown, clip ingest, troubleshooting, and rollback procedures.

### Acceptance

- One-hour randomized soak passes with no missed acceptance thresholds.
- Cold boot reaches Ready deterministically.
- A non-developer can deploy, diagnose, and roll back using the runbook.

## Deferred work

- Crossfades and multi-layer composition.
- Network control, OSC/MIDI, web UI, and remote monitoring.
- Raw FFmpeg/V4L2 Request/DRM backend.
- HDR/10-bit production pipeline.
- Vulkan backend.

These are intentionally deferred until the single-layer, 8-bit HEVC, GLES 3.1 path is measured and stable.

