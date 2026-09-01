# VZ-01 — Typed visualizer contracts

## Goal

Add serializable, GPU-independent types describing a visualizer preset and its bounded resource requirements.

## Existing files allowed

- Modify `vj/crates/vj-types/src/lib.rs`.
- Create and modify `vj/crates/vj-types/src/visualizer.rs`.

## Required API

Define and re-export: `VisualizerId(String)`, `AudioDomain { Frequency, Waveform }`, `PrimitiveStyle { Bars, Dots, PeakDots, Line, Ribbon, Surface }`, `VisualizerLayout { Cartesian, Mirrored, Radial, Spiral, Tunnel }`, `AudioChannel { Mix, Left, Right, Mid, Side }`, `ShapeParams`, `PolarParams`, `Gradient3`, `PeakHoldParams`, `FeedbackParams`, `BloomParams`, `ModulationSource`, `ModulationBinding`, `VisualizerSpec`, and `VisualizerResourcePlan`.

All public preset types derive `Clone`, `Debug`, `PartialEq`, `Serialize`, and `Deserialize`. Defaults must be finite and safe. `VisualizerResourcePlan` includes pass count, persistent target count, target scales, FFT bins, and estimated bytes. Add `validate_for(TierBudget)` that rejects more than eight Workstation passes, unsupported scales outside `{0.25,0.5,1.0}`, non-finite values, invalid frequency ranges, and bins outside `16..=256`.

## Verification

Add tests in `visualizer.rs` for JSON round trip and rejection of an over-budget nine-pass preset. Run:

```text
cargo test -p vj-types visualizer
```

Observable: both tests pass and malformed/non-finite configurations return a typed validation error without panic.

