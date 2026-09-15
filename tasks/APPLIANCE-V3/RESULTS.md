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



