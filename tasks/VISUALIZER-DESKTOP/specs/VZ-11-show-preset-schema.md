# VZ-11 — Show manifest visualizer presets

## Goal

Make visualizers first-class typed show assets.

## Existing files allowed

- Modify `vj/crates/vj-types/src/show.rs`.
- Read `vj/crates/vj-types/src/visualizer.rs`.

## Requirements

Add a visualizer asset collection keyed by `VisualizerId`, a grid slot variant selecting a visualizer and layer, and optional per-tier degradation references. Reject duplicate IDs, missing references, and presets that fail tier validation. Preserve backward compatibility for show files without visualizers.

Include a minimal TOML fixture inline in tests containing one `Orbit Meter`-style radial preset and one grid binding.

## Verification

Run `cargo test -p vj-types show`. Observable: TOML round trip preserves the preset, old manifests still parse, and a missing preset reference returns a descriptive error.

