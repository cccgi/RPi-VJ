# Task Specification: TB-04 — Render a hardcoded triangle

## 1. Goal
Replace the clear-color-only render pass from TB-02/TB-03 with one that draws a single hardcoded RGB triangle,
using a real vertex+fragment shader pipeline. First actual draw call in the program.

## 2. Target Files
* **Modify**: `vj/tools/gpu-spike/src/main.rs`
* **New**: `vj/tools/gpu-spike/src/shader.wgsl`

## 3. Requirements & Constraints
1. `shader.wgsl` — a minimal WGSL pair:
   ```wgsl
   struct VertexOutput {
       @builtin(position) clip_position: vec4<f32>,
       @location(0) color: vec3<f32>,
   };

   @vertex
   fn vs_main(@builtin(vertex_index) idx: u32) -> VertexOutput {
       var positions = array<vec2<f32>, 3>(
           vec2<f32>( 0.0,  0.6),
           vec2<f32>(-0.6, -0.6),
           vec2<f32>( 0.6, -0.6),
       );
       var colors = array<vec3<f32>, 3>(
           vec3<f32>(1.0, 0.2, 0.2),
           vec3<f32>(0.2, 1.0, 0.2),
           vec3<f32>(0.2, 0.2, 1.0),
       );
       var out: VertexOutput;
       out.clip_position = vec4<f32>(positions[idx], 0.0, 1.0);
       out.color = colors[idx];
       return out;
   }

   @fragment
   fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
       return vec4<f32>(in.color, 1.0);
   }
   ```
2. In `main.rs`, after device creation and before the render loop: load this shader with
   `device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("triangle"), source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()) })`,
   build a `RenderPipeline` with an empty `PipelineLayout` (no bind groups yet — that's TB-06), no vertex
   buffers (positions come from the `vertex_index` array in the shader), `PrimitiveTopology::TriangleList`,
   and a fragment target matching the surface's chosen format from TB-02.
3. In the render pass (replacing the no-draw-calls pass from TB-02): `render_pass.set_pipeline(&pipeline);
   render_pass.draw(0..3, 0..1);` — keep the same `Clear` background color as TB-02 as the load op.
4. Do not add a vertex buffer or index buffer — this triangle is deliberately fully procedural in the shader,
   keeping this task's diff small. Buffers come in TB-05.

## 4. Acceptance Criteria & Observable
Run `cargo run -p gpu-spike --release` on the 3090 PC. The window must show the blue-teal background from
before, with a triangle in the center whose three corners are red, green, and blue, smoothly interpolated
across the face (standard barycentric color interpolation — if it's not smoothly interpolated, something is
wrong with your `@location(0)` wiring). No crash, no validation errors in stderr.

## 5. Report
Append to `tasks/RESULTS-TRACKB.md`: confirm the triangle rendered with correct interpolated colors, any wgpu
validation warnings/errors seen in stderr even if non-fatal, anything that surprised you. ≤10 lines.
