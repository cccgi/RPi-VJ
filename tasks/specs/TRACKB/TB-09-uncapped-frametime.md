# Task Specification: TB-09 — uncapped GPU frame time and percentile summary

Model: mid. Machines: the RTX 3090 PC first, then the M2 Pro Mac.

## 1. Goal
TB-07's CSV measures the interval between frame starts under `PresentMode::Fifo`, so every row is really the
vsync period plus jitter — it cannot show how much GPU headroom exists. Gate B needs p99 < 16.0 ms and
p99.9 < 16.67 ms of actual render cost. Add an uncapped mode and a percentile summary.

## 2. Target Files
* **Modify**: `vj/tools/gpu-spike/src/main.rs`
* **Modify**: `vj/tools/gpu-spike/src/metrics.rs`

Read those two files as your source of truth. Do not read anything else in `tasks/` or `docs/`.

## 3. Requirements & Constraints
1. **Present mode from the CLI.** Accept an optional second positional arg after the CSV path:
   `fifo` (default, current behaviour) or `immediate`. Map to `wgpu::PresentMode::Fifo` /
   `wgpu::PresentMode::Immediate`. Before selecting `Immediate`, check it is in
   `surface.get_capabilities(&adapter).present_modes`; if it is not, fall back to `Mailbox`, and if that is
   also absent fall back to `Fifo` — print which one you ended up with and record it in the CSV.
2. **Add two columns** to the CSV, so the header becomes exactly:
   `frame,wall_time_s,frame_ms,cpu_ms,fps,present_mode,adapter_name,backend`
   `cpu_ms` is the time spent inside your own render function — start an `Instant` at the top of `render()`
   (after `get_current_texture()` returns) and stop it after `queue.submit(...)`, before `frame.present()`.
   `frame_ms` keeps its current meaning (start-of-frame to start-of-frame) so old runs stay comparable.
   `present_mode` is the string actually in use, repeated each row like `adapter_name` already is.
3. **Percentile summary on exit.** In the `CloseRequested` handler, after flushing, compute over all recorded
   frames *excluding the first 60* (swapchain warm-up — the PC run showed an 800–2000 fps burst in the first
   few frames, which would poison a mean): count, mean / p50 / p99 / p99.9 / max for both `frame_ms` and
   `cpu_ms`. Keep the raw values in a `Vec<f32>` in `MetricsLogger` and sort a copy; do not add a dependency
   for this. Print the summary to stdout as plain lines and also write it to a sibling file next to the CSV
   named `<csv-stem>-summary.txt`.
4. Do not change the shader, the window size, or the uniform layout.

## 4. Acceptance Criteria & Observable
On each machine, two runs of at least 60 seconds each:
```
cargo run -p gpu-spike --release -- gpu-spike-fifo-<pc|mac>.csv fifo
cargo run -p gpu-spike --release -- gpu-spike-immediate-<pc|mac>.csv immediate
```
The immediate run must show a `frame_ms` mean clearly below the fifo run's ~16.67 ms (if it does not, say so —
that means the present mode did not actually change and is a finding, not a pass). Paste the four
`-summary.txt` files verbatim.

## 5. Report
Append ≤10 lines to `tasks/RESULTS-TRACKB.md`: the four summaries (as a pasted block, which does not count
against the line limit), which present mode each machine actually granted, and whether `cpu_ms` and `frame_ms`
diverge the way you'd expect. `DONE` requires runs on both physical machines; one machine only is
`NEEDS-REVIEW`.
