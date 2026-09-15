# Appliance V3 Storage, Power-Loss, and Security Hardening

This document defines the storage resilience, power-loss survival mechanisms, hardware watchdog integration, and systemd security hardening for the Raspberry Pi Video Jockey (RPi-VJ) Appliance V3, conforming to task **A3-13**, **ARCHITECTURE.md**, **STORAGE.md**, and **OBSERVABILITY.md**.

---

## 1. Systemd Service Hardening & Sandboxing Matrix

All appliance services run under the dedicated unprivileged system account `vj` with shell locked to `/usr/sbin/nologin`. Services are strictly sandboxed using systemd security directives to prevent unauthorized filesystem writes and privilege escalation.

### Service Sandboxing Directives

| Security Directive | Value | Purpose |
|---|---|---|
| `ProtectSystem` | `strict` | Mounts entire `/usr`, `/boot`, `/etc`, and root filesystem strictly read-only for the service process tree. |
| `ProtectHome` | `yes` | Denies all access to `/home`, `/root`, and `/run/user`. |
| `NoNewPrivileges` | `yes` | Prohibits child processes from gaining additional privileges via `setuid` / `setgid` binaries. |
| `PrivateTmp` | `yes` | Provides an isolated `/tmp` and `/var/tmp` namespace, preventing cross-service temp file tampering. |
| `ReadWritePaths` | `/srv/rpi-vj /var/lib/rpi-vj /run/rpi-vj` | Whitelists only explicit application data, state, and socket directories for write operations. |

### Unit Coverage

The hardening profile is enforced on all core appliance units:
- `rpi-vj-broker.service`: Broker, state store, and supervisor API.
- `rpi-vj-renderer.service`: DRM session, Cage Wayland compositor, and Chromium kiosk renderer.
- `rpi-vj-controller.service`: Stream Deck XL native USB HID controller.
- `rpi-vj-supervisor.service`: Fault recovery policy and boot ledger manager.
- `rpi-vj-ingest.service`: Removable media ingest worker.
- `rpi-vj-storage-gc.service`: Periodic storage garbage collector.

---

## 2. Hardware Watchdog Integration

To ensure the appliance automatically recovers from hard kernel lockups or unhandled panics without human intervention, Linux hardware watchdog support is integrated directly with systemd.

### Configuration

- **Kernel Module**: `bcm2835_wdt` loaded on boot via `/etc/modules-load.d/watchdog.conf`.
- **Systemd Configuration**: `/etc/systemd/system.conf.d/00-watchdog.conf`:

```ini
# /etc/systemd/system.conf.d/00-watchdog.conf
# Managed for RPi-VJ Appliance V3 Hardware Watchdog Hardening (A3-13)
[Manager]
RuntimeWatchdogSec=15
RebootWatchdogSec=30
ShutdownWatchdogSec=10min
WatchdogDevice=/dev/watchdog
```

### Operation & Monotonic Clock Invariant
1. Systemd pings `/dev/watchdog` every `RuntimeWatchdogSec / 2` (7.5 seconds).
2. If systemd freezes or the kernel stops scheduling tasks for more than 15 seconds, the Broadcom hardware watchdog timer expires and resets the SoC.
3. **Monotonic Immunity**: Service level recovery timeouts (such as renderer frame heartbeats) strictly use `time.monotonic()`. Stepping the wall clock (via NTP sync or operator change) will **never** trigger a false watchdog reset.

---

## 3. Power-Loss Resilience & Storage Architecture

The appliance filesystem layout is engineered to withstand sudden, unannounced power cuts at any operational stage.

```text
/srv/rpi-vj/
  ├── current -> shows/gen-20260915120000-abcd1234/  (atomic symlink)
  ├── shows/
  │   ├── gen-20260915120000-abcd1234/               (active generation)
  │   └── gen-20260915100000-efgh5678/               (previous generation for rollback)
  └── staging/
      ├── job-<uuid>/                                (work in progress)
      └── quarantine-<timestamp>-<uuid>/             (quarantined partial outputs on boot)

/var/lib/rpi-vj/
  ├── boot-ledger.jsonl                              (durable, fsynced boot & shutdown ledger)
  └── ingest-jobs.jsonl                              (durable, fsynced append-only job ledger)
```

### Storage Invariants
1. **15% Free Space Reserve**: Storage engines reject new ingest jobs before starting if `free_space - required < 0.15 * total_space`.
2. **Immutable Show Generations**: Once installed, show generations in `/srv/rpi-vj/shows/gen-*` are immutable.
3. **Atomic Activation**: Show activation updates `/srv/rpi-vj/current` by creating a temporary symlink `/srv/rpi-vj/.current_tmp_<uuid>` and executing an atomic `rename()`. At no point is `current` absent or pointing to corrupt partial data.
4. **Boot Recovery & Quarantine**: If power is cut mid-transcode or mid-copy, incomplete staging directories are quarantined into `/srv/rpi-vj/staging/quarantine-*` on boot. The active show generation remains untouched and immediately playable.
5. **Rollback Guarantee**: The storage manager preserves the active generation and the immediately previous generation. If a new show is defective, calling rollback atomically reverts `/srv/rpi-vj/current` to the prior generation.

