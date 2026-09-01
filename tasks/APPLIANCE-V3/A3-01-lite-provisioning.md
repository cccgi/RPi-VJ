# A3-01 — Lite OS and Deterministic Provisioning

## Goal

Build a repeatable Raspberry Pi OS Lite 64-bit appliance with no desktop stack.

## Scope

Create one idempotent provisioning script under `scripts/appliance/` and one
package/config manifest beside it. The script must:

- install only Cage, Chromium, ffmpeg/ffprobe, required Rust/native libraries,
  Avahi, udisks/mount helpers, and diagnostics;
- create locked-down `vj` service ownership and required groups;
- disable LightDM, desktop autologin, panels, file managers, screen lockers,
  cloud-init after provisioning, and unnecessary wait-online behavior;
- leave SSH available for maintenance, using key authentication;
- configure hostname `rpi-vj` and mDNS;
- pin or record every package version used by the release;
- install units without starting the renderer until A3-02 exists.

## Inputs allowed

Read this spec and `tasks/APPLIANCE-V3/ARCHITECTURE.md`. Do not copy the legacy
`gig/deploy/install-desktop.sh` behavior.

## Observable

After two runs, the second reports no changes. `systemctl get-default` is
`multi-user.target`; LightDM/Labwc/Wayfire are absent or disabled; no graphical
desktop process exists; `rpi-vj.local` resolves when LAN is present.

