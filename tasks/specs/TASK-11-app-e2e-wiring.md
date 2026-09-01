# Task Specification: TASK-11 — Main App End-to-End Wiring & Loop Integration

## 1. Goal
Wire the `vj-app` main entry point to bind all subsystems (`vj-core`, `vj-surface-hid`, `vj-surface-midi`, `vj-audio`, `vj-video`, `vj-render`, `vj-tempo`) into a single running appliance loop.

## 2. Target Files
*   **Modify**: `vj/crates/vj-app/Cargo.toml`
*   **Modify**: `vj/crates/vj-app/src/main.rs`
*   **New**: `vj/crates/vj-app/src/runner.rs`

## 3. Requirements & Constraints
1. **CLI Arguments**:
   - `vj-app --show <path/to/show.toml> [--tier appliance|portable|workstation]`
2. **Subsystem Wiring**:
   - Load `ShowManifest` via `vj_types::ShowManifest::from_toml`.
   - Initialize `AudioAnalyzer` and `AudioCapture` (Sound Blaster Play! 3 or PipeWire).
   - Initialize `StreamDeck` HID adapter (`vj-surface-hid`) and `X-Touch Mini` MIDI adapter (`vj-surface-midi`).
   - Spawn `GStreamer` video decoders for active decks.
   - Run main event loop:
     - Poll HID & MIDI input events $\to$ push to `CoalescingQueue`.
     - Execute `Reducer::apply(cmd)`.
     - Update Audio Features snapshot once per frame.
     - Render composite frame (`vj-render`) at target FPS (60.0).
     - Emit device feedback updates at max `feedback_hz`.

## 4. Acceptance Criteria & Unit Tests
*   `cargo test -p vj-app` passes with:
    1. `test_runner_initialization_with_show_manifest`: Validates full startup and clean shutdown signal handling.
