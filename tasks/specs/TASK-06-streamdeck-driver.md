# Task Specification: TASK-06 — Stream Deck XL Real HID Driver & Icon Cache

## 1. Goal
Implement the concrete HID communication loop and JPEG icon cache for the Elgato Stream Deck XL in `vj-surface-hid`, supporting 32 physical keys (4x8), clean press-edge transition diffing, and fast asynchronous icon frame delivery.

## 2. Target Files
*   **Modify**: `vj/crates/vj-surface-hid/Cargo.toml`
*   **Modify**: `vj/crates/vj-surface-hid/src/lib.rs`
*   **New**: `vj/crates/vj-surface-hid/src/device.rs`

## 3. Requirements & Constraints
1. **Device Identification**:
   - Vendor ID: `0x0fd9`
   - Product IDs: `0x006c` (XL v1), `0x008f` (XL v2)
2. **Press Edge Detection**:
   - Maintain a 32-bool previous state buffer.
   - Emit `ControlEvent::Press` only when key goes `0 -> 1`.
   - Emit `ControlEvent::Release` only when key goes `1 -> 0`.
   - A key held across multiple poll ticks must NEVER fire duplicate `Press` events.
3. **Icon Cache & Page Report Framing**:
   - Cache 96x96 JPEG byte slices in a local array: `[Option<Vec<u8>>; 32]`.
   - On bank change or reconnect: emit page image reports only for keys that changed hash.
4. **Prohibitions**:
   - Do NOT perform JPEG encoding on the HID polling thread.
   - Do NOT mutate engine state directly; emit `ControlEvent` into the output vector.

## 4. Acceptance Criteria & Unit Tests
*   `cargo test -p vj-surface-hid` passes with:
    1. `test_held_key_fires_only_once`: Polling 100 times with a held key produces exactly 1 press.
    2. `test_simultaneous_presses`: Multiple simultaneous button presses produce distinct events.
    3. `test_icon_cache_diffing`: Changing 1 icon out of 32 only marks 1 icon dirty for transmission.
