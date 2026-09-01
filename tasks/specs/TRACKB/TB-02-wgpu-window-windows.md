# Task Specification: TB-02 — wgpu device init + clear-color window (Windows/3090)

## 1. Goal
Open a real window on the RTX 3090 PC, initialize a wgpu device against it, and clear it to a solid color every
frame. This is the first task in the program that actually touches a GPU. Must be run and observed on the 3090
box itself, not just compiled.

## 2. Target Files
* **Modify**: `vj/tools/gpu-spike/src/main.rs` (replaces the TB-01 stub body entirely)
* Read (do not modify) `vj/tools/gpu-spike/Cargo.toml` from TB-01 to confirm deps are present.

## 3. Requirements & Constraints
Write `main.rs` using `winit` 0.29's `ApplicationHandler` pattern (NOT the older `EventLoop::run` closure API,
which is deprecated in 0.29) and `wgpu` 0.20's standard init sequence:

1. Create a `winit::event_loop::EventLoop`, a window titled `"gpu-spike"`, sized 960x540.
2. Create a `wgpu::Instance` with `backends: wgpu::Backends::PRIMARY` (this picks Vulkan on Windows — do not
   hardcode `Backends::VULKAN`, later tasks reuse this same code path on macOS where it must pick Metal).
3. Create a `wgpu::Surface` from the window, request an adapter via
   `instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), ..Default::default() })`,
   using `pollster::block_on`.
4. Request a device+queue from that adapter (default `DeviceDescriptor`, no special features/limits).
5. Configure the surface with `surface.get_capabilities(&adapter).formats[0]`, `PresentMode::Fifo`, size matching
   the window.
6. In the redraw/render loop: acquire the next surface texture, create a command encoder, run one render pass
   with `load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.4, b: 0.6, a: 1.0 })` and no draw calls, submit,
   present.
7. Print the selected adapter's name and backend to stdout once at startup, e.g.
   `println!("adapter: {} ({:?})", info.name, info.backend);` — this is your observable.
8. Handle window close / Escape key to exit cleanly (no panic on exit).

## 4. Acceptance Criteria & Observable
Run `cargo run -p gpu-spike --release` **on the 3090 PC itself**. A window must appear showing a solid
blue-teal color with no flicker or crash, and stdout must print the adapter line naming an NVIDIA device and
`Vulkan` (or `Dx12` if wgpu picks that — note whichever it actually picked, don't force it). Close the window;
process must exit with code 0, no panic in the terminal.

## 5. Report
Append to `tasks/RESULTS-TRACKB.md`: the exact adapter line printed, confirmation the window rendered without
crashing, which backend wgpu actually selected, anything that surprised you. ≤10 lines. This task is `DONE`
only when run on the physical 3090 machine — if you only compiled it, mark `NEEDS-REVIEW` and say so.
