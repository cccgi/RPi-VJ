# Validation Plan

All performance claims must be measured on the final Raspberry Pi 5 model, cooling solution, USB audio interface, Stream Deck XL revision, storage device, HDMI display mode, and pinned Bookworm/Mesa image.

## Instrumentation

Capture at minimum:

- CPU utilization by thread/process;
- resident memory and allocation counts;
- render CPU time and GPU timestamp duration;
- presentation interval and missed vblanks;
- decoder queue depth, late frames, and copies/uploads per frame;
- audio ring overruns and analyzer duration;
- command queue drops/coalesces;
- Stream Deck HID writes and input latency;
- SoC temperature, clock, and throttling flags.

Write a timestamped JSON or CSV artifact for each test run and record exact build/git revision and system package versions.

## Gate A — Codec and decode path

Test 1080p60 constant-frame-rate clips:

- MP4/HEVC Main 8-bit 4:2:0 with GOP 60;
- MP4/HEVC with short GOP 15 or all-intra if supported by the decoder path;
- MP4/H.264 High as compatibility baseline.

Measure one visible decoder and visible+hidden preroll. Determine whether output buffers are DMA-BUF, CPU memory, or copied during import.

Pass conditions:

- no sustained dropped frames for the selected delivery profile;
- sufficient CPU headroom for DSP and control;
- documented import/copy behavior;
- loop boundary strategy selected from observed timestamps and decoder behavior.

## Gate B — Renderer/backend

Workload:

- one decoded 1080p60 source;
- one moderate full-screen shader;
- one blend/color conversion pass;
- DRM/KMS 1920x1080@60 presentation.

Pass conditions:

- mean frame time below 14 ms;
- p99 below 16.0 ms and p99.9 below 16.67 ms during a 30-minute run;
- no unbounded memory growth;
- no thermal throttling with the production cooling solution;
- no per-frame CPU readback.

If both wgpu and GLES pass, prefer wgpu for maintainability. If only GLES passes due to video interop, record GLES as the accepted backend rather than hiding the copy cost.

## Gate C — Trigger latency

Measure from HID press timestamp to first presented pixel of the requested prepared clip. Use a high-speed camera or GPIO/photodiode method for end-to-end verification, plus internal timestamps for diagnosis.

Pass conditions:

- prepared source: p95 below 50 ms;
- unprepared source: no black frame; latency is reported honestly;
- active-source switch acknowledgment matches the actually presented frame.

## Gate D — Audio latency and stability

Inject known clicks, sine bursts, sweeps, and reference music from a second device. Compare source timestamps with published features and rendered response.

Pass conditions:

- onset-to-feature latency below 50 ms target, 70 ms maximum;
- no callback allocations/locks;
- no flicker during steady sine or constant pink noise;
- silence remains visually quiet;
- beat phase does not jump wildly on low-confidence input;
- unplug/replug recovers without disturbing video output.

## Gate E — ISF compatibility and safety

Fixtures:

- single-pass generator;
- input-image filter;
- two-pass blur;
- persistent feedback;
- float/color/point/event inputs;
- audioFFT texture;
- compile failure;
- deliberately excessive pass/resource request.

Pass conditions:

- valid profile fixtures render correctly;
- invalid assets fail before activation with diagnostics;
- previous live shader remains active after failure;
- resource limits are enforced;
- compilation never occurs on the render thread.

## Gate F — Stream Deck

Test rapid multi-key input, page changes, active-state updates, unplug/replug, and feedback during maximum render workload.

Pass conditions:

- no missed rising-edge events in the supported interaction model;
- current page fully restores after reconnect;
- redundant images are not resent;
- decorative feedback cannot starve input or critical status updates.

## Gate G — Soak and fault injection

Run at least eight hours with clip changes, shader changes, audio input, and Stream Deck feedback. During the run inject:

- corrupt or missing media;
- invalid shader;
- audio disconnect;
- Stream Deck disconnect;
- control command flood;
- decoder stall;
- if safely reproducible, GPU/process restart.

Pass conditions:

- output either continues or falls back to a defined safe scene;
- state and reason for degradation are observable;
- memory/queue usage remains bounded;
- systemd recovery returns to a usable state without manual login.

