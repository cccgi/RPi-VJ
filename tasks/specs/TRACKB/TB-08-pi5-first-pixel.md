# Task Specification: TB-08 — gpu-spike first pixel on the Pi 5 (V3DV/Vulkan)

**Machine: Raspberry Pi 5, Pi OS Bookworm. Do NOT run this on the gig Pi before 2026-08-29.**
Model: cheap. This is a build-and-observe task, not a design task.

## 1. Goal
Find out whether the existing `gpu-spike` binary comes up at all on the Pi 5's V3DV Vulkan driver, and record
what it reports. Nothing is being optimised here. A clean failure is a completely acceptable result — it is
the answer ADR-100 needs.

## 2. Target Files
* **Modify (only if required to build)**: `vj/tools/gpu-spike/src/main.rs`
* **New (output, not source)**: `vj/tools/gpu-spike/gpu-spike-results-pi.csv`

Read the current `vj/tools/gpu-spike/src/main.rs` as your source of truth for what the program does. Do not
read anything else in `tasks/` or `docs/`.

## 3. Requirements & Constraints
1. Build and run: `cargo run -p gpu-spike --release -- gpu-spike-results-pi.csv`
2. The program already selects `wgpu::Backends::PRIMARY` and picks a backend automatically. **Do not hardcode
   a backend.** If PRIMARY finds no adapter, retry once with `wgpu::Backends::all()` and report which one it
   was that worked — that difference is itself the finding.
3. Allowed changes: whatever is needed to compile and run on aarch64 Linux — a missing system package, a
   linker flag, a winit backend feature. **Not allowed:** changing the shader, the window size, the present
   mode, or the metrics format. Those are other tasks' variables and changing them makes the runs
   incomparable.
4. Run it from a desktop session or with a display attached. If wgpu reports no surface / no adapter, capture
   the exact error text — do not work around it by switching to a headless path.
5. Record, by running these commands on the Pi and pasting the output into your report:
   - `vulkaninfo --summary | head -20` (install `vulkan-tools` if absent)
   - `glxinfo -B | grep -i "OpenGL renderer\|OpenGL ES"` (package `mesa-utils`) — this is the GLES fallback's
     baseline and we want it recorded in the same session
   - `vcgencmd measure_temp` before starting and immediately after closing the window

## 4. Acceptance Criteria & Observable
**Success looks like:** a 960×540 window on the Pi showing the same animated grid pattern the PC and Mac show,
running for at least 60 seconds, plus a non-empty `gpu-spike-results-pi.csv` whose `adapter_name` column is
whatever V3DV reports (expected to contain "V3D" — report the exact string).
**Failure also counts as a completed task** if you report: the exact error, at which stage it happened
(instance creation / adapter request / surface creation / device request / pipeline creation / first present),
and the two `vulkaninfo`/`glxinfo` outputs above.
Either way, paste the first 3 and last 3 CSV rows if a CSV was produced, and state the rough average fps.

## 5. Report
Append ≤10 lines to `tasks/RESULTS-TRACKB.md` (append, do not overwrite): status, exact adapter line or exact
error, average fps, the two temperature readings, anything you had to install or change to make it build.
Command output pasted as a block does not count against the 10 lines. Mark `DONE` only if you saw the window
on a physical Pi 5; if it only compiled, mark `NEEDS-REVIEW` and say so.
