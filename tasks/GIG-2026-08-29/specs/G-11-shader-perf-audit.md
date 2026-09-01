# G-11 — Shader Perf Audit on the Pi (all 14)
**Model:** cheap · **Blocked by:** G-03, G-04 · **Blocks:** G-14

## Deliverable
`gig/probe/PERF.md` — a table, produced by measurement, not estimation.

## Method
Build `gig/probe/shader-bench.html`: loads every `.fs` in `gig/shaders/`,
compiles via G-04's parser + G-03's host, and for each one renders 5 seconds at
each of `1920x1080`, `1280x720`, `960x540` with plausible audio uniforms
(drive them with `sin()` at a few Hz so branches that depend on audio actually
execute — a shader benched with all-zero uniforms can be 3× faster than in use).

Report per shader: median fps at each size, and a tag:
- `SAFE` — ≥58 fps at 1080p
- `SCALED` — ≥58 fps at 720p or below, note the size it needs
- `HEAVY` — under 58 fps even at 540p
- `BROKEN` — fails to compile (paste the log)

## Observable
`PERF.md` committed with all 14 rows filled from **the Pi**, plus a one-line
recommendation of which shaders make the Saturday set list.

## Note
Expect the five `vs_*` raymarchers to land in HEAVY or SCALED. That is the
expected result, not a failure — it is the number G-14 needs in order to design
to budget. Do not optimise anything in this task; measure and report.
