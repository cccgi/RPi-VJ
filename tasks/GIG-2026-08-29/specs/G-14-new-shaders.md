# G-14 — Four Pi-Budget Shaders
**Model:** mid · **Blocked by:** G-11 (you need the real fps numbers first)

## Design constraint — MEASURED
`gig/probe/PERF.md` has real numbers from the target Pi. Read it first.
**Target: 4x headroom at 1080p** (≤4.2 ms/frame) so the post chain and a video
deck fit alongside. A shader that merely "hits 60 fps" alone is NOT good enough —
`audio_wave_rings` does exactly that and leaves zero room.

Note the shaders use a `u_*` uniform convention, not stock ISF:
`u_time, u_resolution, u_bass, u_mid, u_treble, u_onset, u_volume, u_beat_phase`,
output via `gl_FragColor`. Match it.

### Also in scope: rescue two existing shaders
`vs_sacred_polyhedron` (30 fps @540p) and `vs_shattered_glitch_core` (30.1 @540p)
each need roughly a 2x cut — a march step-count and iteration reduction, not a
rewrite. Both are VS2-tier visuals worth saving. `vs_dimensional_monolith` needs
3x. `vs_granular_nebula` runs at 0.1 fps at 1080p — **cut it, do not attempt**. Target: **≥58 fps at the size
G-01 recommended, with G-12's post chain on `full`.** You are designing to a
measured budget, not to taste.

VideoCore VII cannot brute-force raymarching. Techniques that look 3D and are
cheap:
- 2D domain-warped noise with fake height shading
- Single-plane raymarch (intersect one plane, texture it) instead of full SDF marching
- Analytic shapes — spheres, tori, boxes — with closed-form intersection
- Layered parallax quads rather than volumetric marching
- **Trails do the depth work for you.** G-12's feedback buffer makes a flat
  shader read as volumetric. Lean on it and keep the shader itself thin.

Hard limits: no loop over 48 iterations; no nested marching loops; prefer
`smoothstep` over `pow`; avoid `sin/cos` inside the innermost loop.

## Deliver four, in ISF format matching the existing `gig/shaders/*.fs` header
1. **Beat-gated tunnel** — cheap 2D polar tunnel; ring emission on `uKick`,
   rotation on `barPhase`. The workhorse — most of the set will use this.
2. **Liquid metal warp** — domain-warped flow with fake specular. Reads as
   expensive, costs almost nothing.
3. **Strobe geometry** — hard-edged shapes snapping on beat divisions. Peak-time
   tool. Include an intensity cap; do not ship a full-frame white strobe.
4. **Deep drift field** — slow, low-contrast, no beat sync. **Every set needs
   somewhere to go that isn't intense.** This one is not optional; ambient
   material is what makes the loud material land.

Use the standard audio uniforms from G-04 (`uBass`, `uKick`, `uEnergy`,
`uBeatPhase`, `uBarPhase`, …). Declare `INPUTS` in the ISF header for anything
G-13 should be able to modulate.

## Safety
No strobing above ~10 Hz, and none of the full-screen high-contrast flashing
that triggers photosensitive reactions. This runs at a public event on a 100"
screen. Cap it in the shader, not in the UI.

## Observable
All four in `PERF.md` tagged `SAFE` at the target size **with post on**, measured
on the Pi. Then look at them on the TV — if one doesn't earn its place in the
set, say so in `../RESULTS.md` and cut it. Three good ones beat four filler.
