# Task Specification: TB-05 — Fullscreen quad with one ISF-lite fragment shader

## 1. Goal
Replace the triangle from TB-04 with a fullscreen quad rendering one of the gig's existing fragment shaders
(reused as-is, not the ISF parser crate — that integration is a later task), with static/hardcoded uniform
values. Proves the render target size and quad geometry are right before wiring live uniforms in TB-06.

## 2. Target Files
* **Modify**: `vj/tools/gpu-spike/src/main.rs`
* **New**: `vj/tools/gpu-spike/src/quad.wgsl`
* Read (do not modify) exactly one existing shader for reference: `gig/shaders/cyber_grid.fs` — this is a GLSL
  ISF-style shader; you are PORTING its fragment logic to WGSL by hand for this spike, not running it as-is.
  Wgpu does not consume GLSL directly.

## 3. Requirements & Constraints
1. `quad.wgsl` vertex stage: same procedural-vertex-index trick as TB-04, but 6 vertices (two triangles) covering
   clip space -1..1 on both axes, with a `@location(0) uv: vec2<f32>` output in 0..1 range (flip Y as needed so
   `uv.y == 0` is the top of the screen, matching how `cyber_grid.fs` expects its coordinate).
2. Fragment stage: hand-port `cyber_grid.fs`'s core math into WGSL. Keep it faithful but you may simplify if a
   GLSL builtin has no direct WGSL equivalent (e.g. `mod()` → WGSL's `%` behaves differently for negatives; use
   `fract()`-based patterns instead and note this in your report). Hardcode whatever uniforms that shader reads
   (time, resolution, audio bands) as WGSL `const` values for now — e.g. `const U_TIME: f32 = 3.0;`. Live
   uniforms are TB-06, not this task.
3. Build the pipeline the same way as TB-04 (empty layout, no bind groups, `TriangleList`, 6 vertices,
   `draw(0..6, 0..1)`), just with `quad.wgsl` instead of `shader.wgsl`. You can delete or keep TB-04's triangle
   pipeline — your choice, note which in your report.

## 4. Acceptance Criteria & Observable
Run `cargo run -p gpu-spike --release` on the 3090 PC. The window must fill entirely with the `cyber_grid`
pattern (not just a corner or a stretched/squashed version of it — aspect ratio and UV orientation must look
right by eye, compared against how `cyber_grid` looks in the gig build if you have it running for reference).
No crash, no validation errors.

## 5. Report
Append to `tasks/RESULTS-TRACKB.md`: confirm the pattern filled the window correctly, any GLSL→WGSL semantic
differences you had to work around (mod, sign, texture wrapping, etc. — these matter for the real ISF host
later), anything that surprised you. ≤10 lines.
