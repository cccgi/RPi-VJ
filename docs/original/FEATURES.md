# Original implementation: features and status

This document describes what the `original` branch contains as of 31 August 2026. The complete show application is [`gig/`](../../gig); the other code paths are research or partial implementations.

## Show engine

The gig engine is a single-page JavaScript application rendered by Chromium. WebGL2 performs composition and shader rendering; two HTML video elements provide alternating clip decks; Web Audio extracts live audio features; WebHID controls the Stream Deck XL.

### Video playback

- Two-deck playback permits a new clip to load before replacing the old one.
- Clips loop, play muted, and can be cleared independently of shader effects.
- Playback speed can be trimmed from 0.25× to 4× in multiplicative 1% steps.
- Layout modes include normal, horizontal mirror, vertical mirror, and quad mirror.
- A resolution governor lowers expensive shaders to 720p or 540p and can dynamically reduce render scale when frame time is too high.
- The shipped `show.json` records historical 1080p, 720p, quad-mirror, neon, and 50th-birthday clip pages. Media itself is excluded.

### Visual generators

The repository ships audio-reactive fragment shaders including plasma, tunnel, kaleidoscope, particles, oscilloscope, multiband rings, spectrum, cyber wormhole, wave rings, liquid metal, strobe geometry, spectrum bars, cyber grid, strata drift, sacred polyhedron, shattered glitch, and deep drift. Retired experiments remain under `gig/shaders/_retired` for reference.

### Mixing and effects

- Blend modes: normal, add, screen, multiply, overlay, soft light, luma key, and difference.
- Independent video-layer and shader-layer effects.
- Effects: invert, hue, contrast, black-and-white, posterize, threshold, warm, cool, mirror H/V, quad, kaleidoscope, zoom in/out, wave, twirl, pixelate, scanline, RGB shift, edge, vignette, grain, halftone, and strobe.
- Four fixed opacity/intensity presets make level changes predictable from the Stream Deck.
- Optional post-processing code supports trails, bloom, chromatic aberration, and grain, but the historical runbook marks this chain unqualified for the show.

### Audio and tempo

- Live microphone/line-input capture through Web Audio.
- Volume, bass, mid, treble, spectral and onset-derived values drive shaders and choreography.
- Tap tempo and BPM nudging are exposed on the controller.
- When live audio fails, the visual clock continues at a fallback tempo.
- The historical hardware route used a Pioneer DDJ-FLX10 line source through a Creative Sound Blaster Play! 3 USB adapter.

### Control and safety

- Stream Deck XL has nine configured pages and uploads generated labels/thumbnails to all 32 LCD keys.
- Page navigation, reset, and blackout occupy stable locations on every page.
- Reset stops and blanks both video decks, clears the shader, restores opacity/layout/speed, and removes blackout.
- Blackout toggles the HDMI composition black without shutting down the process.
- Deliberate shutdown gesture: hold RESET for three seconds, then press BLACK within five seconds.
- WebHID reconnect handling and complete key-state republishing are implemented, although Chromium permission/profile behavior remains a reliability dependency.
- Optional Behringer X-Touch Mini WebMIDI control and five shader preset slots exist in the application.

### Operations

- Python HTTP/control server with health, diagnostics, shutdown, restart, and HDMI-mode endpoints.
- User systemd units for the server and kiosk, both configured to restart.
- Udev rules for Stream Deck HID access.
- Deployment, profile repair, 1080p-lock, audio selection, Samba, SSH alias, sync, diagnostics, soak reporting, and probe scripts.
- Browser/device/frame heartbeat logs intended to survive enough failure modes for diagnosis.
- Capability pages test WebGL, codecs, video decks, audio, ISF shaders, and performance.

## What is not guaranteed

The original stack is not a sealed headless appliance. It depends on a desktop/session/display stack, Chromium startup, a persistent browser profile, WebHID permission grants, and Web Audio permission/device selection. The 29 August field incident showed repeated boots without a usable kiosk and a later boot whose Stream Deck keys could not launch clips. See [`gig/APPLIANCE_RELIABILITY.md`](../../gig/APPLIANCE_RELIABILITY.md) and the replacement plan in [`tasks/APPLIANCE-V3/`](../../tasks/APPLIANCE-V3/README.md).

## Other code paths

### Root Rust/mpv prototype

`src/` is a small native experiment using mpv JSON IPC, hidapi, generated key images, and a native PipeWire capture sample in `native/pw_capture.c`. It explores DRM/KMS and low-latency design choices, but is not wired to the gig engine's complete feature set.

### Native Rust workspace

`vj/` is a more ambitious modular design. It contains tested data models, command queues, tempo arbitration, controller abstractions, codec policy, shader metadata parsing, device profiles, an ingest CLI, and platform seams. Its GPU, decode, capture, and hardware transports remain incomplete; `vj-app` is a scaffold rather than a replacement show binary. See [`vj/README.md`](../../vj/README.md).

### Appliance V3

`tasks/APPLIANCE-V3/` is the implementation plan for a desktop-free Pi OS Lite appliance using KMS/DRM, a native Stream Deck daemon, deterministic local media, persistent observability, USB ingestion, and a local admin portal. On `original`, these are specifications only.
