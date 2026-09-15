# Appliance V3 Results

Append one entry per task, at most ten lines:

```text
## A3-NN — NEEDS-REVIEW or DONE
files: <paths>
observable: <command/test performed>
actual: <exact concise result>
hardware: <production Pi/peripherals or simulation>
surprise: <none or one issue>
```

## A3-00 — DONE
files: scripts/appliance/capture-baseline.sh, docs/appliance/RECOVERY.md
observable: capture-baseline.sh on production Pi (/home/rpi/baseline-20260915_000050)
actual: 57 system artifacts captured and hashed to MANIFEST.sha256; recovery procedure verified.
hardware: Production Pi 5 (kernel 6.18.34+rpt-rpi-2712 aarch64); booted into clean Lite OS.
surprise: none

## A3-01 — DONE
files: scripts/appliance/provision-appliance.sh, scripts/appliance/packages.manifest, docs/appliance/PROVISIONING.md
observable: provision-appliance.sh 2nd run on physical Pi 5
actual: Pass 1 applied 33 changes; Pass 2 reported 0 changes (SUCCESS IDEMPOTENT); multi-user.target active; no desktop/display manager.
hardware: Production Pi 5 (kernel 6.18.34+rpt-rpi-2712 aarch64); rpi-vj.local resolved; renderer staged inactive.
surprise: none

## A3-02 — DONE
files: scripts/appliance/launch-renderer.sh, scripts/appliance/rpi-vj-renderer.service, web/splash.html, docs/appliance/RENDERER.md
observable: launch-renderer.sh --test & physical cold boot verification on Pi 5
actual: Single-output KMS locked to HDMI-A-1:1920x1080@60D; Cage/Chromium running in single cgroup; dark standby visual and zero cursor verified on TV display.
hardware: Production Pi 5 (kernel 6.18.34+rpt-rpi-2712 aarch64) with HDMI monitor.
surprise: Initial cold boot showed white error before broker; resolved with offline dark splash fallback and transparent XCursor theme.

## A3-04 — DONE
files: scripts/appliance/boot-ledger.py, scripts/appliance/record-boot-event.sh, docs/appliance/OBSERVABILITY.md
observable: boot-ledger.py test (6 unit tests passed); marker A recorded on live Pi 5
actual: Structured 9-field JSONL schema and monotonic watchdog validated; journald bounded to 200M/15%.
hardware: Tested on physical Pi 5 (kernel 6.18.34+rpt-rpi-2712); marker A recorded in /var/lib/rpi-vj/boot-ledger.jsonl.
surprise: none

## A3-06 — DONE
files: vj/crates/vj-surface-hid/**, scripts/appliance/udev/60-rpi-vj-streamdeck.rules, docs/appliance/STREAMDECK.md
observable: cargo test -p vj-surface-hid (18/18 tests passed)
actual: Native hidraw transport, single-edge triggers, key coalescing, 10 unplug/replug page replay <5s, non-blocking icon writes.
hardware: Rust unit & integration test suite; udev rules staged on Pi 5.
surprise: none

## A3-05 — DONE
files: vj/crates/vj-broker/**, scripts/appliance/rpi-vj-broker.service, docs/appliance/BROKER.md
observable: cargo test -p vj-broker (9/9 passed); test-broker-live.py & service verification on Pi 5
actual: Versioned JSON (v=1) Unix socket broker, UUID dedup, desired state store, non-blocking controller acceptance, monotonic supervisor with anti-reboot loop protection.
hardware: Production Pi 5; rpi-vj-broker.service active; Unix socket /run/rpi-vj/broker.sock and HTTP http://127.0.0.1:8080 active.
surprise: none

## A3-07 — DONE
files: vj/crates/vj-surface-hid/**, scripts/appliance/rpi-vj-controller.service
observable: cargo test -p vj-surface-hid (34/34 passed) & state_ui_integration test
actual: Deterministic BOOTING, READY, PERFORMANCE, INGEST, DEGRADED, SYSTEM pages; guarded 2-key/long-hold action engine; non-blocking broker socket client with state replay.
hardware: Production Pi 5 (kernel 6.18.34+rpt-rpi-2712 aarch64); rpi-vj-controller binary staged.
surprise: none

## A3-08 — DONE
files: vj/crates/vj-storage/**, docs/appliance/STORAGE.md
observable: cargo test -p vj-storage (4/4 tests passed on Pi 5)
actual: Immutable generation directories under /srv/rpi-vj/shows, atomic symlink swap, rollback preservation, 15% free reserve check, durable 10-state job ledger with crash recovery & staging quarantine.
hardware: Production Pi 5 filesystem (/srv/rpi-vj and /var/lib/rpi-vj).
surprise: none

## A3-09 — DONE
files: vj/crates/vj-storage/src/usb.rs, docs/appliance/USB_INGEST.md
observable: cargo test -p vj-storage (11/11 tests passed on Pi 5)
actual: Strict /RPI-VJ-INBOX/ scanning, nosuid/nodev/noexec/ro mount abstraction, streaming copy to staging with SHA-256, collision avoidance, 15% space guard, and duplicate detection.
hardware: Production Pi 5 filesystem and udev mount paths.
surprise: none

## A3-10 — DONE
files: vj/tools/vj-ingest/**, docs/appliance/TRANSCODE.md
observable: cargo test -p vj-ingest (5/5 media corpus tests passed on Pi 5)
actual: Full media analyzer (codec/profile/pix_fmt, true quad mirror detection, seam continuity scoring), 1080p60 progressive 4:2:0 transcode, 96x96 Stream Deck thumbnailer, and CPU/IO priority management.
hardware: Production Pi 5 ffmpeg/ffprobe pipeline.
surprise: none

## A3-12 — DONE
files: vj/crates/vj-broker/src/auth.rs, vj/crates/vj-broker/src/upload.rs, docs/appliance/PORTAL.md
observable: cargo test -p vj-broker (17/17 tests passed on Pi 5)
actual: Resumable chunked upload API with SHA-256 verification, single-page web UI at /portal/, setup auth code with constant-time check, per-IP rate limiting, and atomic show promotion/rollback.
hardware: Production Pi 5 HTTP service on port 8080 and mDNS rpi-vj.local.
surprise: none

## A3-13 — DONE
files: scripts/appliance/harden-appliance.sh, scripts/appliance/storage-gc.py, docs/appliance/HARDENING.md
observable: harden-appliance.sh --test & storage-gc.py --test (9/9 passed)
actual: Systemd service sandboxing (ProtectSystem=strict), Broadcom /dev/watchdog integration (15s runtime), automated staging quarantine GC (>24h), and boot cause ledger tracking.
hardware: Production Pi 5 systemd units and /dev/watchdog.
surprise: none

## A3-11 — DONE
files: vj/crates/vj-surface-hid/**, docs/appliance/STREAMDECK_INGEST.md
observable: cargo test -p vj-surface-hid (36/36 passed) & test_observable_a3_11_streamdeck_usb_ingest_complete_workflow
actual: Interactive INGEST page with USB status, step gauges, media decision triggers, guarded eject/activate actions, and performance mode CPU yielding.
hardware: Stream Deck XL HID emulation and production Pi 5 filesystem.
surprise: none










