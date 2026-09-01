# VZ-06 — Visualizer frame parameter buffer

## Goal

Define the stable CPU/GPU frame parameter layout used by every visualizer shader.

## Existing files allowed

- Read `vj/crates/vj-types/src/visualizer.rs`.
- Modify `vj/crates/vj-render/src/lib.rs`; create `vj/crates/vj-render/src/visualizer_params.rs` as part of this task.

## Requirements

Define `#[repr(C)]` bytemuck-safe structs for time/delta/resolution/frame index, volume/bass/mid/treble/onset, beat/bar phase/BPM/confidence, primitive/layout enums encoded as `u32`, shape/polar values, three linear RGBA colors, and effect amounts. Respect uniform-buffer alignment; include explicit padding.

Implement conversion from validated `VisualizerSpec` plus evaluated modulation values. Clamp and sanitize every externally sourced float. Include a WGSL struct string or matching `.wgsl` declaration generated/maintained in the same new module.

## Verification

Tests assert alignment is a multiple of 16, byte size is stable, enum mappings are stable, and NaN/Inf become safe defaults. Observable: tests pass on x86_64 Windows.
