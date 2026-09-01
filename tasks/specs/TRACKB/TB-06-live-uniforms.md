# Task Specification: TB-06 — Live u_time/u_resolution uniforms + on-screen frame counter

## 1. Goal
Replace TB-05's hardcoded `const U_TIME` with a real uniform buffer updated every frame from wall-clock time and
actual window size, and print a rolling frame-time readout so later tasks (TB-07) have something to log.

## 2. Target Files
* **Modify**: `vj/tools/gpu-spike/src/main.rs`
* **Modify**: `vj/tools/gpu-spike/src/quad.wgsl`

## 3. Requirements & Constraints
1. Define a `#[repr(C)] #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)] struct Uniforms { time: f32,
   resolution: [f32; 2], _pad: f32 }` (the `_pad` keeps it 16-byte aligned, which WGSL uniform buffers require).
2. Create a `wgpu::Buffer` with `usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST`, size
   `size_of::<Uniforms>()`, and a matching `BindGroupLayout`/`BindGroup` at `@group(0) @binding(0)`. This is the
   first bind group in the program — the pipeline layout from TB-04/TB-05 changes from empty to
   `[&uniform_bind_group_layout]`.
3. In `quad.wgsl`, add:
   ```wgsl
   struct Uniforms { time: f32, resolution: vec2<f32>, _pad: f32 };
   @group(0) @binding(0) var<uniform> u: Uniforms;
   ```
   and replace the `U_TIME` const uses with `u.time`, and any resolution-derived math with `u.resolution`.
4. In the render loop, before each frame: compute elapsed seconds since program start (`std::time::Instant`),
   fill an `Uniforms` value with real time and the current surface size (updated on resize events — handle
   `WindowEvent::Resized` by reconfiguring the surface, same as any wgpu app must), and
   `queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]))` before the render pass.
5. Track frame times with a simple ring buffer (last 60 frames) and every ~1 second print
   `println!("frame: {:.2}ms avg, {:.0} fps", avg_ms, 1000.0/avg_ms);` to stdout. On-screen text rendering is
   out of scope — stdout is the observable for this task.

## 4. Acceptance Criteria & Observable
Run `cargo run -p gpu-spike --release` on the 3090 PC. The `cyber_grid` pattern must now visibly animate (it has
time-driven motion in the original shader — if it's static, `u.time` isn't reaching the shader, fix it).
Resizing the window must not crash or freeze the image. Stdout must print an fps line roughly once per second
showing a plausible number (expect several hundred fps at 960x540 on a 3090 with no post-processing — if it's
wildly different, note the actual number, don't force it to look "right").

## 5. Report
Append to `tasks/RESULTS-TRACKB.md`: confirm animation works, confirm resize doesn't crash, the actual fps
number observed, anything that surprised you. ≤10 lines.
