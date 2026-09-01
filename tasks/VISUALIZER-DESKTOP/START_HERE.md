# Desktop Visualizer — Start Here

This packet turns [`docs/handoff/BORIS_STYLE_VISUALIZER.md`](../../docs/handoff/BORIS_STYLE_VISUALIZER.md) into small coding tasks for the Rust desktop application in `vj/`.

## Orchestrator instructions

1. Read only this file, [`MASTER_PLAN.md`](MASTER_PLAN.md), and [`BOARD.md`](BOARD.md).
2. Dispatch one task per subagent using [`DISPATCH_PROMPTS.md`](DISPATCH_PROMPTS.md).
3. A worker reads its one spec plus only the source files named by that spec.
4. Independent tasks may run in parallel only when the dependency graph permits it.
5. Workers append a report of at most ten lines to [`RESULTS.md`](RESULTS.md).
6. The orchestrator alone updates `BOARD.md` after reviewing the observable.
7. `DONE` requires the observable on the stated hardware. Compilation-only work is `NEEDS-REVIEW` when a physical GPU/audio device is required.

Do not ask workers to read the Boris documentation, the supplied video, the older `gig/` browser implementation, or the full `docs/v2` set. The relevant contracts and constants are inlined in each task.

## Target

- Application: `vj/crates/vj-app`
- Renderer: `vj/crates/vj-render`, wgpu 0.20
- Audio: `vj/crates/vj-audio`, 48 kHz stereo input
- Primary hardware: Windows workstation with RTX 3090
- Initial output: 1920x1080 at 60 Hz
- Pixel pipeline: linear `Rgba16Float` intermediates, sRGB presentation
- Audio textures: 256×1 FFT and 256×1 waveform

## First dispatch

Dispatch `VZ-01`, `VZ-02`, and `VZ-04` in parallel. Do not dispatch any later task until its listed dependencies are reviewed.

