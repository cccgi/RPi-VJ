# G-16 — Transcode the Clip Library
**Model:** cheap · **Blocked by:** G-02 (the codec DECISION line) · **Run on the 3090, not the Pi**

## Deliverable
`gig/tools/transcode.sh` + the finished library in `gig/media/`

## Requirements
- Read the format from G-02's `DECISION` line in `../RESULTS.md`. **Do not pick
  a codec yourself** — that decision was made against measurements.
- `transcode.sh <input-dir> <output-dir>`, recursive, skipping files already
  present in the output with a newer mtime (re-running must be cheap).
- Always: `-g 15 -keyint_min 15 -sc_threshold 0` for clean closed-GOP loops,
  `-an` to strip audio, `-movflags +faststart`, `-pix_fmt yuv420p`.
- Normalise to a common resolution and frame rate. Mixed frame rates in one set
  cause visible judder at cut points.
- **Trim to loop cleanly.** A clip whose first and last frame don't match will
  visibly hitch every loop. Where a clip doesn't loop, either trim it or note it
  as one-shot in `show.json`.
- Emit a `manifest.json` listing each output with duration, resolution and size.
- Print total output size at the end — the Pi's SD card is finite and a 40 GB
  library discovered on Friday night is a bad Friday night.

## Observable
The library transcoded, `manifest.json` written, and **at least three clips
played on the Pi** through G-06 with zero dropped frames over 60 seconds.
Record clip count, total size, and dropped-frame counts in `../RESULTS.md`.
