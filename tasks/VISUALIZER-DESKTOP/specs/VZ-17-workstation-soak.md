# VZ-17 — RTX 3090 workstation soak

## Goal

Validate gig stability. This is run-only; do not edit source.

## Existing files allowed

- Read `tasks/VISUALIZER-DESKTOP/RESULTS.md` for the VZ-16 command/results.
- Read `tasks/VISUALIZER-DESKTOP/BOARD.md` to confirm prerequisites.

## Procedure

Run the release desktop app for eight hours at 1920x1080/60 with live USB audio for at least two hours and deterministic synthetic audio for the remainder. Cycle the three presets every 10–30 seconds. Exercise resize/fullscreen, audio disconnect/reconnect, invalid preset selection, controller command bursts, and history reset. Record CSV metrics and application logs.

## Pass criteria

- No panic, device loss, black unrecovered output, or unbounded queue.
- Previous visual remains during invalid preset activation.
- Audio reconnects without restarting the renderer.
- p99 GPU frame time below 14 ms for the standard tier.
- Pool/resource counts stabilize; RSS/GPU memory show no monotonic growth.

Append exact duration, failures, p99, starting/ending memory, and recommended status to `RESULTS.md`. `DONE` requires the physical RTX 3090 run.

