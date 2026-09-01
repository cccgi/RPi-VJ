# G-20 — Composite FX ids (mirror+posterize+palette in one slot)

Protocol: AGENT_PROTOCOL_V2. One task = one conversation. Model: cheap.
Files to open: `gig/src/mixer.js`, `gig/show.json`. That is all. Do not open anything else.

## Why (from reference analysis 2026-08-26)
13 frames from the reference video (`refs/_small/ref01..13.jpg`) were analysed.
**11 of 13 are a mirror/kaleidoscope fold over a detailed source, plus a heavy
posterize/contour step, plus a palette remap.** They are NOT raymarching. The
expensive part (image detail) comes from footage we already have 30 clips of.

`mixer.js` already has every ingredient: fx 9/10/11 (mirrors), 12 (kaleido),
16 (twirl), 5 (posterize), 2 (hue shift), and both `fxUV` (geometry, pre-sample)
and `fxColor` (colour, post-sample) are already called with the SAME id:

```
if (u_fx > 0) duv = fxUV(duv, u_fx, u_fx_amount);
...
if (u_fx > 0) deckSample.rgb = clamp(fxColor(deckSample.rgb, uv, u_fx, u_fx_amount), 0.0, 1.0);
```

**The gap is that `u_fx` is one int, and every existing id is geometry XOR colour.**
So today you can have mirror OR posterize, never mirror→posterize. That single
limitation is the whole distance between our output and the reference.

Fix: add composite ids that branch in BOTH functions. No new uniforms, no new
passes, no change to show.js, streamdeck.js, or the binding schema. Cost on Pi is
a few extra ALU ops inside the existing single mixer pass — no new texture reads
except id 29.

## Part A — add a palette helper to the mixer shader

Insert immediately AFTER the existing `hueShift` function in `mixer.js`:

```glsl
// Cosine palette (Inigo Quilez form). t in 0..1 -> rgb.
vec3 pal(float t, vec3 a, vec3 b, vec3 c, vec3 d) {
  return a + b * cos(6.2831853 * (c * t + d));
}
```

## Part B — geometry branches

Append these to `fxUV`, before its final `return uv;`:

```glsl
  else if (fx == 25) {                       // MANDALA: kaleido + concentric repeat
    vec2 p = uv - 0.5;
    float ang = atan(p.y, p.x), r = length(p);
    float seg = 6.2831853 / 8.0;
    ang = abs(mod(ang, seg) - seg * 0.5);
    r = mix(r, fract(r * mix(1.0, 5.0, a)) * 0.5, a);
    uv = 0.5 + vec2(cos(ang), sin(ang)) * r;
  }
  else if (fx == 26) {                       // TEMPLE: quad mirror, nested twice
    uv = abs(uv - 0.5) + 0.5;
    uv = abs(uv * mix(1.0, 2.0, a) - 0.5) + 0.5;
  }
  else if (fx == 27) {                       // SMOKE MIRROR: quad mirror + slow drift
    uv.x += sin(u_fx_time * 0.11) * 0.06 * a;
    uv = abs(uv - 0.5) + 0.5;
  }
  else if (fx == 28) {                       // SOLAR: kaleido + rotation
    vec2 p = uv - 0.5;
    float ang = atan(p.y, p.x) + u_fx_time * 0.05;
    float r = length(p);
    float seg = 6.2831853 / mix(6.0, 16.0, a);
    ang = abs(mod(ang, seg) - seg * 0.5);
    uv = 0.5 + vec2(cos(ang), sin(ang)) * r;
  }
  else if (fx == 29) {                       // NEON: kaleido, wide segments
    vec2 p = uv - 0.5;
    float ang = atan(p.y, p.x), r = length(p);
    float seg = 6.2831853 / 5.0;
    ang = abs(mod(ang, seg) - seg * 0.5);
    uv = 0.5 + vec2(cos(ang), sin(ang)) * r;
  }
  else if (fx == 30) {                       // DEEP FOLD: mirror + twirl
    uv.y = abs(uv.y - 0.5) + 0.5;
    vec2 p = uv - 0.5; float r = length(p);
    float ang = atan(p.y, p.x) + a * 2.5 * (0.35 - min(r, 0.35));
    uv = 0.5 + vec2(cos(ang), sin(ang)) * r;
  }
```

## Part C — colour branches

Append these to `fxColor`, before its final `return c;`:

```glsl
  if (fx == 25) { float n = mix(16.0, 4.0, a); return floor(c * n) / n; }
  if (fx == 26) { float n = mix(20.0, 5.0, a);
                  vec3 q = floor(c * n) / n;
                  return mix(c, q * vec3(1.35, 0.85, 0.45), a); }
  if (fx == 27) { return mix(c, clamp((c - 0.42) * 2.2 + 0.42, 0.0, 1.0), a); }
  if (fx == 28) { float l = luma(c);
                  return mix(c, pal(l + u_fx_time * 0.02,
                                    vec3(0.5), vec3(0.5),
                                    vec3(1.0, 0.9, 0.6),
                                    vec3(0.0, 0.15, 0.35)), a); }
  if (fx == 29) { float l = luma(c);
                  float n = mix(12.0, 4.0, a);
                  float band = floor(l * n) / n;
                  float edge = abs(l - band) * n;
                  return mix(c, pal(band, vec3(0.35), vec3(0.65),
                                    vec3(1.0), vec3(0.0, 0.33, 0.67))
                                * (0.35 + edge * 1.8), a); }
  if (fx == 30) { return hueShift(mix(c, vec3(luma(c)), a * 0.3),
                                  a * 0.5 + u_fx_time * 0.01); }
```

## Part D — bind them

In `show.json`, extend the existing `videofx` and `shaderfx` banks with six keys
each, using the exact same entry shape already used for fx 1..24:

| id | label   | reference frame |
|----|---------|-----------------|
| 25 | MANDALA | ref03, ref08, ref11 |
| 26 | TEMPLE  | ref09, ref12 |
| 27 | SMOKE   | ref06 |
| 28 | SOLAR   | ref04 |
| 29 | NEON    | ref10 |
| 30 | FOLD    | ref05 |

Do not renumber or move any existing key.

## Observable (DONE means this was seen on the Pi)
1. Kiosk loads with no shader compile error in the console — check via
   `python3 ~/RPi-VJ/gig/tools/cdp_freeze_test.py` on the Pi itself (127.0.0.1,
   never `localhost`).
2. With a clip playing, pressing the MANDALA key produces a radially symmetric,
   posterized image that visibly differs from both fx 12 and fx 5 alone.
3. All six ids hold 1080p60 with the post chain at its normal tier. If any one
   drops frames, report which — do not silently lower quality.

## Out of scope for Saturday
ref01 (reaction-diffusion), ref02 (particle swarm), ref13 (volumetric nebula).
These need ping-pong sim buffers, point-sprite geometry, or raymarched 3D noise
respectively. They belong to the 3090 track, not the Pi. Do not attempt them here.
