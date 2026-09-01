# VZ-09 — Render-target pool and half-resolution bloom

## Goal

Provide bounded reusable render targets and thresholded separable bloom.

## Existing files allowed

- Modify `vj/crates/vj-render/src/lib.rs`.
- Create `vj/crates/vj-render/src/post.rs` plus WGSL shaders beneath `vj/crates/vj-render/src/shaders/`.

## Requirements

Implement a pool keyed by width, height, format, and usage. Targets are prepared outside a live render pass and reused; counters expose created, checked-out, and peak targets. Bloom uses threshold/downsample, horizontal blur, vertical blur, and additive composite at 0.5 scale using `Rgba16Float`.

Radius is bounded and converted to a fixed kernel; do not generate shader source at runtime. Disabling bloom bypasses all bloom passes. Resize retires old targets at a controlled boundary.

## Verification

Render a bright shape with bloom off/on for 300 frames. Observable: visible halo only when enabled, pool-created count stabilizes after warm-up, and resize creates one new size class without frame-by-frame growth.

