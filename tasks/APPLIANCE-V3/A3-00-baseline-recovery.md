# A3-00 — Freeze Baseline and Recovery Image

## Goal

Preserve the current Pi and produce a verified rollback path before replacing
the desktop deployment.

## Scope

Create `scripts/appliance/capture-baseline.sh` and
`docs/appliance/RECOVERY.md`. Do not alter Pi configuration.

The capture script writes a timestamped directory containing OS/kernel/package
versions, boot configuration, unit definitions, enabled units, mounts, block
devices, udev rules, USB identities, DRM connectors, network configuration,
checksums of application/config files, and recent logs. Redact secrets.

The recovery document must cover imaging the microSD, verifying the image hash,
restoring to a spare card, and proving that restored media boots.

## Prohibited shortcuts

- Do not copy passwords, private keys, Wi-Fi PSKs, or Samba secrets.
- Do not call a filesystem copy a recovery image.
- Do not modify the production card during capture.

## Observable

On the Pi, run the script. Restore the image to a spare card and boot it. Record
the image SHA-256 and successful restored boot in `RESULTS.md`. Until the spare
card boots, status cannot exceed `NEEDS-REVIEW`.

