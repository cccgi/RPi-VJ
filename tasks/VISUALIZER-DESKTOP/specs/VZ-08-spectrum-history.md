# VZ-08 — Persistent spectrum-history ring

## Goal

Create a bounded 256×128 GPU history texture for flowing surfaces and ribbons.

## Existing files allowed

- Read `vj/crates/vj-render/src/audio_gpu.rs`.
- Modify `vj/crates/vj-render/src/lib.rs`; create `vj/crates/vj-render/src/spectrum_history.rs` and its WGSL helper as part of this task.

## Requirements

Implement a ring-indexed `R32Float` texture. Write the newest 256-bin row without shifting older rows. Expose current row, active rows, age-to-row mapping, clear/reset, and a texture view/sampler. A sequence may be inserted only once. Workstation capacity is 128 rows; design constructor capacity so lower tiers can choose 32/64.

Include a visualization shader/test that maps frequency to X and history age to Y. Avoid per-frame allocation and full-texture copies.

## Verification

Insert numbered rows through wraparound and read back selected rows. Observable: newest/oldest ordering remains correct before and after wrap; reset returns all zeros.
