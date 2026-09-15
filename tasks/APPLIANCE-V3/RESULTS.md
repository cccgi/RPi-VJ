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






