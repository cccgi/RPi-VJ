# Task Specification: TASK-13 — Ableton Link Tempo & Phase Sync

## 1. Goal
Integrate live Ableton Link peer discovery and phase synchronization into `vj-tempo` for zero-configuration beat matching with Rekordbox 6/7 and Serato DJ Pro.

## 2. Target Files
*   **Modify**: `vj/crates/vj-tempo/Cargo.toml`
*   **Modify**: `vj/crates/vj-tempo/src/lib.rs`
*   **New**: `vj/crates/vj-tempo/src/link.rs`

## 3. Requirements & Constraints
1. **Link Session Management**:
   - Join local Link session on default network interface (UDP port 20808).
   - Track peer count (`num_peers`).
   - Query session tempo (BPM), beat phase ($0.0 .. 1.0$), and bar phase ($0.0 .. 1.0$).
2. **Authority Integration**:
   - Report Link state to `TempoArbiter` as `TempoSource::Link`.
   - When `num_peers > 0`, Link takes precedence over audio-derived tempo.
   - Smooth phase ramp on handover (no sudden visual jumps).

## 4. Acceptance Criteria & Unit Tests
*   `cargo test -p vj-tempo` passes with:
    1. `test_link_source_outranks_audio_when_peer_present`: Verify authority handover logic.
    2. `test_link_phase_calculation`: Validate grid beat phase conversion math.
