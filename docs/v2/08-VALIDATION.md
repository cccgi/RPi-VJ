# Validation Gates

Gates A–G are carried forward from the earlier handoff and extended to the new scope. H, I, and J are new.

Every performance claim is measured on the final hardware: the actual Pi 5 with its production cooling, your USB audio interface, your Stream Deck XL revision, your storage device, your HDMI display mode, and a pinned OS image — or the actual 3090 box and the actual M2 Pro. A number measured anywhere else is not evidence.

Nothing in this program may be described as passing a gate until it has been measured. The existing `STATUS.md` sets exactly the right precedent: reasoned budgets and measured results are different things and must be labelled differently.

## Instrumentation

Every run writes a timestamped JSON or CSV artifact recording the git revision, OS/kernel/Mesa/driver versions, and package versions alongside the measurements:

- CPU utilisation per thread and process
- resident memory and allocation counts
- render CPU time and GPU timestamp duration
- presentation interval and missed vblanks
- decoder queue depth, late frames, copies and uploads per frame
- audio ring overruns and analyzer duration
- command queue drops and coalesces
- surface message counts and input latency per device
- SoC/GPU temperature, clock, and throttle flags

Report percentiles, not means. A mean frame time hides exactly the stutter an audience sees.

---

## Gate A — Codec and decode path · per platform

Test 1080p60 constant-frame-rate clips across the tier's formats: HEVC Main 8-bit 4:2:0 GOP 60; HEVC short GOP 15 or all-intra; H.264 High as compatibility baseline; HAP and HAP Q on desktop tiers.

Measure one visible decoder, and visible plus hidden preroll. Determine whether output buffers are DMA-BUF, CPU memory, or copied at import.

**Pass:** no sustained dropped frames on the tier's delivery profile · CPU headroom remains for DSP and control · import and copy behaviour is documented, not assumed · loop-boundary strategy chosen from observed timestamps and decoder behaviour.

## Gate B — Renderer and backend · per platform

Workload: one decoded 1080p60 source (4K60 on Workstation), one moderate full-screen shader, one blend and colour-conversion pass, presented at the platform's native path.

**Pass:** mean frame time below 14 ms · p99 below 16.0 ms and p99.9 below 16.67 ms over 30 minutes · no unbounded memory growth · no thermal throttling with production cooling · **no per-frame CPU readback**.

If both wgpu and GLES pass on the Pi, prefer wgpu for maintainability. If only GLES passes because of video interop, record GLES as the accepted Pi backend in ADR-100 rather than hiding the copy cost.

## Gate C — Trigger latency · per platform

Measure from the input-report timestamp to the first presented pixel of the requested prepared clip. Use a high-speed camera or a photodiode/GPIO method for true end-to-end verification, plus internal timestamps for diagnosis. Internal timestamps alone are not sufficient evidence.

**Pass:** prepared source p95 below 50 ms · unprepared source produces no black frame and its latency is reported honestly as degraded · the activation acknowledgement matches the frame actually presented.

## Gate D — Audio latency and stability · per platform

Inject known clicks, sine bursts, sweeps, and reference music from a second device. Compare source timestamps against published features and the rendered response.

**Pass:** onset-to-feature latency below 50 ms target, 70 ms maximum · no allocations or locks in the capture callback under instrumentation · no flicker under steady sine or constant pink noise · silence stays visually quiet · beat phase does not jump on low-confidence input · unplug and replug recovers without disturbing video output · on the Pi, `pw-top` shows 48 kHz, quantum 256, and zero errors.

## Gate E — ISF compatibility and safety

Fixtures: single-pass generator · input-image filter · two-pass blur · persistent feedback · float/colour/point/event inputs · `audioFFT` texture · deliberate compile failure · deliberately over-budget pass and resource request.

**Pass:** valid profile fixtures render correctly at their declared internal scale · invalid assets fail before activation with diagnostics · the previously live shader remains active after a failure · resource limits are enforced per tier · compilation never occurs on the render thread.

## Gate F — Stream Deck

