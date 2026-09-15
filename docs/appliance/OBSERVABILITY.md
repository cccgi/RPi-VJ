# Appliance V3 Persistent Observability & Boot Ledger Specification

This document defines the persistent logging and boot ledger architecture for the
Raspberry Pi Video Jockey (RPi-VJ) Appliance V3, conforming to task **A3-04** and the
Appliance V3 Architecture Contract.

---

## 1. Architectural Contract & Objectives

After any arbitrary power removal, crash, or unexpected reboot, every failed boot
must be diagnosable using durable persistent storage.

### Core Constraints
1. **Bounded Storage**: System journal usage is strictly capped at **<=200 MiB** and
   configured to always retain **>=15% free disk space** (`/var/log/journal/`).
2. **Durable Ledger**: Structured event ledger stored as append-only JSONL at
   `/var/lib/rpi-vj/boot-ledger.jsonl`.
3. **Flushing Policy**: Records are flushed and `fsync`'d at discrete critical lifecycle
   boundaries to survive sudden power removal. Per-frame render loops (60 FPS) never
   fsync to storage.
4. **Monotonic Recovery Invariant**: All heartbeat, timeout, and recovery decisions
   depend solely on monotonic uptime. No recovery decision may use realtime. Artificially
   stepping wall time forward (e.g. by 1 hour) or backward never triggers a watchdog or
   recovery action.

---

## 2. Bounded Persistent Journald Configuration

Systemd's persistent journal retains service logs across boots, allowing operators to
inspect the prior boot's renderer journal.

Drop-in configuration path: `/etc/systemd/journald.conf.d/10-rpi-vj.conf`

```ini
# /etc/systemd/journald.conf.d/10-rpi-vj.conf
# Managed for RPi-VJ Appliance V3 Persistent Observability (A3-04)
[Journal]
Storage=persistent
SystemMaxUse=200M
SystemKeepFree=15%
RuntimeMaxUse=50M
MaxRetentionSec=1month
ForwardToSyslog=no
```

### Installation
```bash
python3 /opt/rpi-vj/scripts/appliance/boot-ledger.py journald-conf --install
# Or via shell wrapper:
/opt/rpi-vj/scripts/appliance/record-boot-event.sh setup-journald
systemctl restart systemd-journald
```

### Querying Prior Boot Logs
To view the renderer journal from the immediately preceding boot:
```bash
journalctl -b -1 -u rpi-vj-renderer.service
```

---

## 3. Boot Ledger Specification

The boot ledger file is located at `/var/lib/rpi-vj/boot-ledger.jsonl`. Each record is
a single-line JSON object adhering to the schema below.

### Record Schema (9 Mandatory Fields)

| Field | Type | Description |
|---|---|---|
| `boot_id` | `string` | Linux kernel boot ID (`/proc/sys/kernel/random/boot_id`) or transient UUID |
| `monotonic_uptime` | `float` | Monotonic uptime in seconds with millisecond precision |
| `realtime` | `string` | ISO 8601 UTC timestamp (`YYYY-MM-DDTHH:MM:SS.ffffffZ`) |
| `clock_synchronized` | `boolean` | `true` if NTP / systemd-timesyncd is synchronized, `false` otherwise |
| `event` | `string` | Unique event name (see Required Events catalog) |
| `service` | `string \| null` | Systemd service name or subsystem |
| `exit_status` | `integer \| null` | Exit code (e.g., `0`, `1`, `137`) or `null` if not applicable |
| `drm_connector` | `string \| null` | Detected DRM connector (e.g. `HDMI-A-1`) or `null` |
| `detail` | `any \| null` | Structured object or string providing contextual diagnostic data |

### Example JSONL Record
```json
{"boot_id":"3a4f891b-74b8-47be-9861-59fa3e7a0e19","monotonic_uptime":4.218,"realtime":"2026-09-15T07:22:45.123456Z","clock_synchronized":false,"event":"renderer_ready","service":"rpi-vj-renderer","exit_status":null,"drm_connector":"HDMI-A-1","detail":{"resolution":"1920x1080@60"}}
```

---

## 4. Minimum Lifecycle Events Catalog

The ledger records at minimum the following 14 discrete events:

