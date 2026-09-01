# VZ-10 — Bounded visualizer render graph

## Goal

Compose primitives, optional history/feedback, and bloom into one prepared visualizer instance.

## Existing files allowed

- Read `vj/crates/vj-render/src/primitives.rs`.
- Modify `vj/crates/vj-render/src/lib.rs`; create `vj/crates/vj-render/src/visualizer.rs` as part of this task. Use the public post/history APIs re-exported by `lib.rs`; do not open their implementation files.

## Requirements

Implement `PreparedVisualizer` and `VisualizerRenderer`. Preparation validates the resource plan, creates pipelines/targets, and returns diagnostics without disturbing a live instance. Activation is an atomic/frame-boundary state swap. Render updates audio textures/history/parameters, executes only enabled passes, and returns an `Rgba16Float` layer texture.

Support deterministic `reset_history`. Track per-frame allocations, pass count, target bytes, and last GPU error. A failed replacement keeps the previous instance. Maximum Workstation pass count is eight.

## Verification

Cycle valid primitive-only and history+bloom specs 1,000 times plus an invalid nine-pass spec. Observable: invalid spec is rejected, previous output remains, and allocation/resource counters are flat after each unique valid spec is prepared once.