Rapid multi-key input, page changes, active-state updates, unplug and replug, and feedback under maximum render load.

**Pass:** no missed rising edges · a held key produces exactly one press event · the current page fully restores after reconnect · redundant images are not resent · decorative feedback cannot starve input or critical status updates.

## Gate G — Soak and fault injection

At least eight hours with clip changes, shader changes, live audio, and surface feedback. Inject during the run: corrupt or missing media · invalid shader · audio disconnect · surface disconnect · command flood · decoder stall · and, if safely reproducible, a GPU or process restart.

**Pass:** output continues or falls back to a defined safe scene · the state and reason for degradation are observable · memory and queue usage remain bounded · service recovery returns to a usable state without an attached keyboard.

---

## Gate H — Control surface conformance · new

Run with all six controllers connected simultaneously.

**Pass:**

- Each device drives `clip.grid` from its own independent scroll window, and the windows are visible in engine state so devices can observe each other's page.
- Unplugging any device leaves the show running; replugging restores that device's exact page, **including re-entering Programmer Mode on the Launchpad and any device-mode initialisation**.
- No missed rising edges on any device under rapid multi-key input at full GPU load.
- Absolute knobs (APC, MPK, X-Touch fader) never jump a parameter after a bank change under the `scale` policy.
- Feedback traffic on every device stays within its declared rate cap during a beat-synchronised decorative animation, and the priority ordering is observably enforced when a device saturates.
- A full-grid SysEx refresh occurs only on connect, mode change, and bank change — never per frame.
- MIDI learn captures a binding on each device, correctly infers absolute versus relative for its continuous controls, and the persisted TOML reloads identically.
- **Adding a seventh controller requires one TOML profile and no engine change**, with a Rust driver needed only if its LED protocol is SysEx bulk. Demonstrate this with any borrowed controller.

## Gate I — Cross-platform parity · new

The same show file, the same clip masters, the same bindings, on all three platforms.

**Pass:**

- A show authored at the Workstation tier loads at the Appliance tier through its declared degradation block, with layer count and render scale reduced exactly as declared and nothing silently dropped.
- A show whose `min_tier` exceeds the running tier refuses to load and names the shortfall. It does not half-load.
- A show referencing a clip with no rendition for the running tier fails with the list of clips needing transcoding, and `vj-ingest` resolves it.
- Golden-image comparison per blend mode matches across backends within a documented tolerance.
- Bindings authored on one platform load unchanged on the others; only transport identity differs.
- CI builds all three targets on every change to a shared crate.

## Gate J — Tempo and sync · new

**Pass:**

- A Link peer (rekordbox or Serato on the laptop) is discovered over the gig network path, and engine tempo matches within one beat of the DJ software changing tempo.
- Phase error between the engine's `bar_phase` and the DJ software's bar stays within a documented tolerance over 10 minutes.
- Authority handover Link → Audio (unplug the network) and Audio → Link (replug) produces no visible phase jump.
- A quantized launch lands within one frame of its target boundary at p95, measured from the presentation timestamp.
- A press 50 ms after a bar boundary still fires on that bar rather than waiting a full bar.
- Audio-only tempo tracking holds through a 32-bar section with a sparse breakdown without `u_bpm` dropping to zero, and `u_beat_confidence` decays smoothly rather than snapping.

---

## Gate status

| Gate | Pi 5 | Windows / 3090 | macOS / M2 Pro |
|---|---|---|---|
| A — decode | not measured | not measured | not measured |
| B — renderer | not measured | not measured | not measured |
| C — trigger latency | not measured | n/a until B6 | n/a until B8 |
| D — audio | not measured | not measured | not measured |
| E — ISF | not measured | not measured | not measured |
| F — Stream Deck | not measured | not measured | not measured |
| G — soak | not measured | not measured | not measured |
| H — surfaces | not measured | not measured | not measured |
| I — parity | not measured | not measured | not measured |
| J — tempo | not measured | not measured | not measured |

Keep this table in the repository and update it only with a linked measurement artifact.
