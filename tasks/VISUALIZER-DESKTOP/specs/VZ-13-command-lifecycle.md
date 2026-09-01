# VZ-13 — Visualizer command and reducer lifecycle

## Goal

Add control-plane selection, preparation, activation, degradation, reset, and failure behavior.

## Existing files allowed

- Modify `vj/crates/vj-core/src/lib.rs`.
- Read `vj/crates/vj-types/src/visualizer.rs`.

## Requirements

Add commands: `SelectVisualizer { request_id, layer, visualizer }` and `ResetVisualizerHistory { layer }`. Add effects for asynchronous preparation and frame-boundary activation. Add events: `VisualizerPreparing`, `VisualizerReady`, `VisualizerActivated`, `VisualizerDegraded`, and `VisualizerFaulted` with diagnostics.

Reducer state stores desired and active visualizer separately. Command acceptance never claims visibility. Failure retains the prior active visualizer. Unknown layer/asset and tier over-budget produce typed rejection/fault events.

## Verification

Unit-test success, preparation failure, degraded tier selection, reset, and stale-ready completion after a newer request. Observable: stale completion cannot replace the latest desired preset.

