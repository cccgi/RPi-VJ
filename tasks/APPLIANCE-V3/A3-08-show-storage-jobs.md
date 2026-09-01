# A3-08 — Immutable Show Storage and Ingest Job Model

## Goal

Prevent partial copies, failed transcodes, or power loss from corrupting the
active show.

## Scope

Implement generation directories under `/srv/rpi-vj/shows`, staging under
`/srv/rpi-vj/staging`, and an atomic `/srv/rpi-vj/current` symlink. Define a
small SQLite or append-only job store under `/var/lib/rpi-vj` with states:

`discovered -> copying -> probing -> needs_decision -> transcoding -> validating -> ready -> activating -> complete`, plus `failed` and `cancelled`.

Every transition is durable and idempotent. On boot, resume safe stages and
quarantine ambiguous output. Active generations are immutable. Keep the prior
generation for rollback. Enforce 15% free-space reserve before work begins.

## Observable

Cut power once during copy, transcode, validation, and activation. After each
reboot, the old show remains playable and the job either resumes or enters a
specific recoverable failure. Atomic activation never exposes mixed generations.

