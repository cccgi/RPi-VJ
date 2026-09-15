# RPi-VJ Appliance V3 — Command Broker & Monotonic Supervisor

## Overview

The `vj-broker` is the central command and state hub for the RPi-VJ Appliance. It exposes a single versioned IPC contract over Unix-domain sockets and an embedded HTTP/API server, while supervising the appliance lifecycle using monotonic deadlines.

## Protocol Specification (Version 1)

All messages exchanged over the Unix socket (`/run/rpi-vj/broker.sock`) and WebSocket connections use JSON framing with newline delimiters (`NDJSON`).

### 1. Command Message (Controller / API -> Broker)

```json
{
  "v": 1,
  "id": "uuid-or-unique-string",
  "type": "command",
  "name": "clip.trigger",
  "args": { "key": 7, "bank": 1 }
}
```

Supported core commands:
- `clip.trigger`: `{ "key": <u32>, "bank": <u32 (optional)> }`
- `clip.stop`: `{}`
- `bank.select`: `{ "index": <u32> }`
- `blackout.toggle`: `{}`
- `blackout.set`: `{ "value": <bool> }`
- `master.opacity`: `{ "value": <f32 [0.0..1.0]> }`
- `tempo.bpm`: `{ "bpm": <f32 [20.0..300.0]> }`
- `show.load`: `{ "name": <string> }`
- `property.set`: `{ "key": <string>, "value": <any> }`

### 2. Acknowledgment Message (Broker -> Controller / API)

```json
{
  "v": 1,
  "id": "uuid-or-unique-string",
  "type": "ack",
  "status": "accepted",
  "detail": null
}
```

Status codes:
- `accepted`: Command validated, deduplicated, and applied to desired state. Controller unblocks immediately.
- `applied`: Command fully rendered/executed by downstream pipeline.
- `failed`: Command rejected due to invalid syntax or parameters.

### 3. State Sync & Subscription

```json
{
  "v": 1,
  "type": "state",
  "state": {
    "version": 42,
    "show_name": "default",
    "active_bank": 1,
    "active_clip": 7,
    "blackout": false,
    "master_opacity": 1.0,
    "tempo_bpm": 128.0,
    "properties": {}
  }
}
```

### 4. Heartbeat (Renderer -> Broker)

```json
{
  "v": 1,
  "type": "heartbeat",
  "source": "web_renderer",
  "timestamp_ms": 1700000000000,
  "frame_count": 1420
}
```

---

## Monotonic Supervisor & Escalation Ladder

The supervisor enforces an escalation ladder to recover from rendering failures while strictly preventing destructive reboot loops.

### Clock Guarantees
- All deadlines and timeout calculations use `std::time::Instant` (Linux `CLOCK_MONOTONIC`).
- Completely immune to NTP adjustments, daylight saving time shifts, or unconfigured RTC jumps at cold boot.

### Escalation Ladder
1. **Normal**: Heartbeats received within <= 3.0s. Status is healthy.
2. **Warning (`DegradedWarning`)**: Heartbeat missing for > 3.0s. Log warning.
3. **Escalation Level 1 (`RestartRenderer`)**: Heartbeat missing for > 6.0s or process termination. Supervisor restarts `rpi-vj-renderer.service` via systemctl.
4. **Escalation Level 2 (`ControlledReboot`)**: Renderer restarts fail >= 3 times within a sliding 60-second window. Trigger **exactly one** controlled system reboot, recording the incident in `/var/lib/rpi-vj/supervisor-state.json` and `/var/lib/rpi-vj/boot-ledger.jsonl`.
5. **Escalation Level 3 (`DurableDegraded`)**: If the renderer fails again after a recent reboot (within 180s), rebooting is permanently suppressed to prevent infinite reboot loops. The appliance enters durable degraded mode, exposes diagnostic status on HTTP port 8080, and notifies observability systems.
