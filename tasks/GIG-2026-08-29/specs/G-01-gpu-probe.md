# G-01 — Pi 5 GPU + WebGL2 Capability Probe
**Model:** cheap · **Blocks:** everything · **Do this first.**

## Why
No fps number for this hardware exists anywhere in the repo. Every later budget
(internal render scale, shader complexity, post-chain depth) is a guess until
this task produces real numbers. Do not let anyone write renderer code first.

## Deliverables
1. `gig/probe/probe.html` — standalone, no build step, no imports, no CDN.
2. `gig/probe/run-probe.sh` — launches Chromium on the Pi pointed at it.

## probe.html requirements
On load, render into a `<pre>`:
- `navigator.userAgent`
- WebGL2: `VERSION`, `SHADING_LANGUAGE_VERSION`, `RENDERER`, `VENDOR`
  (use the `WEBGL_debug_renderer_info` extension for unmasked strings; fall
  back to the masked ones)
- `MAX_TEXTURE_SIZE`, `MAX_FRAGMENT_UNIFORM_VECTORS`, `MAX_VARYING_VECTORS`
- whether `EXT_color_buffer_float` and `OES_texture_float_linear` are available
  (the post-chain in G-12 wants float FBOs — we need to know now)

Then run a **benchmark sweep**. A fullscreen quad with a fragment shader doing
a configurable number of raymarch steps. For each combination of:
- canvas backing size: `1920x1080`, `1440x810`, `1280x720`, `960x540`
- march steps: `16`, `32`, `64`, `96`

…render for 3 seconds, discard the first 30 frames, and report **median frame
time in ms and the resulting fps**, as a plain text table. Use
`performance.now()` deltas inside `requestAnimationFrame`, not an fps counter
that averages everything.

The benchmark shader must be representative, not trivial — a sphere-tracing
loop over a repeating SDF with a normal calculation and one light:

```glsl
float map(vec3 p){ p = mod(p+2.0, 4.0) - 2.0; return length(p) - 1.0; }
// march STEPS times, compute normal by 4-tap tetrahedron, one diffuse light
```
Put `STEPS` in via `#define` and recompile per sweep row.

Finish with a one-line verdict printed at the bottom:
`RECOMMENDED: <WxH> @ <steps> steps → <fps> fps`
choosing the largest size/steps combination that held **≥58 fps median**.

## run-probe.sh
```sh
#!/bin/sh
chromium-browser --kiosk --enable-features=VaapiVideoDecoder \
  --ignore-gpu-blocklist --enable-gpu-rasterization \
  --autoplay-policy=no-user-gesture-required \
  "file://$(cd "$(dirname "$0")" && pwd)/probe.html"
```
Try `chromium` if `chromium-browser` is absent. Make it executable.

## Observable (this is what DONE means)
Run it on the **Pi 5, plugged into the actual TV**, and paste the full table
plus the RECOMMENDED line into `../RESULTS.md`.

Also record, from an SSH session while the sweep runs:
`vcgencmd measure_temp` and `vcgencmd get_throttled` at the start and end.

## Do not
- Do not add a UI, controls, or styling beyond legible monospace on black.
- Do not import anything. One file, no dependencies.
- Do not report a single averaged fps — the sweep table is the deliverable.
