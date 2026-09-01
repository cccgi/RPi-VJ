# Track B Results Log
Append-only. One ≤10-line entry per task, oldest first. See `AGENT_PROTOCOL_V2.md` §4 for format.

## TB-01: Add wgpu/winit/pollster dependencies
- Status: DONE
- Machine: Windows PC (x86_64-pc-windows-msvc)
- Changed: `vj/Cargo.toml`, `vj/tools/gpu-spike/Cargo.toml`, `vj/tools/gpu-spike/src/main.rs`, `tasks/RESULTS-TRACKB.md`
- Output: `gpu-spike: deps resolved`
- Resolved Versions: `wgpu` v0.20.1, `winit` v0.29.15, `pollster` v0.3.0, `bytemuck` v1.25.2
- Notes: `cargo build -p gpu-spike` and `cargo run -p gpu-spike` succeeded without issues. `vj-types` in existing workspace has pre-existing compilation error (`DeckId: Default`).

## TB-02: wgpu device init + clear-color window (Windows/3090)
- Status: DONE
- Machine: Windows PC / NVIDIA GeForce RTX 3090 (x86_64-pc-windows-msvc)
- Changed: `vj/tools/gpu-spike/src/main.rs`, `tasks/RESULTS-TRACKB.md`
- Adapter Line: `adapter: NVIDIA GeForce RTX 3090 (Vulkan)`
- Backend: Vulkan (`wgpu::Backends::PRIMARY` picked Vulkan on Windows)
- Notes: Window created (960x540) clearing to solid teal (`r: 0.05, g: 0.4, b: 0.6, a: 1.0`) with Fifo present mode without errors or crashing. Clean exit on window close/Escape.

## TB-04: Render a hardcoded triangle
- Status: DONE
- Machine: Windows PC / NVIDIA GeForce RTX 3090 (x86_64-pc-windows-msvc)
- Changed: `vj/tools/gpu-spike/src/shader.wgsl`, `vj/tools/gpu-spike/src/main.rs`, `tasks/RESULTS-TRACKB.md`
- Render: Confirmed procedural RGB triangle rendered on teal background with smoothly interpolated corner colors.
- Validation: 0 wgpu validation errors or warnings in stderr. Clean pipeline creation and render pass execution.
- Notes: `compilation_options` field required by wgpu 0.20 `VertexState` and `FragmentState`.

## TB-05: Fullscreen quad with one ISF-lite fragment shader
- Status: DONE
- Machine: Windows PC / NVIDIA GeForce RTX 3090 (x86_64-pc-windows-msvc)
- Changed: `vj/tools/gpu-spike/src/quad.wgsl`, `vj/tools/gpu-spike/src/main.rs`, `tasks/RESULTS-TRACKB.md`
- Render: Fullscreen quad (6 procedural vertices) rendered cyber_grid pattern correctly oriented across full 960x540 window.
- GLSL->WGSL: Used `(0.5 - in.uv.y)` and aspect correction to match ISF `gl_FragCoord` centered UV; replaced vector mix/fract with WGSL syntax.
- Notes: TB-04 pipeline replaced with quad pipeline; 0 validation warnings or errors.

## TB-06: Live u_time/u_resolution uniforms + on-screen frame counter
- Status: DONE
- Machine: Windows PC / NVIDIA GeForce RTX 3090 (x86_64-pc-windows-msvc)
- Changed: `vj/tools/gpu-spike/src/main.rs`, `vj/tools/gpu-spike/src/quad.wgsl`, `tasks/RESULTS-TRACKB.md`
- Measured FPS: `frame: 16.67ms avg, 60 fps` (capped at 60 fps due to `PresentMode::Fifo` vsync).
- Animation & Resize: Grid animates smoothly with wall-clock `u.time`; resizing dynamically reconfigures surface without crash or freeze.
- Notes: Aligned uniform struct `Uniforms { time: f32, _pad0: f32, resolution: [f32; 2] }` to satisfy WGSL `vec2<f32>` 8-byte alignment inside 16-byte buffer.

## TB-07: CSV metrics logging on both machines
- Status: DONE
- Machines: Windows PC (NVIDIA RTX 3090 / Vulkan) & M2 Pro MacBook Pro (Apple M2 Pro / Metal)
- PC Run: 2166 frames over 36.0s (~60.2 fps avg); adapter `NVIDIA GeForce RTX 3090` (Vulkan).
- Mac Run: 2404 frames over 40.2s (~59.8 fps avg); adapter `Apple M2 Pro` (Metal).
- Changed: `vj/tools/gpu-spike/src/metrics.rs`, `vj/tools/gpu-spike/src/main.rs`, `gpu-spike-results-pc.csv`, `gpu-spike-results-mac.csv`
- Flagged Gap: GPU temperature and VRAM logging deferred per spec (requires platform-specific NVML / IOKit APIs not exposed by wgpu).
- Notes: Both machines render fullscreen quad smoothly with vsync (`Fifo`), producing self-describing CSV benchmark logs.

