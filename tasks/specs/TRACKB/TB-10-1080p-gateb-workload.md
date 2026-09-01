# Task Specification: TB-10 — 1080p offscreen target + Gate-B-weight shader

Model: mid. Machines: the RTX 3090 PC first, then the M2 Pro Mac. Depends on TB-09 (needs its `cpu_ms` column
and percentile summary to be worth anything).

## 1. Goal
The spike currently renders one cheap procedural shader into a 960×540 window. Gate B's workload is 1080p60
with a moderate full-screen shader plus a blend pass. Raise the spike to a comparable weight so the numbers
mean something. Video decode is **not** part of this task (that is TB-11 and B3).

## 2. Target Files
* **Modify**: `vj/tools/gpu-spike/src/main.rs`
* **New**: `vj/tools/gpu-spike/src/heavy.wgsl`

Read the current `main.rs` and `src/quad.wgsl` as your source of truth for the existing pipeline shape and the
uniform struct. Do not read anything else in `tasks/` or `docs/`.

## 3. Requirements & Constraints
1. **Render offscreen at a fixed 1920×1080**, independent of window size:
   - Create an offscreen `wgpu::Texture`, `format: wgpu::TextureFormat::Rgba8UnormSrgb`, size 1920×1080,
     usage `RENDER_ATTACHMENT | TEXTURE_BINDING`.
   - Pass 1: the shader renders into that texture. `u_resolution` in the uniform buffer becomes
     `[1920.0, 1080.0]` for this pass — not the window size.
   - Pass 2: a blit/blend pass samples that texture into the swapchain view with a `wgpu::Sampler`
     (`FilterMode::Linear`), multiplying by a constant opacity of `0.85` and adding a constant
     `vec3(0.02, 0.0, 0.04)` tint. This is the stand-in for Gate B's "one blend and colour-conversion pass" —
     keep it exactly this trivial, its job is to exist, not to look good.
   - The window stays resizable and just shows the scaled result; resizing must not resize the offscreen
     target.
2. **`heavy.wgsl`** is the pass-1 shader. Port `gig/shaders/kaleidoscope_mirror.fs` from GLSL to WGSL — that is
   the third and last repo file you may open. Where it reads audio uniforms that do not exist here, replace
   each with a named `const` driven off `u.time` (e.g. `let bass = 0.5 + 0.5 * sin(u.time * 1.7);`) so the
   shader keeps doing real per-pixel work and its cost does not get optimised away. Keep the existing
   `Uniforms { time: f32, _pad0: f32, resolution: vec2<f32> }` layout — do **not** add uniform fields.
   `quad.wgsl` stays in the repo untouched; add a CLI switch (third positional arg, `light` | `heavy`,
   default `heavy`) that picks which pass-1 shader is used, so the cheap workload is still runnable for
   comparison.
3. Keep TB-09's CSV columns, present-mode arg, and percentile summary working unchanged.

## 4. Acceptance Criteria & Observable
On each machine, four runs of at least 60 seconds:
```
cargo run -p gpu-spike --release -- r-light-fifo.csv fifo light
cargo run -p gpu-spike --release -- r-light-imm.csv  immediate light
cargo run -p gpu-spike --release -- r-heavy-fifo.csv fifo heavy
cargo run -p gpu-spike --release -- r-heavy-imm.csv  immediate heavy
```
The window must show the kaleidoscope pattern, visibly tinted, filling the window at any window size, with no
wgpu validation errors. The heavy/immediate `cpu_ms` and `frame_ms` percentiles must be measurably higher than
light/immediate — if they are identical, the pass-1 switch is not doing anything and that is a bug, not a pass.

## 5. Report
Append ≤10 lines to `tasks/RESULTS-TRACKB.md`: the eight summary blocks (pasted, not counted against the line
limit), and in prose: heavy p99 on each machine, whether either machine misses Gate B's p99 < 16.0 ms at
1080p, and any GLSL→WGSL construct that did not port cleanly (that list is direct input to B2's translator).
