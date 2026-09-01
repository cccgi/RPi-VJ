# Task Specification: TASK-08 — Sound Blaster Play! 3 PipeWire Stream Capture

## 1. Goal
Implement the real-time audio capture stream for the Creative Sound Blaster Play! 3 (USB Audio) in `vj-audio` using PipeWire and CPAL backends, feeding preallocated lock-free ring buffers.

## 2. Target Files
*   **Modify**: `vj/crates/vj-audio/Cargo.toml`
*   **New**: `vj/crates/vj-audio/src/capture.rs`
*   **Modify**: `vj/crates/vj-audio/src/lib.rs`

## 3. Requirements & Constraints
1. **Audio Stream Contract**:
   - Sample Rate: 48,000 Hz.
   - Channels: 2 (Stereo f32).
   - Quantum / Buffer Size: 256 samples (5.33 ms period).
2. **Real-Time Callback Discipline**:
   - Capture callback MUST NOT allocate memory (`Vec::new()`, `Box::new()`, etc.).
   - Capture callback MUST NOT acquire mutexes, write to log files, or perform I/O.
   - Pass samples into an SPSC ring buffer (or invoke direct callback into `AudioAnalyzer`).
3. **Gain Control**:
   - Apply configurable input gain multiplier (default: 0.25 to prevent line-level clipping from the Pioneer FLX-10 Booth Out).

## 4. Acceptance Criteria & Unit Tests
*   `cargo test -p vj-audio` passes with:
    1. `test_capture_trait_ring_handoff`: Verify samples written from simulated real-time sink arrive in order without buffer overrun.
    2. `test_audio_gain_scaling`: Verify hot input signals are scaled appropriately before analysis.
