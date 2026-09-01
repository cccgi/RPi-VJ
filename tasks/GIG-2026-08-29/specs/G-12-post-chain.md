# G-12 — Post-Processing Chain
**Model:** mid · **Blocked by:** G-07 · **Highest visual value in the project**

## Why this matters more than new shaders
The complaint is that visuals look basic next to VS 2 / Resolume. The gap is
mostly post-processing, not shader complexity. Trails and bloom are cheap and
transform the look; a more elaborate raymarcher is expensive and doesn't.
Budget your time accordingly.

## Deliverable
`gig/src/post.js` — a chain applied to the mixer's output.

## The chain, in order
```
mixerOutput ──> [1 feedback] ──> [2 bloom] ──> [3 CA + grain + vignette] ──> screen
```

### 1. Feedback / trails  ← biggest win, do this first
Ping-pong two FBOs. Each frame:
```glsl
vec2 uv2 = (uv - 0.5) * (1.0 - zoom) + 0.5;   // zoom: ~0.002..0.02
uv2 += rotate(uv2 - 0.5, rotSpeed);            // optional slow rotation
vec3 prev = texture(uPrev, uv2).rgb * decay;   // decay: 0.85..0.98
out = max(current, prev);                      // max() keeps trails bright
```
Params: `decay`, `zoom`, `rotation`, `mode` (max | add | mix). Expose all four —
they are the most performative controls in the whole engine and want to be on
X-Touch faders eventually.
**Beware:** `decay` ≥ 0.99 with `add` blows out to white in seconds. Clamp.

### 2. Bloom
Threshold → downsample → separable Gaussian → upsample → add.
- Downsample to **1/4 resolution** before blurring. Blurring at full res on V3D
  will cost you the frame budget for no visible gain.
- Two passes (horizontal, vertical), 9 taps each.
- Params: `threshold` (0.6–0.9), `intensity`, `radius`.

### 3. Finish pass (one shader, all three effects)
- **Chromatic aberration**: sample R/G/B at slightly different radial offsets,
  amount scaled by distance from centre and modulated by `uKick`. A CA pulse on
  the kick is the single most "expensive-looking" cheap trick available.
- **Grain**: hash noise, amount 0.02–0.06. Kills banding on the TV's gradients.
- **Vignette**: subtle, keeps the eye centred on a 100" panel.

## Format — MEASURED, do not re-decide
On this Pi: `EXT_color_buffer_float` **available**, `OES_texture_float_linear`
**NOT available**. Float targets exist but cannot be linearly filtered, and the
bloom chain depends on linear filtering for its downsample/upsample.
**Use `RGBA8` throughout.** Do not attempt RGBA16F for the blur chain.

## Budget — MEASURED, do not guess
Frame budget 16.7 ms. Shader costs (from `gig/probe/PERF.md`):
Tier A shaders 2.1–4.2 ms, Tier B 5.6–8.4 ms.
**The whole post chain must fit in ~5 ms at 1080p** to leave room for a video
deck alongside a Tier B shader. Measure each stage separately and report it.

## Cost control
Every stage individually toggleable, and a global `postQuality` of
`off | light | full`:
- `light` = feedback + finish pass, no bloom
- `full` = everything
Wire `postQuality` into G-07's adaptive governor: **drop bloom before dropping
render scale.** Losing bloom is less visible than losing resolution.

## Observable
On the **Pi**: the same shader with post `off` vs `full`, side by side or
A/B toggled, plus the fps cost of each stage measured separately and recorded in
`../RESULTS.md`. Then judge it on the actual TV — if `full` doesn't look
dramatically richer, something is wrong with the parameters, not the concept.
