# VZ-04 — Reusable desktop wgpu surface host

## Goal

Move the proven window/surface lifecycle into `vj-render` without visualizer logic.

## Existing files allowed

- Modify `vj/crates/vj-render/Cargo.toml`.
- Modify `vj/crates/vj-render/src/lib.rs`; create `vj/crates/vj-render/src/surface.rs` as part of this task.

## Requirements

Add a `DesktopSurfaceHost` owning instance, surface, adapter, device, queue, configuration, window size, and surface recovery. Prefer the high-performance adapter. Support resize, `Lost`/`Outdated` reconfiguration, zero-size suspension, `OutOfMemory` fatal return, and sRGB presentation. Expose adapter info and a method that acquires a frame and invokes a supplied render closure.

Add workspace dependencies `wgpu`, `winit`, `pollster`, and `bytemuck`. Expose a small `run_desktop` event-loop helper so `vj-app` does not need a direct `winit` dependency.

Do not copy metrics or the demo shader. No global GPU state.

## Verification

Run existing workspace tests, then exercise through a minimal module-local test/demo path on the physical RTX 3090. Observable: a resizable window clears continuously, reports an NVIDIA RTX 3090 adapter, and survives minimize/restore.