1. `kernel_start`: Earliest userspace/initramfs hook execution.
2. `userspace_start`: Systemd initialization complete.
3. `renderer_start`: Cage / Chromium launch initiated (`rpi-vj-renderer.service`).
4. `renderer_ready`: DRM mode set, Cage active, Chromium rendering window ready.
5. `renderer_exit`: Cage/Chromium termination (includes exit code).
6. `first_frame`: Initial video frame successfully displayed on DRM output.
7. `streamdeck_discover`: Stream Deck USB HID device enumerated.
8. `streamdeck_open`: Stream Deck device opened by `rpi-vj-controller.service`.
9. `streamdeck_disconnect`: Stream Deck detached or USB bus reset.
10. `ingest_start`: Removable media ingest job queued.
11. `ingest_end`: Ingest job completed (includes validation exit status).
12. `supervisor_action`: Supervisor recovery intervention, restart, or state transition.
13. `clean_shutdown_request`: Graceful shutdown/reboot requested by operator.
14. `unclean_filesystem_indication`: Detected on boot when prior boot lacked `clean_shutdown_request`.

---

## 5. Unclean Reboot Detection

On boot, `rpi-vj-supervisor.service` runs `boot-ledger.py check-unclean`:
1. Scans `/var/lib/rpi-vj/boot-ledger.jsonl` backwards to find the preceding boot's events.
2. If the last event of the preceding boot was NOT `clean_shutdown_request`, the prior boot
   terminated abruptly (power loss or kernel panic).
3. The supervisor immediately logs an `unclean_filesystem_indication` event for the current
   boot ID with details indicating the prior boot ID, last event, and monotonic timestamp.

---

## 6. Monotonic Time Invariant & Wall-Clock Immunity

- **Watchdog / Health Checks**: Use `evaluate_recovery_watchdog(last_hb_monotonic, current_monotonic, timeout)`.
- If an NTP synchronization or an operator artificially advances wall time by 1 hour
  (`date -s "+1 hour"`):
  - Monotonic uptime is completely unaffected.
  - The watchdog elapsed time remains strictly monotonic (e.g. 2.0 seconds).
  - No spurious recovery action, reboot, or service restart occurs.

---

## 7. CLI Utilities & Helpers

### Python Engine: `scripts/appliance/boot-ledger.py`
- `record`: Records event with JSON formatting, auto-detected DRM connector, and fsync.
- `read`: Dumps or filters events by boot ID or limit.
- `check-unclean`: Evaluates preceding boot state and writes unclean indication if needed.
- `journald-conf`: Generates or installs `/etc/systemd/journald.conf.d/10-rpi-vj.conf`.
- `evaluate-watchdog`: Evaluates watchdog threshold using strictly monotonic input.
- `test`: Executes self-contained unit test suite.

### Shell Wrapper: `scripts/appliance/record-boot-event.sh`
Provides straightforward CLI and systemd integration:
```bash
record-boot-event.sh marker A
record-boot-event.sh renderer-ready
record-boot-event.sh clean-shutdown
record-boot-event.sh check-unclean
record-boot-event.sh read
```

---

## 8. Verification & Observable Procedure

To verify the observable required by task **A3-04**:

```bash
# 1. Generate marker A in current boot
/opt/rpi-vj/scripts/appliance/record-boot-event.sh marker A

# 2. Simulate or execute power cut (unclean shutdown without clean-shutdown event)
# (On test harness: kill system or reboot abruptly without recording clean_shutdown_request)

# 3. Boot system and generate marker B
/opt/rpi-vj/scripts/appliance/record-boot-event.sh check-unclean
/opt/rpi-vj/scripts/appliance/record-boot-event.sh marker B

# 4. Verify both markers and unclean indication are readable
python3 /opt/rpi-vj/scripts/appliance/boot-ledger.py read

# 5. Read prior boot's renderer journal
journalctl -b -1 -u rpi-vj-renderer.service

# 6. Artificially advance wall time by 1 hour
date -s "+1 hour"

# 7. Evaluate watchdog
python3 /opt/rpi-vj/scripts/appliance/boot-ledger.py evaluate-watchdog --last-monotonic 10.0 --timeout 30.0
# Result: Watchdog healthy (monotonic clock unaffected by wall-time change)
```

---

## 9. Hardware Gate Status

- **Local Verification (Passed)**:
  - Unit test suite: 6/6 tests passing (schema compliance, unclean detection, marker flow, journal bounds, and 1-hour wall-time monotonic immunity).
  - Shell wrapper execution verified.
- **Remaining Hardware Gate**:
  - Verification on physical Raspberry Pi 4/5 hardware with actual power-cut switch, microSD write flush endurance, and HDMI-A-1 DRM output.
