# A3-04 — Persistent Logs and Boot Ledger

## Goal

Make every future failed boot diagnosable after arbitrary power removal.

## Scope

Add bounded persistent journald configuration and a boot-ledger writer. Keep
journal usage <=200 MiB and retain sufficient free disk. Each JSONL entry under
`/var/lib/rpi-vj/boot-ledger.jsonl` includes boot ID, monotonic uptime, realtime,
clock-synchronized flag, event, service, exit status, DRM connector, and detail.

Record at minimum: kernel/userspace start, renderer start/ready/exit, first frame,
Stream Deck discover/open/disconnect, ingest start/end, supervisor action, clean
shutdown request, and next-boot unclean-filesystem indication.

No recovery decision may use realtime. Ensure records are flushed at critical
boundaries without syncing every render frame.

## Observable

Generate marker `A`, remove power, reboot, and generate marker `B`. Both markers
and the prior boot's renderer journal are readable. Artificially advance wall
time by one hour; no watchdog/recovery action occurs.

