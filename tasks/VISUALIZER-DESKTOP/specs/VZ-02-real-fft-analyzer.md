# VZ-02 — Real FFT analyzer

## Goal

Replace the analyzer’s approximate band extraction with a real, allocation-free steady-state FFT path while preserving `AudioFeatures`.

## Existing files allowed

- Modify `vj/crates/vj-audio/Cargo.toml`.
- Modify `vj/crates/vj-audio/src/analyzer.rs`.

## Requirements

Add compatible `realfft`/`rustfft` dependencies. Use a 2048-sample Hann window at 48 kHz, an overlap accumulator with 256-sample hop, power spectrum, log-frequency aggregation to `spectrum_len` bins, and fixed arrays/preallocated vectors owned by `AudioAnalyzer`. No allocation or FFT-plan creation occurs in `process_block` after construction.

Bands: sub-bass 20–75 Hz, bass 75–250 Hz, low-mid 250–500 Hz, mid/high-mid 500–4000 Hz, treble 4000–16000 Hz. Preserve dual-rate envelopes. Silence must produce finite zeros after the noise gate; never normalize a frame by its own maximum. Retain a slowly adapting upper envelope per perceptual bin.

## Verification

Add unit tests in `analyzer.rs` generating silence, 60 Hz, 1 kHz, and 8 kHz sine blocks. Assert the intended band dominates and every output is finite. Run:

```text
cargo test -p vj-audio analyzer
```

Observable: all four signals classify correctly, and a debug allocation counter or documented test hook shows no capacity growth after warm-up.

