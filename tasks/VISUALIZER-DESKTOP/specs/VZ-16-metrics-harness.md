# VZ-16 — Metrics and preset-cycle harness

## Goal

Measure frame timing and resource stability across the three MVP presets.

## Existing files allowed

- Modify `vj/crates/vj-app/src/main.rs`.
- Create `vj/crates/vj-app/src/visualizer_metrics.rs`.

## Requirements

Add `--visualizer-bench SECONDS`, `--cycle-ms N`, and `--metrics-csv PATH`. Log frame index, preset, CPU frame ms, GPU ms when timestamp queries are supported, present interval, missed-frame flag, render-target bytes, pool-created count, and process RSS when available without a new heavy dependency.

Cycle the three presets after prewarming them. Print p50/p95/p99 and memory delta. Metrics collection must be bounded and streamed to disk off the render-critical section.

## Verification

Run 10 minutes at 1920x1080 synthetic audio on RTX 3090. Observable: CSV parses, all presets appear, p99 GPU time is reported or explicitly unsupported, pool count stabilizes, and memory delta is stated.

