# VZ-07 — Analytic Cartesian and radial primitives

## Goal

Render bars, dots, peak dots, and lines from the FFT texture with one fullscreen pass.

## Existing files allowed

- Read `vj/crates/vj-render/src/audio_gpu.rs`.
- Modify `vj/crates/vj-render/src/lib.rs`; create `vj/crates/vj-render/src/primitives.rs` and `vj/crates/vj-render/src/shaders/visualizer_primitives.wgsl` as part of this task.

## Requirements

Create a prepared render pipeline targeting `Rgba16Float`. The WGSL shader must support Cartesian, mirrored, and radial layouts; rounded bars, circles, peak dots, and antialiased lines; 16–256 bins; three-stop gradient; scale, width/density, taper, inner/outer radius, rotation, angular repeats, and swirl.

One output pixel fetches only the bins it needs; no loop over all 256 bins. Clamp texture coordinates and protect division by zero. Transparent background outside the primitive.

## Verification

Add a small demo/test path using a deterministic ramp FFT. On RTX 3090 capture Cartesian bars and radial dots at 1920x1080. Observable: bin order is low-to-high, center/radius is correct, no NaNs or edge streaks, and resizing preserves aspect.
