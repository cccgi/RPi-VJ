# VZ-12 — Bounded audio modulation evaluator

## Goal

Evaluate preset modulation bindings once per frame without mutating base parameters.

## Existing files allowed

- Modify `vj/crates/vj-core/src/lib.rs` only to expose the module.
- Create `vj/crates/vj-core/src/modulation.rs`.

## Requirements

Implement fixed/bounded modulation state for sources: volume, bass, mid, treble, onset, beat phase, bar phase, and constant. Formula: sanitize source, optional bipolar conversion, `pow(max(source,0), curve)`, dual-rate attack/release, then `clamp(base + amount * value, min, max)`. `curve` must be positive and finite. Beat/bar phase may wrap but must not be EMA-smoothed unless the binding requests it.

Coalesce duplicate targets so the last binding wins and no output vector grows after construction. Provide reset.

## Verification

Tests cover curve, attack/release rate independence, clamping, NaN sanitation, phase wrap, and reset. Observable: deterministic results at 30, 60, and 120 fps within tolerance.

