# A3-13 — Storage, Power-Loss, and Security Hardening

## Goal

Make expected operator mistakes and venue conditions survivable without hiding
diagnostic evidence.

## Scope

Define filesystem reserve, log limits, staging cleanup, generation retention,
and media checksums. Evaluate high-endurance microSD versus USB SSD using measured
decode and ingest workloads. Do not enable a read-only root until updates,
persistent journals, state, and ingest have explicit writable locations.

Retain the Pi hardware watchdog. Log boot cause where possible. Add controlled
shutdown confirmation and a Stream Deck `SAFE TO REMOVE POWER` state. Restrict
systemd units with least privilege, fixed device access, protected system paths,
and no arbitrary shell endpoint. Back up release/config/state separately from
replaceable media.

## Observable

Run filesystem-full simulation, ten forced power removals at controlled ingest
stages, invalid USB content, repeated failed authentication, renderer crash loop,
and controller disconnect storm. The prior show remains playable, the unit does
not reboot-loop, and each cause is present in persistent logs.

