# VZ-03 — Deterministic audio regression fixtures

## Goal

Protect FFT, smoothing, onset, and channel behavior with black-box tests.

## Existing files allowed

- Read `vj/crates/vj-audio/src/lib.rs`.
- Create `vj/crates/vj-audio/tests/visualizer_features.rs`.

## Requirements

Generate samples in memory; do not add binary WAV files. Test 48 kHz stereo blocks for silence, left-only 60 Hz, right-only 1 kHz, an 8 kHz burst, and a kick-like decaying 55 Hz transient. Process enough blocks to fill the 2048-sample window.

Assert: silence remains below 0.01; the expected spectrum region dominates; waveform samples remain within `[-1,1]`; onset rises for the transient and decays; sequence increases; all values are finite. Do not assert exact floating-point amplitudes.

## Verification

```text
cargo test -p vj-audio --test visualizer_features
```

Observable: deterministic pass on repeated runs.

