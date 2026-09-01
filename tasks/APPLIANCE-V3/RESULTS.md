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

## A3-00 — NEEDS-REVIEW
files: scripts/appliance/capture-baseline.sh, docs/appliance/RECOVERY.md
observable: bash -n capture-baseline.sh, test_a3_00.py (syntax, manifest, secret redaction, key suppression)
actual: All local static & execution tests passed; directory tree and SHA-256 manifest generated.
hardware: Simulation/dev test; pending physical Pi baseline capture and spare-card boot.
surprise: none

