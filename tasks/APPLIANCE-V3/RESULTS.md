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

## A3-02 — NEEDS-REVIEW
files: scripts/appliance/launch-renderer.sh, scripts/appliance/rpi-vj-renderer.service, docs/appliance/RENDERER.md
observable: launch-renderer.sh --test & --dry-run (5/5 tests passed on Pi 5)
actual: Single-output KMS forced to HDMI-A-1:1920x1080@60D; Cage/Chromium cgroup containment & profile sanitization verified.
hardware: Tested on physical Pi 5; pending 25-boot offline hardware gate (A3-03).
surprise: none

## A3-04 — NEEDS-REVIEW
files: scripts/appliance/boot-ledger.py, scripts/appliance/record-boot-event.sh, docs/appliance/OBSERVABILITY.md
observable: boot-ledger.py test (6 unit tests passed); marker A recorded on live Pi 5
actual: Structured 9-field JSONL schema and monotonic watchdog validated; journald bounded to 200M/15%.
hardware: Tested on physical Pi 5 (kernel 6.18.34+rpt-rpi-2712); pending power-cut reboot verification.
surprise: none