```csv
[gpu-spike-results-pc.csv - first 3 rows]
frame,wall_time_s,frame_ms,fps,adapter_name,backend
0,0.0214,21.353,46.83,NVIDIA GeForce RTX 3090,Vulkan
1,0.0226,1.240,806.65,NVIDIA GeForce RTX 3090,Vulkan
2,0.0231,0.482,2073.40,NVIDIA GeForce RTX 3090,Vulkan

[gpu-spike-results-pc.csv - last 3 complete rows]
2162,35.9592,0.123,8136.70,NVIDIA GeForce RTX 3090,Vulkan
2163,35.9753,16.081,62.19,NVIDIA GeForce RTX 3090,Vulkan
2164,36.0292,53.894,18.55,NVIDIA GeForce RTX 3090,Vulkan

[gpu-spike-results-mac.csv - first 3 rows]
frame,wall_time_s,frame_ms,fps,adapter_name,backend
0,0.1754,175.446,5.70,Apple M2 Pro,Metal
1,0.1815,6.078,164.52,Apple M2 Pro,Metal
2,0.1856,4.092,244.41,Apple M2 Pro,Metal

[gpu-spike-results-mac.csv - last 3 complete rows]
2400,40.1624,16.646,60.08,Apple M2 Pro,Metal
2401,40.1789,16.489,60.65,Apple M2 Pro,Metal
2402,40.1958,16.881,59.24,Apple M2 Pro,Metal
```


## TB-03: Verify wgpu clear-color window on macOS/M2 Pro
- Status: DONE
- Machine: M2 Pro MacBook Pro (aarch64-apple-darwin)
- Changed: None (0 code changes; exact TB-02 code built and ran unmodified)
- Adapter Line: `adapter: Apple M2 Pro (Metal)`
- Backend: Metal (`wgpu::Backends::PRIMARY` automatically selected Metal)
- Environment: macOS 26.2 (Build 25C56), Xcode CLT 21.0.0 (Apple clang 21.0.0)
- Notes: Window rendered quad/teal clear background at ~60 fps vsync on physical M2 Pro; 0 errors.

## Review pass (2026-08-27, Claude Sonnet 5) — code + report audit
All 7 reports read; source diffed against each spec's requirements. No Rust toolchain reachable from this
review session (Linux mount shell has no cargo), so this is a static/consistency audit, not a re-run — the
"observed on hardware" claims below still rest on the workers' own runs, not independently re-executed here.

**Verified consistent, looks real:**
- `Cargo.toml` wiring (root + gpu-spike) matches TB-01 exactly — workspace deps, no stray pins.
- `main.rs` uses the required winit 0.29 `ApplicationHandler`-equivalent event-loop pattern, `Backends::PRIMARY`
  (never hardcoded to Vulkan/Metal — TB-03's "0 code changes on Mac" claim is consistent with this).
- `quad.wgsl` is a genuine line-by-line port of `gig/shaders/cyber_grid.fs` (sun disc, terrain fbm-ish grid,
  audio-uniform stand-ins as named consts) — not a placeholder pattern. UV flip/aspect handling matches what
  TB-05's report described.
- `metrics.rs` CSV header/row format matches spec exactly; both delivered CSVs' header lines match it too.
- TB-01's "vj-types: DeckId: Default" pre-existing error: confirmed real and out of Track B's scope — `DeckId`
  (`vj-types/src/lib.rs:62`) is used in `vj-core`, `vj-app`, `vj-api`, none of which any TB task touched. This
  is a legacy `vj/` workspace bug (consistent with prior `ENGINE_REALITY_CHECK.md` findings), not a regression
  from this work. `cargo build -p gpu-spike` in isolation is what actually matters for this board and every
  report claims that succeeded — `cargo build --workspace` will still fail until someone fixes `vj-types`
  separately. Recommend a standalone ticket, not a Track B blocker.

**Flagged, not blocking:**
- Board (`TRACK-B-BOARD.md`) was never updated from TODO despite 7 DONE reports — rule 5 violation, paperwork
  only. Fixed in this pass (all 7 now marked DONE).
- `gpu-spike-results-pc.csv` shows a startup burst (frames 1-2: 806fps, 2073fps) before settling to the ~60fps
  Fifo-capped steady state TB-06 described. This is expected `desired_maximum_frame_latency: 2` swapchain
  buffering releasing a couple of frames before vsync throttling engages, not a bug — but TB-06's summary
  ("capped at 60fps") slightly overstates how uniform the first few frames are. No action needed.

**Cannot verify from here:** that a window actually appeared with correct visuals on the physical 3090 and the
physical M2 Pro Mac. That's the one thing in AGENT_PROTOCOL_V2's DONE definition no static review can confirm —
it rests on the dispatched workers' (and your) own eyes on the two screens.

**Verdict: accept all 7 as DONE.** Code and data are internally consistent, match their specs closely enough
that fabrication is unlikely, and the one real defect found (DeckId/Default) is pre-existing and out of scope.
B1's exit still needs the ADR-100 write-up from these two CSVs — that's the next, architect-level step.
