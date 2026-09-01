# Task Specification: TB-12 — 30-minute soak with external thermal/memory sampling

Model: cheap. Machines: all three (Pi only after 2026-08-29). Depends on TB-11 — run the final binary.

## 1. Goal
Gate B's remaining pass conditions are "no unbounded memory growth" and "no thermal throttling with production
cooling," both over 30 minutes. No code changes at all in this task: run the thing and watch it.

## 2. Target Files
* **Modify**: none. **New (outputs)**: `vj/tools/gpu-spike/soak-<pc|mac|pi>.csv` and
  `vj/tools/gpu-spike/soak-<pc|mac|pi>-telemetry.csv`

If any code change seems necessary, stop and report `BLOCKED` — do not edit.

## 3. Requirements & Constraints
1. Run the spike for **at least 32 minutes**, uninterrupted, with the window visible and not minimised:
   `cargo run -p gpu-spike --release -- soak-<machine>.csv immediate heavy import`
2. In a **second terminal**, start a telemetry sampler at 5-second intervals for the same duration, writing
   `soak-<machine>-telemetry.csv` with a timestamp column plus:
   - PC: `nvidia-smi --query-gpu=timestamp,temperature.gpu,utilization.gpu,memory.used,clocks.sm --format=csv -l 5`
   - Mac: `sudo powermetrics --samplers gpu_power -i 5000` piped to the file (raw text is acceptable here; note
     in your report that it is not CSV)
   - Pi: a 5-second loop of `vcgencmd measure_temp`, `vcgencmd get_throttled`, and `free -m`
3. Sample the spike process's own resident memory at start, +10 min, +20 min, +30 min (Task Manager /
   Activity Monitor / `ps -o rss= -p <pid>`) and record all four numbers.
4. Do not touch the machine during the run — no other GPU app, no sleep, no lid close.

## 4. Acceptance Criteria & Observable
- A soak CSV with ≥ 30 minutes of rows and a summary file from it.
- The four RSS readings, and whether the last is more than 10% above the first.
- The telemetry file, and the max temperature reached.
- On the Pi specifically: `vcgencmd get_throttled` must be reported verbatim; anything other than
  `throttled=0x0` at the end is a Gate B failure and must be stated as such.
- Compare the percentile summary from the last 5 minutes against the first 5 minutes (you can do this by
  splitting the CSV on `wall_time_s` in a spreadsheet or a two-line script). Frame times drifting upward over
  30 minutes is the throttling signature and is the thing this task is looking for.

## 5. Report
Append ≤10 lines to `tasks/RESULTS-TRACKB.md`: per machine — run length, first-5-min vs last-5-min p99, the
four RSS numbers, max temperature, throttle state. State plainly for each machine whether it passes Gate B's
memory and thermal conditions. `DONE` requires all three machines; fewer is `NEEDS-REVIEW` naming which ran.
