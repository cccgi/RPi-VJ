# Task Specification: TASK-10 — ISF Fragment Shader Host & Compositor

## 1. Goal
Implement the ISF 2.0 fragment shader parser, GLSL uniform binder, and multi-layer linear alpha compositor in `vj-isf` and `vj-render`.

## 2. Target Files
*   **Modify**: `vj/crates/vj-isf/src/lib.rs`
*   **New**: `vj/crates/vj-isf/src/parser.rs`
*   **New**: `vj/crates/vj-render/src/compositor.rs`
*   **Modify**: `vj/crates/vj-render/src/lib.rs`

## 3. Requirements & Constraints
1. **ISF Header JSON Parsing**:
   - Extract JSON frontmatter block `/*{ ... }*/` from `.fs` shader files.
   - Validate inputs: `float`, `color`, `bool`, `point2D`, `event`.
   - Enforce pass budget (max 4 passes for Pi 5 / Appliance tier).
2. **Audio Uniform Binding Contract**:
   - Automatically inject standard uniform block into fragment shader:
     `uniform float u_time; uniform vec2 u_resolution; uniform float u_bass; uniform float u_mid; uniform float u_treble; uniform float u_volume; uniform float u_onset; uniform float u_beat_phase;`
3. **Compositing & Layer Blending**:
   - Execute layer stack back-to-front.
   - Implement `BlendMode::Alpha`, `BlendMode::Additive`, `BlendMode::Multiply`, `BlendMode::Screen`.
   - All blending performed in linear color space.

## 4. Acceptance Criteria & Unit Tests
*   `cargo test -p vj-isf` and `cargo test -p vj-render` pass with:
    1. `test_isf_parser_extracts_inputs`: Parse `neon_tunnel.fs` header successfully.
    2. `test_layer_blending_math`: Golden-value tests for Alpha and Additive blend calculations.
    3. `test_over_budget_shader_rejected`: Shader with 6 passes fails validation on Appliance tier.
