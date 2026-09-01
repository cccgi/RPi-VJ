# A3-12 — LAN Ingest Portal and Stable Discovery

## Goal

Make occasional PC/Mac transfer easy without fixed IPs, manually maintained SSH
commands, or direct writes into the active media tree.

## Scope

Provide a small authenticated web portal and resumable upload API at
`http://rpi-vj.local/` (or an explicitly documented local HTTPS alternative).
The portal shows appliance identity, free space, active show, queued jobs,
upload progress, validation outcome, and activation controls. Uploaded files
enter the same staging/job pipeline as USB files.

Generate a one-time setup credential/recovery code; do not reuse Linux or Samba
passwords. Bind to LAN only, rate-limit authentication, validate names and sizes,
and prohibit arbitrary paths/commands. Keep SSH key maintenance access. SMB may
be offered only as an inbox share, never the repo or active show directory.

The Stream Deck system page shows hostname and current IPv4 address so mDNS
failure is recoverable without a monitor or SSH.

## Observable

From Windows and macOS on networks assigning different IPs, discover by hostname,
upload a large file with interruption/resume, observe the job on Stream Deck,
activate it, and confirm direct traversal or active-show overwrite is rejected.

