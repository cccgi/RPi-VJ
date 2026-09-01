# Task Specification: TB-01 — Add wgpu/winit/pollster dependencies

## 1. Goal
Get `wgpu`, `winit`, `pollster`, and `bytemuck` into the workspace so later tasks can open a GPU device. No
rendering code in this task — just dependency wiring that compiles.

## 2. Target Files
* **Modify**: `vj/Cargo.toml` (workspace root)
* **New**: `vj/tools/gpu-spike/Cargo.toml`
* **New**: `vj/tools/gpu-spike/src/main.rs` (stub only — see §3.3)

## 3. Requirements & Constraints
1. In `vj/Cargo.toml`, under `[workspace.dependencies]`, add:
   ```toml
   wgpu = "0.20"
   winit = "0.29"
   pollster = "0.3"
   bytemuck = { version = "1", features = ["derive"] }
   ```
   Do not pin exact patch versions beyond this — `cargo update` will resolve them; record whatever exact
   versions `Cargo.lock` picks in your report (§4).
2. Add `"tools/gpu-spike"` — it is likely already covered by the existing `members = ["crates/*", "tools/*"]`
   glob in `[workspace]`; confirm this and do not duplicate the entry if so.
3. Create `vj/tools/gpu-spike/Cargo.toml`:
   ```toml
   [package]
   name = "gpu-spike"
   version = "0.1.0"
   edition = "2021"

   [dependencies]
   wgpu = { workspace = true }
   winit = { workspace = true }
   pollster = { workspace = true }
   bytemuck = { workspace = true }
   ```
4. Create `vj/tools/gpu-spike/src/main.rs` as a stub that only proves the crate compiles and the deps resolve:
   ```rust
   fn main() {
       println!("gpu-spike: wgpu = {}", wgpu::VERSION.unwrap_or("unknown"));
   }
   ```
   (If `wgpu::VERSION` doesn't exist in the resolved version, just print `"gpu-spike: deps resolved"` instead —
   note in your report which one you used.)
5. Do not touch `vj-render`, `vj-isf`, or any existing crate in this task.

## 4. Acceptance Criteria & Observable
Run `cargo build -p gpu-spike` from `vj/`. It must succeed with no errors. Run `cargo run -p gpu-spike` and paste
the printed line into your report. Also run `cargo build --workspace` once to confirm nothing existing broke.

## 5. Report
Append to `tasks/RESULTS-TRACKB.md`: files changed, the exact `gpu-spike` output line, resolved `wgpu`/`winit`
versions from `Cargo.lock`, anything that surprised you. ≤10 lines.
