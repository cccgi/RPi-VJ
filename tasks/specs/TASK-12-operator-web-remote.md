# Task Specification: TASK-12 — Operator Live Web Remote & Telemetry

## 1. Goal
Implement a lightweight WebSocket/HTTP remote monitoring endpoint in `vj-api` allowing an operator to inspect live deck status, layer opacity, audio FFT meters, and system FPS telemetry from an iPad or phone browser without disturbing HDMI output.

## 2. Target Files
*   **Modify**: `vj/crates/vj-api/Cargo.toml`
*   **Modify**: `vj/crates/vj-api/src/lib.rs`
*   **New**: `vj/crates/vj-api/src/remote.rs`

## 3. Requirements & Constraints
1. **Telemetry Streaming**:
   - Broadcast engine state snapshot at 15–30 Hz over WebSocket.
   - Payload includes: Active Bank ID, Deck A/B playing clip IDs, Layer 1–8 opacities, Audio Volume/Bass/Beat trigger, System FPS.
2. **Remote Command Dispatch**:
   - Accept JSON `EngineCommand` requests (e.g. Trigger Clip, Set Opacity, Blackout, Tap Tempo).
3. **Zero Impact on Render Thread**:
   - Web server runs on a background worker thread.
   - Reads double-buffered state snapshots; never locks or blocks the render thread.

## 4. Acceptance Criteria & Unit Tests
*   `cargo test -p vj-api` passes with:
    1. `test_remote_telemetry_json_serialization`: Validates state JSON broadcast.
    2. `test_remote_command_ingestion`: Validates incoming WebSocket JSON commands translate to `EngineCommand`.
