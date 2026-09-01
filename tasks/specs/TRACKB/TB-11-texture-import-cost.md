# Task Specification: TB-11 — per-frame texture import and its copy cost

Model: mid. Machines: the RTX 3090 PC first, then the M2 Pro Mac, then the Pi 5 (Pi only after 2026-08-29).
Depends on TB-10.

## 1. Goal
This is the task ADR-100's escape-hatch clause actually turns on: whether wgpu can take a decoded frame per
frame without a CPU round-trip that blows the budget. Real decode is B3's job. This task measures the *upload
path alone*, which is the part that would be shared by every decoder backend, using a synthetic frame.

## 2. Target Files
* **Modify**: `vj/tools/gpu-spike/src/main.rs`
* **New**: `vj/tools/gpu-spike/src/source.rs`

Read the current `main.rs` as your source of truth. Do not read anything else in `tasks/` or `docs/`.

## 3. Requirements & Constraints
1. `source.rs`: a `SyntheticSource` holding one CPU-side `Vec<u8>` of 1920×1080×4 bytes (BGRA-order data is
   fine, declare the texture format to match). Each frame, mutate it cheaply on the CPU — e.g. write a moving
   32-pixel-wide vertical bar by memsetting only the rows/columns it touches — so the buffer is genuinely
   dirty every frame and the driver cannot elide the upload. Do **not** regenerate all 8.3 MB per frame with a
   per-pixel loop; that measures your Rust, not the import path.
2. Upload it each frame with `queue.write_texture` into a 1920×1080 `Rgba8UnormSrgb` (or `Bgra8UnormSrgb`)
   texture with usage `TEXTURE_BINDING | COPY_DST`. Time **only** the `write_texture` call and add a column
   `upload_ms` to the CSV (header becomes
   `frame,wall_time_s,frame_ms,cpu_ms,upload_ms,fps,present_mode,adapter_name,backend`). Include `upload_ms` in
   the percentile summary alongside the other two.
3. Bind that texture into pass 1 and have `heavy.wgsl` sample it once (mix it 50/50 with its procedural output)
   so the upload cannot be dead-code-eliminated and you can see on screen that the bar is moving.
4. Add a CLI switch (fourth positional arg, `import` | `noimport`, default `import`) that skips step 2 and 3's
   sample entirely, so each machine produces a matched pair with and without the import.
5. **Report, do not fix.** If `write_texture` is slow, that is the result. Do not go implement staging-buffer
   ring bufferring, `wgpu-hal` external memory, or a DMA-BUF path — those are B3 decisions and need this
   number first.

## 4. Acceptance Criteria & Observable
On each machine, at least 60 seconds each:
```
cargo run -p gpu-spike --release -- r-import.csv   immediate heavy import
cargo run -p gpu-spike --release -- r-noimport.csv immediate heavy noimport
```
The window must show the moving bar blended into the kaleidoscope. Paste both summary files.
The number this task exists to produce: **mean and p99 `upload_ms`, and the delta in `frame_ms` p99 between the
import and noimport runs, per machine.** State those explicitly.

## 5. Report
Append ≤10 lines to `tasks/RESULTS-TRACKB.md`: the per-machine upload_ms mean/p99, the import-vs-noimport p99
delta, and one sentence on whether 8.3 MB/frame at 60 fps (≈500 MB/s) looked like it was bounded by bus
bandwidth, by the CPU-side memcpy, or by neither. Flag to the architect if the Pi's number differs from the
desktops' by more than the raw bandwidth ratio would explain — that is the GLES-fallback trigger.
