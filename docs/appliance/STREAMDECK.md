# Appliance V3 Native Stream Deck XL Transport Specification

This document details the native Linux `hidraw` driver and transport layer for the **Elgato Stream Deck XL** control surface on the **RPi-VJ Appliance V3** platform, implementing task **A3-06** and adhering to [`tasks/APPLIANCE-V3/ARCHITECTURE.md`](file:///c:/Users/Admin/Projects/RPi-VJ/tasks/APPLIANCE-V3/ARCHITECTURE.md).

---

## 1. Architectural Principles

1. **No Chromium or WebHID Dependency:** Stream Deck hardware is opened and operated exclusively by a native Rust service (`rpi-vj-controller.service`). WebHID, browser profiles, user pairing prompts, and Chromium input drivers are completely eliminated from the control path.
2. **Decoupled Input and Output Pipelines:** Input report polling and icon image writing run on independent concurrent threads over thread-safe transport handles. A stalled or sluggish USB icon transmission never blocks button presses.
3. **Exact Edge Triggers:** Every physical action produces exactly one logical Press event and one logical Release event. Held buttons generate zero duplicate repeat events across polling loops.
4. **Coalesced Bounded Output:** Icon updates for the same key index are automatically coalesced so that rapid page changes or UI state transitions do not buffer redundant JPEG transfers.
5. **Durable Disconnect & Automatic Page Replay:** Hot-unplugging and replugging the USB cable automatically detects reconnection, re-initializes device brightness, and replays the entire active 32-key cached icon page within 5 seconds.

---

## 2. Hardware Topology & Udev Rule

- **Vendor ID (VID):** `0x0fd9` (Elgato Systems GmbH)
- **Product ID (PID):** `0x006c` / `0x008f` (Stream Deck XL Revision 1 & 2)
- **Key Count:** 32 keys arranged in a 4-row x 8-column matrix (logical index 0 to 31).
- **Display Resolution:** 96x96 pixels per key in baseline JPEG format.
- **Report Size:** 512 bytes output report payload with 8-byte framing header (Report ID `0x02`, Command `0x07`).

### Udev Access Rule (`/etc/udev/rules.d/60-rpi-vj-streamdeck.rules`)
Narrowly matches Elgato Stream Deck XL hardware and grants permissions to the `vj` service group with a dedicated `/dev/streamdeck_xl` symlink:

```udev
SUBSYSTEM=="hidraw", KERNEL=="hidraw*", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="006c", GROUP="vj", MODE="0660", TAG+="uaccess", SYMLINK+="streamdeck_xl"
SUBSYSTEM=="hidraw", KERNEL=="hidraw*", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="008f", GROUP="vj", MODE="0660", TAG+="uaccess", SYMLINK+="streamdeck_xl"
```

---

## 3. Crate Architecture (`vj/crates/vj-surface-hid`)

The driver is structured into clean, modular layers:

- **`device.rs`:** Device constants, report framing, JPEG chunk generation, key index mapping (hardware <-> logical grid 0..31), and input report delta parsing.
- **`transport.rs`:** Abstract `Transport` trait with Linux `HidrawTransport` (`libc::poll` / non-blocking I/O) and `MockTransport` for deterministic hardware failure injection.
- **`queue.rs`:** Bounded input queue (`BoundedInputQueue`) and key-coalescing output queue (`CoalescingOutputQueue`).
- **`publisher.rs`:** Publishes real-time JSON state to `/run/rpi-vj/controller-state.json` and records lifecycle events to `/var/lib/rpi-vj/boot-ledger.jsonl`.
- **`controller.rs`:** `StreamDeckController` coordinating background input reader, background output worker, page cache management, and automatic reconnection loop.
- **`main.rs`:** Standalone binary (`rpi-vj-controller`) packaged for `rpi-vj-controller.service`.

---

## 4. Verification & Observable Validation

All 5 required observable criteria have been verified with automated integration tests in `tests/observable.rs`:

1. **Pure Native Execution:** Runs without Chromium or WebHID.
2. **Key Accuracy:** All 32 keys produce the correct logical index (0..31) exactly once on press and release.
3. **Icon Routing:** Transmitted icon JPEG payloads match their respective target key indices.
4. **Reconnection Recovery:** Ten consecutive unplug/replug cycles restore and replay the full 32-key page in under 5.0 seconds.
5. **Decoupled Responsiveness:** An 800 ms stalled icon write on the output channel does not delay button press reporting (< 80 ms latency).
