# Implementation Plan

## Delivery strategy

Keep a runnable vertical slice throughout. Do not create every target crate before behavior exists. Extract modules when the phase needs their boundary.

## Phase 0 — Baseline and reproducibility

Status: current repository is approximately here, but validation evidence is incomplete.

Tasks:

- Build and run the existing launcher on the exact Pi 5 Bookworm Lite image.
- Record Mesa, kernel, mpv, PipeWire, GStreamer, and firmware versions.
- Confirm the two configured Stream Deck XL USB product IDs.
- Confirm 1080p60 HDMI mode and DRM device selection.
- Test the supplied systemd, udev, PipeWire, and installer files.
- Add a `docs/target-baseline.md` containing command output and observed behavior.

Exit criteria:

- Existing clip switching and reconnecting Stream Deck work after boot without a desktop session.
- A 30-minute playback soak has no child exit or memory growth.

## Phase 1 — Typed control core

Tasks:

- Add `vj-core` or an equivalent internal module containing IDs, commands, events, state, and reducer.
- Introduce bounded queues. Reserve/coalesce parameter updates instead of enqueueing every change.
- Wrap the existing mpv implementation behind `VideoDeck`/`PlaybackBackend`.
- Translate Stream Deck HID events into `EngineCommand`s.
- Add state revisions and feedback snapshots.
- Add graceful shutdown and explicit worker health events.

Tests:

- reducer command/state table tests;
- queue saturation/coalescing tests;
- request lifecycle tests;
- Stream Deck reconnect snapshot replay tests.

Exit criteria:

- Existing user behavior remains intact.
- No device adapter directly mutates playback state.
- Queue saturation has deterministic behavior.

## Phase 2 — Media index and better switching

Tasks:

- Add show/media configuration with stable clip IDs.
- Probe codec, dimensions, frame rate, duration, GOP/keyframes, and color metadata.
- Generate and cache 96x96 key thumbnails off the HID thread.
- Add deck A/B abstractions.
- If staying temporarily on mpv, run two paused/prerolled instances or two controllable playback contexts and acknowledge first-frame readiness.
- Add loop and transition state.

Exit criteria:

- Prepared clip trigger to visible activation is under 50 ms at p95.
- A late preparation never causes black output.
- HEVC and H.264 clips are clearly identified in diagnostics.

## Phase 3 — Audio capture and DSP

Tasks:

- Implement PipeWire capture into a fixed SPSC ring.
- Implement 2048-point Hann-windowed FFT using `rustfft` or equivalent.
- Publish 128 log/mel bins initially, retaining capacity for 256.
- Implement RMS, bass/mid/treble, noise-floor and upper-envelope normalization.
- Implement time-based attack/release filters.
- Implement spectral-flux onset detection and beat phase/confidence.
- Add offline WAV fixtures and deterministic analyzer tests.

Exit criteria:

- Callback contains no allocations or locks under instrumentation.
- Analyzer survives ring overrun by dropping old data and reporting a counter.
- Onset-to-snapshot latency meets the validation target.
- Disconnect fades features and capture reconnects without restarting video.

## Phase 4 — GPU compositor spike and backend decision

This phase is a decision experiment, not a wholesale rewrite.

Implement the smallest program that:

- decodes one target-profile MP4;
- imports or uploads frames to the GPU;
- renders one moderate audio-reactive fragment shader;
- composites both at 1920x1080;
- presents via DRM/KMS at 60 fps;
- records CPU, GPU/frame time, copies, dropped frames, memory, and temperature.

Compare wgpu/Vulkan with GLES/EGL if wgpu video import is not clearly zero/low-copy. Record the decision in ADR-002.

Exit criteria:

- backend selected from measurements;
- 30-minute run satisfies frame and thermal gates;
- selected path has a documented decoded-buffer lifetime and synchronization model.

## Phase 5 — ISF host

Tasks:

- Parse ISF 2 metadata and validate the embedded profile.
- Generate built-ins/macros and audio bindings.
- Compile and cache shaders outside the render thread.
- Implement scalar/image/audioFFT inputs.
- Implement bounded multipass and persistent targets from a pooled allocator.
- Surface compiler diagnostics without replacing the live shader.
- Add conformance fixtures: generator, image filter, two-pass, persistent feedback, FFT input, and intentional failure.

Exit criteria:

- All conformance fixtures run at 1080p60 or their declared internal scale.
- Bad shaders cannot stall or blank the live output.
- No compilation occurs on live selection.

## Phase 6 — Production compositor and two decks

Tasks:

- Integrate selected render backend into the main engine.
- Implement explicit YUV/range/matrix/transfer conversion.
- Implement alpha, additive, multiply, screen, luma key, and chroma key.
- Implement layer transforms and opacity ramps.
- Integrate two-deck preroll and transitions.
- Add texture/render-target pooling and GPU timing.
- Add adaptive internal shader scale behind a configuration flag.

Exit criteria:

- Two prepared sources plus one moderate shader transition without deadline misses.
- Color test vectors match expected values within documented tolerances.
- Render thread has no steady-state heap allocations.

## Phase 7 — Stream Deck instrument surface

Tasks:

- Add profiles/pages and semantic bindings.
- Add press/release, hold, and optional repeat behavior.
- Add clip/shader thumbnails and parameter value keys.
- Hash/coalesce image updates and enforce priorities.
- Cap decorative updates around 10–15 fps.
- Add blackout/error behavior that remains responsive during render load.

Exit criteria:

- Key input remains responsive during full GPU workload.
- Reconnect repaints the current page and active state exactly.
- Thumbnail work never runs on the HID thread.

## Phase 8 — Appliance hardening

Tasks:

- Add structured metrics: frame percentiles, missed vblanks, queue drops, audio overruns, decode stalls, device status, temperature.
- Add watchdog heartbeat and systemd restart policy.
- Make mutable writes atomic and minimize them.
- Add startup asset validation and a safe fallback scene.
- Perform power-loss, device-unplug, corrupt-media, invalid-shader, thermal, and 8-hour soak tests.
- Pin and document the OS/software baseline.

Exit criteria:

- All validation gates pass on target hardware.
- Installation from a clean Bookworm Lite image is documented and repeatable.
- Recovery behavior is observable and does not require an attached keyboard.

## Explicit non-goals for the first production release

- NDI, SRT, RTMP, recording, projection mapping, or multiple HDMI outputs.
- Arbitrary desktop GLSL compatibility.
- Loading unvalidated shaders during a performance.
- Network-distributed render services.
- Decoding every clip continuously for instant access.
- Full compatibility with Elgato's desktop plugin ecosystem.

