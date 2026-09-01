# A3-07 — Stream Deck State UI and Command Integration

## Goal

Make the Stream Deck the complete operator interface for boot, show control,
ingest, faults, and safe shutdown.

## Scope

Implement deterministic pages for `BOOTING`, `READY`, `PERFORMANCE`, `INGEST`,
and `DEGRADED`. Define a fixed system page containing health, IP/hostname, free
space, temperature/throttle state, ingest entry, performance arm/disarm,
renderer restart, and guarded shutdown.

Clip presses enqueue broker commands immediately. Never hold a global input lock
while rendering or uploading icons. Use long-hold or two-key confirmation for
shutdown, reboot, show activation, delete, and ingest cancellation.

On renderer restart, desired bank, blackout, and active clip state are replayed.
The controller remains interactive and shows recovery progress.

## Observable

With HDMI disconnected, complete boot-status inspection, switch banks, select a
clip, restart the renderer, confirm restored state, view the current IP, and
perform guarded clean shutdown using only the Stream Deck.

