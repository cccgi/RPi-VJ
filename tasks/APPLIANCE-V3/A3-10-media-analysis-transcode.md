# A3-10 — Media Analysis and Appliance Transcode

## Goal

Produce predictable Pi-playable loops and advisory quad/seam analysis from
arbitrary USB/LAN media.

## Scope

Extend `vj/tools/vj-ingest` rather than creating another transcoder. Emit a
machine-readable report containing codec, profile, pixel format, dimensions,
rate, duration, audio streams, decode errors, sampled perceptual hashes, quadrant
similarity scores, first/last-frame seam score, and proposed action.

The production rendition is 1920x1080 or policy-approved quadrant crop,
progressive 8-bit 4:2:0, with codec/GOP proven on the production Pi. Generate a
96x96 Stream Deck JPEG. Seam repair may use a configurable crossfade, but must
preserve the original and require policy/operator approval above a conservative
threshold. Limit CPU/I/O priority and pause jobs in PERFORMANCE mode.

## Observable

Use a corpus containing normal, true quad-mirror, similar-but-not-identical
quadrants, seamless loop, bad seam, variable-rate, corrupt, and unsupported
files. Reports classify the corpus as expected; every accepted rendition passes
full decode on the Pi and loops for ten minutes without a decoder error.

