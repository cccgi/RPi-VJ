# A3-14 — Release Qualification and Cutover

## Goal

Prove the entire production appliance under the actual gig topology and create a
repeatable release/cutover process.

## Scope

Build a release manifest containing OS image hash, package lock, application
commit/hash, unit/config hashes, media generation, hardware identities, and all
gate results. Create a single installer/updater with preflight, atomic release
switch, health check, and rollback. Preserve the previous application release
and show generation.

Execute:

- A3-03's 25 cold boots;
- eight-hour mixed clip/shader soak with live audio;
- 500 randomized Stream Deck presses;
- controller, audio, HDMI, LAN, and USB disconnect/reconnect;
- Chromium and Cage kills;
- NTP jumps forward/backward;
- USB and LAN ingest followed by rollback;
- venue-style boot with no network, keyboard, mouse, or operator monitor;
- spare-card recovery drill.

## Observable

All tests pass on the production Pi, TV/cable, power supply, Stream Deck XL,
audio interface, and storage. Results and artifact hashes are recorded. Any
failure blocks release; no agent may waive a hardware gate.

