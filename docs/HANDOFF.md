# Claude Implementation Handoff

## Start here

This repository is a researched prototype, not a hardware-validated production system. The next implementer should treat this document and the files under `docs/` as authoritative. `RESEARCH.md` and `rpi-vj-architecture.md` contain useful earlier exploration, but they describe components that do not all exist in the current repository.

The target is a Raspberry Pi 5 running Raspberry Pi OS 64-bit Lite Bookworm, connected to an HDMI display, a Stream Deck XL, and a USB sound card line input. The system must boot without a desktop compositor, launch clips from physical keys with minimum jitter, capture audio through PipeWire, and preserve GPU capacity for later audio-reactive shaders.

## Read in this order

1. [`docs/RESEARCH_FINDINGS.md`](RESEARCH_FINDINGS.md) — verified hardware/software conclusions and rejected assumptions.
2. [`docs/ARCHITECTURE.md`](ARCHITECTURE.md) — target component boundaries, data flow, thread model, and latency budgets.
3. [`docs/IMPLEMENTATION_PLAN.md`](IMPLEMENTATION_PLAN.md) — staged work packages and acceptance criteria.
4. [`README.md`](../README.md) — current build/deployment notes.
5. Current implementation: `src/`, `native/`, `config/`, and `scripts/`.

## What currently exists

| Area | Current state | Important limitation |
|---|---|---|
| Video | `src/player.rs` starts mpv in DRM mode and sends `loadfile` commands over inherited JSON IPC | No decoded-frame pool, no shader compositor, no measured <50 ms guarantee, no automatic mpv restart |
| Stream Deck | `src/streamdeck.rs` uses hidapi, polls 32 keys, reconnects, uploads JPEG icons, and restores cached state | Protocol/product IDs require real XL/XL Mk2 testing; icons are generated motifs rather than clip thumbnails |
| Audio | `native/pw_capture.c` negotiates F32 stereo at 48 kHz and requests quantum 256 | It only computes peak and is not connected to the Rust process or an FFT pipeline |
| Orchestration | `src/main.rs` maps command-line clips to keys | Clip paths and settings are hard-coded at launch; no config schema, lifecycle state machine, health endpoint, or signal handling |
| Deployment | PipeWire, udev, systemd, and install files exist under `config/` and `scripts/` | system-vs-user PipeWire deployment must be selected and tested; DRM node is hard-coded in the unit |
| Tests | One JPEG signature unit test exists | Cargo could not be run in the original Windows environment; no Pi/hardware tests exist |

## Non-negotiable facts

- Pi 5 hardware-decodes HEVC. H.264 runs in software. Do not design or document an H.264 zero-copy hardware path.
- A 256-frame quantum at 48 kHz is 5.33 ms per graph period. “Under 10 ms” is plausible for capture-to-analysis, but must be measured with the actual USB interface.
- Ordinary file/page caching is not the same as predecoded first-frame caching. A hard <50 ms HID-to-photon target requires hot decoder slots or held decoded DMABUF/EGL images.
- Zero-copy must be proven from negotiated buffer types and DRM state. An option named `hwdec` does not prove that no CPU copy occurred.
- DRM/KMS ownership excludes a desktop compositor. Avoid X11/Wayland in the production image.
- Never perform allocation, logging, image encoding, file I/O, or blocking synchronization in the PipeWire real-time callback.

## Immediate next task

Perform Phase 0 in `IMPLEMENTATION_PLAN.md` on a Pi 5 before expanding features. The existing Rust and C sources must compile against the exact Bookworm package versions, and the mpv, HID report, PipeWire format, and DRM-device assumptions must be observed on real hardware. Fix evidence-backed incompatibilities before implementing shaders or a larger preload system.

## Definition of done

The system is ready for a live performance only when all of the following are recorded on the target hardware:

- 60-minute randomized clip-trigger soak with no crash, hang, USB reconnect failure, audio xrun, thermal throttle, or sustained frame drop.
- HID-report-to-photon latency distribution with p50, p95, and p99 values; the agreed threshold applies to p99, not one best-case measurement.
- `pw-top` showing 48,000 Hz, quantum 256, and zero errors with the chosen USB interface.
- mpv/GStreamer debug output and DRM state showing the expected HEVC hardware and DMABUF path.
- Service recovery after mpv crash, Stream Deck unplug/replug, audio interface unplug/replug, HDMI disconnect/reconnect, and reboot.
- Reproducible build with a committed `Cargo.lock`, recorded OS/kernel/Mesa/mpv versions, and a rollbackable release artifact.

