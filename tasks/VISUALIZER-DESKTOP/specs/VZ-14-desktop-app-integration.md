# VZ-14 — Desktop application integration

## Goal

Wire live audio features, reducer state, prepared visualizers, and the wgpu surface into the desktop application loop.

## Existing files allowed

- Modify `vj/crates/vj-app/src/runner.rs`.
- Modify `vj/crates/vj-app/src/main.rs`.

## Requirements

On Workstation tier, create the desktop render runtime and run the winit loop. Publish the newest audio snapshot without blocking render. Convert reducer effects into prepare/activate/reset requests. Render the active visualizer to a layer and present it. Handle resize, surface loss, audio absence, shutdown, and a failed preset while preserving the last good output.

Add CLI options `--visualizer ID`, `--synthetic-audio`, and `--window WIDTHxHEIGHT`. Synthetic mode produces deterministic multiband/beat data and must not require an audio device. No shader compilation or file I/O occurs inside the frame callback.

## Verification

On RTX 3090 run a show with one preset in synthetic mode and live-input mode. Observable: animated responsive output, resize/minimize recovery, preset failure preserves output, and clean close exits without panic.

