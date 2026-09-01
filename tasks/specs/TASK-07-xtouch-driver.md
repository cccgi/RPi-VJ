# Task Specification: TASK-07 — Behringer X-Touch Mini MIDI Driver & Rings

## 1. Goal
Implement the MIDI decode and feedback encoder driver for the Behringer X-Touch Mini in `vj-surface-midi` and `vj-devices`, handling 8 relative rotary encoders, 8 push buttons, 8 layer faders/buttons, and driving the 13-segment LED rings.

## 2. Target Files
*   **Modify**: `vj/crates/vj-surface-midi/src/lib.rs`
*   **New**: `vj/crates/vj-devices/src/xtouch_mini.rs`
*   **Modify**: `vj/crates/vj-devices/src/lib.rs`

## 3. Requirements & Constraints
1. **Input Protocol (Standard MIDI Mode)**:
   - Encoders 1–8: CC 16–23 on Channel 0 (Relative Two's Complement: 1..63 up, 127..65 down).
   - Push Encoders: Notes 0–7 (Momentary Press/Release).
   - Layer A Buttons Row 1: Notes 8–15.
   - Layer A Buttons Row 2: Notes 16–23.
   - Master Fader: CC 9 on Channel 0 (0..127 Absolute).
2. **Output Feedback (LED Rings & Buttons)**:
   - Ring Mode CCs: Send CC 16–23 with values 0..11 to set segment levels.
   - Ring Display Mode (Single Dot, Pan, Fan, Spread, Trim) configured via CC.
   - Button LEDs: Send Note On/Off (0x90) with velocity 0 (off) or 127 (on).
3. **Prohibitions**:
   - Do NOT allocate heap memory during incoming MIDI message processing.
   - Do NOT block the real-time MIDI callback.

## 4. Acceptance Criteria & Unit Tests
*   `cargo test -p vj-devices` passes with:
    1. `test_xtouch_relative_encoder_delta`: Clockwise turn (CC value 1) emits delta +1; Counter-clockwise (CC value 127) emits delta -1.
    2. `test_xtouch_ring_feedback_encoding`: Float value 0.5 in Pan mode formats valid CC byte sequence.
    3. `test_xtouch_button_press_release`: Note on/off messages map cleanly to `ControlEvent::Press` and `ControlEvent::Release`.