---

## 4. Automated Storage Garbage Collection

To prevent disk saturation from abandoned jobs or unmanaged generations, the storage garbage collector runs periodically and on boot.

### Garbage Collection Policies

1. **Orphaned Staging Cleanup (>24h)**:
   - Scans `/srv/rpi-vj/staging/`.
   - Any staging job (`job-*`) or quarantine folder (`quarantine-*`) older than 24 hours (86,400 seconds) is automatically removed.
   - Terminal jobs (marked `Completed`, `Failed`, or `Cancelled` in `/var/lib/rpi-vj/ingest-jobs.jsonl`) older than 5 minutes are safely cleaned.

2. **Generation Retention Policy (Keep 2)**:
   - Scans `/srv/rpi-vj/shows/`.
   - Preserves the active generation (pointed to by `/srv/rpi-vj/current`).
   - Preserves the previous valid generation (for instant operator rollback).
   - Prunes older immutable generations.

### Systemd Service & Timer

- Service: `/etc/systemd/system/rpi-vj-storage-gc.service`
- Timer: `/etc/systemd/system/rpi-vj-storage-gc.timer` (executes every hour and 10 minutes after boot).
- Script: `/opt/rpi-vj/scripts/appliance/storage-gc.py`.

Manual execution:
```bash
python3 /opt/rpi-vj/scripts/appliance/storage-gc.py --max-staging-age-hours 24 --keep-generations 2
```

---

## 5. Boot Cause Logger: Clean Shutdown vs Power Cut

Every reboot or power removal is recorded and classified in `/var/lib/rpi-vj/boot-ledger.jsonl`.

### Clean Shutdown Mechanism
- Unit `/etc/systemd/system/rpi-vj-shutdown-logger.service` runs `record-boot-event.sh clean-shutdown` on orderly system shutdown (`shutdown.target`, `reboot.target`).
- Records a durable, `fsync`'d `clean_shutdown_request` entry in `boot-ledger.jsonl`.

### Sudden Power-Cut Detection
- On boot, `rpi-vj-supervisor.service` runs `boot-ledger.py check-unclean`.
- It inspects the final event recorded by the preceding boot ID.
- If the last event is **NOT** `clean_shutdown_request`, the supervisor logs `unclean_filesystem_indication` with details (`prior_boot_id`, `last_event`, `last_monotonic_uptime`, `reason`).

### Querying Boot Cause
```bash
python3 /opt/rpi-vj/scripts/appliance/boot-ledger.py check-unclean
# Output: CLEAN_BOOT (exit 0) or UNCLEAN_REBOOT_DETECTED (exit 1)
```

---

## 6. Appliance Hardening Utility (`harden-appliance.sh`)

The script `/opt/rpi-vj/scripts/appliance/harden-appliance.sh` validates, simulates, and applies all hardening configurations idempotently.

### Usage

```bash
# Check hardening status without changes (returns 0 if compliant, 1 if drift)
./scripts/appliance/harden-appliance.sh --check

# Dry-run inspection
./scripts/appliance/harden-appliance.sh --dry-run

# Apply all hardening configurations
sudo ./scripts/appliance/harden-appliance.sh --apply

# Run automated test suite
./scripts/appliance/harden-appliance.sh --test
```

---

## 7. Verification & Observable Validation

To verify task **A3-13**:

1. **Systemd Sandboxing**:
   ```bash
   systemctl show rpi-vj-broker.service -p ProtectSystem -p ProtectHome -p NoNewPrivileges -p ReadWritePaths
   ```
2. **Watchdog Verification**:
   ```bash
   grep -E 'RuntimeWatchdogSec' /etc/systemd/system.conf.d/00-watchdog.conf
   ```
3. **Power-Loss & Boot Cause Observable**:
   ```bash
   # In boot A: write marker
   /opt/rpi-vj/scripts/appliance/record-boot-event.sh marker A
   # Abruptly cut power (or simulate power loss without clean shutdown event)
   # On reboot (boot B):
   /opt/rpi-vj/scripts/appliance/record-boot-event.sh check-unclean
   /opt/rpi-vj/scripts/appliance/record-boot-event.sh marker B
   python3 /opt/rpi-vj/scripts/appliance/boot-ledger.py read
   ```
   Both markers and the `unclean_filesystem_indication` are present in persistent logs, and the active show remains playable.
