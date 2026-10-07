# Shader adaptation rules for the Pi 5 (WP-C) — 2026-10-07

Derived from the 18 original shaders in `engine/examples/shaders/` (byte-identical
to `gig/shaders/*.fs`), their earlier versions in `vj/shaders/`, the two retired
shaders in `gig/shaders/_retired/`, and the owner's benchmark notes.

**Status of the numbers.** Every count in sections 1-2 was counted from source.
Every fps / headroom figure is quoted from the owner's August 2026 notes, which
were measured in the abandoned Chromium/WebGL renderer (V3D 7.1.10.2, Mesa
25.0.7), not in the native engine. Nothing here has been measured with the new
`--bench` mode. Section 6 lists what WP-D must confirm.

## 1. The 18 originals — measurable facts

Common to all 18: single pass, **0 texture fetches**, no `discard`, no nested
loops, no supersampling/AA loop, no `precision` statement (engine injects
`highp`), 45-154 lines. "Transc." = calls to `sin/cos/atan/exp/pow` executed per
pixel on the worst path, including ones whose arguments depend only on uniforms
(u-only). A value-noise tap = 4 hashes = 4 `sin`.

| Shader | Technique | Loops (bound) | Transc./px (sin+cos / atan / exp / pow) | Noise taps | Branch in loop / early exit | Recorded Pi result (source) |
|---|---|---|---|---|---|---|
| plasma_flow | 2D sine plasma | none | 9 (9/0/0/0), 2 u-only | 0 | - | 1080p, 5x headroom, 3.3 ms (PERF) |
| neon_tunnel | 2D polar tunnel `1/r` | none | 6 (5/1/0/0) | 0 | - | 1080p, 6x, 2.8 ms (PERF, pre-flow version) |
| beat_tunnel | 2D polar polygon tunnel | none | 5 (4/1/0/0), 2 u-only | 0 | - | 1080p, 7x, 2.2 ms (PERF) |
| kaleidoscope_mirror | polar fold | none | 11 (10/1/0/0), 2 u-only | 0 | - | 1080p, 4x, 4.2 ms (PERF, earlier version) |
| audio_oscilloscope | 2D line + halo | none | 3 (3/0/0/0) | 0 | - | 1080p, 8x+, 2.1 ms (PERF) |
| spectrum_bars | 5 bars | 1 (5) | 0 | 0 | 2 nested `if` in loop | 1080p, 6x, 2.8 ms (PERF) |
| spectrum_flow | 32 columns | none | <=4 (1/0/3/0) | 0 | `if (hit > 0.01)`; 2 `u_spectrum[]` reads | none found |
| cyber_grid | analytic single-plane perspective | none | 3 (3/0/0/0) | 0 | early `return` for the sky half | 1080p, 8x+, 2.1 ms (PERF) |
| strobe_geometry | 2D grid SDFs, `step()` edges | none | 3 (3/0/0/0), all u-only | 0 | one `if/else` | 1080p, 8x+, 2.0 ms (PERF) |
| liquid_metal | domain warp, 1 height sample, `dFdx/dFdy` normal | none | 12 (12/0/0/0) | 0 | - | 1080p, 6x, 2.5 ms (PERF); "1.0x" before the fix (source comment) |
| deep_drift | 2D fbm (2 octaves) + 1 tap | none | 12 (12 hash sin) | 3 | - | 1.0x -> 1.50x -> 1.99x (source comment, handoff); PERF says 8x+ (conflict, see 3.3) |
| multiband_rings | 5 polar rings | 1 (5) | 11 (5/1/0/5) | 0 | ternaries only; 10 `u_spec_flow[]` reads | no number; 30-read version "cost this shader its 1080p verdict" (source comment) |
| audio_wave_rings | 3 polar wavefronts | 1 (3) | 16-19 (6/1/6-9/3), 3 exp u-only | 0 | `if (hit > 0.02)` in loop | rewrite unmeasured; earlier 8-ring version 1x, 16.7 ms (PERF) |
| vs_cyber_wormhole | polar tunnel x3 colour channels | 1 (3) | 16 (12/3/1/0) | 0 | 5 `if (ch == n)` in loop | 1080p, 2x, 8.4 ms (PERF) |
| particle_warp | 3 star layers + 4 comets | 2 (3, 4) | 29 (10/2/17/0), 5 u-only | 0 | none | 3x before comets (PERF); 1.0x after, scale 0.7 (`gig/src/mixer.js`) |
| strata_drift | 5-7 parallax value-noise layers | 1 (7, `break` at `layers`, default 5) | ~61 default, ~77 at 7 layers | 13 default, 17 max (2 octaves/layer) | `if (i >= n) break`; lightning `if (strike > 0.02)` | 20 fps at 1080p, needs 720p, scale 0.7 (`mixer.js`) |
| vs_sacred_polyhedron | SDF raymarch, 3 polyhedra | 1 (28) | per step: 14 u-only trig + 1 exp | 0 | hit `break`, far `break` | 28.5 / 42.0 / 59.9 fps at 1080/720/540p (PERF); scale 0.4 (`mixer.js`) |
| vs_shattered_glitch_core | SDF raymarch, 2 boxes + sphere | 1 (28) | per step: 4 u-only trig + 1 sin + 1 exp | 0 | hit `break`, far `break` | 30.0 / 45.0 / 59.9 fps (PERF); scale 0.4 (`mixer.js`) |

Raymarch parameters (identical in both): 28 steps, far cap `dist > 14.0`
(camera 4.2-4.5 units from a scene of radius ~2.5), hit epsilon 0.003, step
`max(d * 0.95, 0.03)`, 3-tap forward-difference normal (epsilon 0.006) evaluated
once on hit, 1 glow `exp` per step, at most 3 primitives in `map()`, no domain
repetition, no `smin`, no noise in `map()`. Worst case 31 `map()` calls/pixel.

### Audio drive per shader

| Shader | Uniforms used | What they drive |
|---|---|---|
| plasma_flow | bass, mid, treble, onset | phase offsets of the three waves and colours; onset flash +0.25 |
| neon_tunnel | flow, flow_low/mid/high, bass, mid, treble, onset | travel on `u_flow_low`; ring frequency on mid/treble; glow threshold and gain on bass; colour phase on flow clocks; flash +0.4 |
| beat_tunnel | bass, kick/onset, energy/volume, hit_high/treble, mid, beat/bar phase x locked | mouth radius; bar-snapped rotation; ring width and brightness on energy; kick emission; master gain `0.12 + energy*1.5 + kick*0.5` |
| kaleidoscope_mirror | bass, mid, treble, onset | combined level -> zoom 2.35..0.80; onset zoom punch 0.18; glow threshold on bass; flash +0.35 |
| audio_oscilloscope | volume, bass, mid, treble, flow_low/mid/high, onset | amplitude and gate on level; each harmonic scrolls on its band clock; halo on onset |
| spectrum_bars | bass, mid, treble, onset | bar heights only |
| spectrum_flow | spectrum[32], hit_low/mid/high, volume | column level, comet length, burst on hit |
| cyber_grid | bass, mid, onset | sun size, scroll speed (`u_time*(speed + bass*3.2)`), terrain height; flash +0.9 |
| strobe_geometry | kick/onset, bass, energy/volume, hit_*, treble, mid, flow, beat/bar x locked | shape size on energy; zoom on bass/kick; rotation stepped by `floor(u_flow*2)`; flash capped 0.75, output clamped 0.92 |
| liquid_metal | bass, kick/onset/hit_low, energy/volume, hit_high/treble | time offset and zoom on bass/kick; relief and specular on energy; master gain `0.18 + energy*1.4 + kick*0.4` |
| deep_drift | volume, sub, bass, low_mid, treble | brightness lift, mist opening, sparkle; deliberately no onset |
| multiband_rings | sub, bass, low_mid, high_mid, treble, hit_*, spec_flow[32] | one ring per band: width, tooth sharpness (`pow`), brightness; rotation on that band's spectrum clocks |
| audio_wave_rings | sub+bass, low_mid+high_mid, treble, hit_*, volume | one wavefront per zone: speed, crest sharpness, hit ring; centre bloom on volume |
| vs_cyber_wormhole | flow_low, bass, mid, treble, onset | travel on `u_flow_low`; chromatic offset on onset/bass; per-channel gain; flash +0.22; soft shoulder `col/(1+0.55*col)` |
| particle_warp | bass, onset, flow_low, treble | speed and comet launch epoch on `u_flow_low`; comet size on bass+onset; star size on treble; flash +0.5 |
| strata_drift | sub, bass, low_mid, high_mid, treble, hit_high, hit_low | one band per layer (threshold and brightness); hits fire lightning; treble shimmer |
| vs_sacred_polyhedron | bass, mid, treble, onset | radius and one rotation axis on bass; shard burst on onset; glow per step on onset; flash +0.35 |
| vs_shattered_glitch_core | bass, mid, treble, onset | box/sphere/shell sizes; glitch slice on onset; glow on onset; flash +0.3 |

## 2. The envelope

| Measure | Maximum reached by the originals | Where |
|---|---|---|
| Passes / texture fetches / AA samples | 1 / 0 / 1 | all |
| Loop nesting depth | 1 (no loop inside a loop, no looped function called from a loop) | all |
| Loops per shader | 2 | particle_warp |
| Non-march loop bound | 7 (5 at default input) | strata_drift |
| Non-march loop iterations per pixel, total | 7 | particle_warp (3+4), strata_drift |
| Raymarch steps | 28 | both raymarchers |
| Raymarch far cap | 14.0 units, about 3x camera-to-origin distance | both |
| Hit epsilon / minimum step / relaxation | 0.003 / 0.03 / 0.95 | both |
| `map()` calls per pixel | 31 (28 + 3 normal taps) | both |
| Primitives per `map()` | 3 | both |
| Extra work per march step | 1 `exp` | both |
| fbm octaves | 2 | deep_drift, strata_drift |
| Noise type | 2D value noise, `fract(sin(dot))` hash; no 3D, simplex or Voronoi noise | 2 shaders |
| Noise taps per pixel at full 1080p | 3 | deep_drift |
| Noise taps per pixel at reduced resolution | 13 default, 17 max, at 0.7 scale | strata_drift |
| Transc. calls per pixel, full 1080p with recorded headroom >= 2x | 16 | vs_cyber_wormhole |
| Transc. calls per pixel, any tier | 29 at 0.7 scale; ~61-77 at 0.7 scale (noise) | particle_warp; strata_drift |
| `atan` per pixel | 3 | vs_cyber_wormhole |
| `pow` per pixel | 5 | multiband_rings |
| `exp` per pixel (non-march) | 17 | particle_warp |
| Dynamically indexed uniform-array reads per pixel | 10 | multiband_rings |
| Screen-space derivatives | 1 `dFdx` + 1 `dFdy` | liquid_metal |
| Resolution scale assigned | 1.0 for 14 shaders; 0.7 for 2; 0.4 for 2 raymarchers | `gig/src/mixer.js` `SHADER_SCALE_HINTS` |

Static count against recorded verdict (old renderer; coarse, to be confirmed):
<= 12 transc./px sat in the owner's Tier A (>= 4x headroom); 16 gave 2x; 24-32
used the whole 1080p frame (particle_warp after comets; the earlier 8-ring
audio_wave_rings at 32 trig); ~60 needed 720p. Owner's stated design target
(`tasks/GIG-2026-08-29/specs/G-14-new-shaders.md`): **4x headroom at 1080p,
<= 4.2 ms**, and "no loop over 48 iterations; no nested marching loops; prefer
`smoothstep` over `pow`; avoid `sin/cos` inside the innermost loop."

## 3. What changed, and why two were retired

Git holds one commit (`cc4787a`, "Preserve original RPi-VJ implementation"), so
there is no per-change history. The evidence is `vj/shaders/` (13 files, dated
08-24) against `gig/shaders/` (dated 08-26), plus comments in the files.

### 3.1 Performance changes

| Shader | Before -> after | Evidence |
|---|---|---|
| vs_sacred_polyhedron | 60 -> 28 steps; 6-tap central normal -> 3-tap forward normal reusing `d`; epsilon 0.001 -> 0.003; step `max(d*0.8, 0.015)` -> `max(d*0.95, 0.03)`; far 15 -> 14; `pow(x,2.5)` -> `x*x*x`; `pow(fresnel,6.0)` -> `fresnel*fresnel`; `normalize(vec3(phi,1,0))` -> `const` literals | diff; comments "Reduced from 60 to 28 iterations (2.1x speedup on Raymarching budget)", "3-sample fast normal (50% cheaper than 6-sample central differences)". 7.5 fps at 1080p before (`RESULTS.md`), 28.5 after (PERF) |
| vs_shattered_glitch_core | 55 -> 28 steps; torus-knot SDF (per-step `atan`, `rot`, an `if (mod(...) < 0.3)` branch) replaced by box+sphere+shell; branch became `step() *`; screen-space `if` on `gl_FragCoord` removed; `pow(fresnel,3.0)`, `pow(fresnel,8.0)` -> multiplies; same epsilon/step/normal changes | diff; "28 iterations (2x cut from standard 60-step march)". 10 fps before (`RESULTS.md`), 30.0 after (PERF). The geometry changed: a look change the owner accepted |
| liquid_metal | 3 `liquidHeight()` samples (finite-difference gradient) -> 1 sample + `dFdx/dFdy` | comment: "liquidHeight() costs ~9 trig ops and was evaluated three times per pixel ... which is why this shader benched at 1.0x headroom" |
| deep_drift | fbm 3 -> 2 octaves; second fbm -> one `noise()` tap (24 -> 16 -> 12 hash calls/px) | comments: third octave "only ever contributed 0.20 of amplitude ... its loss is not visible"; "took deep_drift from 1.50x to 1.99x headroom"; "sin()-vs-mul hash made ZERO difference in the same test (11.117ms both ways): the cost here is fbm() octave count, not the hash function, on this GPU" |
| multiband_rings | 6 spectrum-clock reads per ring -> 2 (30 -> 10 per pixel) | comment: "the six-tap average cost this shader its 1080p verdict -- 30 dynamic uniform-array reads per pixel -- and the extra taps changed the motion imperceptibly" |
| particle_warp | angle wrap `atan(sin,cos)` -> `mod` | comment: "mod does the same job far cheaper, and this ran once per comet per pixel". Adding the 4-comet loop took it from 3x to 1.0x headroom (`mixer.js`) |
| audio_wave_rings | 8-iteration ring loop (4 trig each) -> 3-iteration wavefront loop | PERF: "1x, 16.7 ms ... Surprisingly expensive for a 2D shader; likely an over-large ring loop. Cheap to optimise (reduce iteration count)". The rewrite also changed the look; no measurement of the rewrite was found |

### 3.2 Reactivity changes (not performance)

neon_tunnel, vs_cyber_wormhole, particle_warp, audio_oscilloscope: `u_time *
(speed + u_bass*k)` replaced by `u_time*small + u_flow_low*k`, because "scaling
u_time makes the whole tunnel jump whenever the level changes". kaleidoscope:
level-driven zoom added ("only its colour moved, which read as barely
reactive"). cyber_grid: gains raised 3-4x. vs_cyber_wormhole: angular constant
4.0 -> 4.18879 to close the `atan` seam; soft shoulder instead of a hard clip.
plasma_flow and spectrum_bars are unchanged.

### 3.3 The retired pair

| Shader | Facts from source | Recorded result |
|---|---|---|
| vs_dimensional_monolith | 70 steps, far 40.0, epsilon 0.001, step `max(d*0.7, 0.02)`, 6-tap normal (76 `map()` calls worst case), 2 `mod` domain repetitions, `smin`, 2 pixel-varying trig + 5 u-only trig per `map()`, 2 `pow` on hit | PERF: 6.0 / 12.0 / 20.0 fps at 1080/720/540p, "HEAVY — needs ~3x cut" |
| vs_granular_nebula | 45 fixed volumetric steps, each calling a 4-octave fbm of 3D value noise (8 hashes per tap): 180 noise taps, 1,440 hashes per pixel; 3 trig per step; loop nested in loop | PERF: 4.6 fps at 540p, "CUT — needs >10x". G-14: "runs at 0.1 fps at 1080p — cut it, do not attempt". BOARD: "granular_nebula is unrecoverable". (`RESULTS.md` says "~5" at 1080p; the notes disagree) |

The reason is recorded as performance only. **No note records the decision to
move them to `_retired/`**, and both retired files are byte-identical to their
`vj/shaders/` versions. Inference: granular_nebula was cut as instructed;
monolith was listed for a 3x cut in G-14 but no optimised version was ever
written, so it was retired unmodified rather than rescued. Against the envelope
it exceeds steps (70 vs 28), far cap (40 vs 14), `map()` calls (76 vs 31) and
uses domain repetition and `smin`, which no surviving shader does.

Conflicts in the notes, unresolved: `gig/probe/PERF.md` lists deep_drift at 8x+
and liquid_metal at 6x, while `tasks/HANDOFF_AUDIO_DONE.md` (one day later) and
the source comments put both at 1.0x before their fixes. `mixer.js` quotes 12.0
and 15.0 fps at 1080p for the two raymarchers; PERF quotes 28.5 and 30.0. The
0.4 scale hint is 768x432, not the 960x540 that PERF tested. Treat all headroom
figures as coarse.

## 4. Rules

Auto = safe to apply by script (look unchanged beyond minor quality).
Judgement = changes the look; produce a candidate and have a person view it.
"Check" is written against Shadertoy-derived sources in
`target/native-13-shaders/`, where loops are often golfed
(`for(...; ++i < 77.; ...)`, `i++<1e2`), bounds of 64-200 are common, and many
march loops have no `break` (e.g. `33cGDj` 77 steps, `tlVGDt` 99, `MdX3zr` 64).

### 4.1 Rules the evidence supports

| # | Check | Limit | Evidence | Standard fix | Apply |
|---|---|---|---|---|---|
| E1 | Literal bound of the loop that advances a ray (`t += d`, `p += d*dir`, `z += k*d`) | <= 28 steps | Both rescues (3.1); G-14 hard limit 48 for any loop | Lower the bound to 28; raise relaxation toward 0.95, minimum step 0.03, hit epsilon 0.003. For accumulating marchers (no hit test, glow summed per step) multiply the accumulated value by old/new step count | Judgement |
| E2 | Shader is a raymarcher (rule E1 loop present) | Render tier 540p or lower unless measured otherwise | 28 steps: 28.5-30 fps at 1080p, 42-45 at 720p, 59.9 at 540p (PERF); owner ran them at 0.4 scale | Set `render_height` 540 in the manifest; try 720 only after measuring | Auto |
| E3 | Normal function with 6 `map()` calls (`map(p+e) - map(p-e)`) | 3 extra `map()` calls, reusing the hit distance | sacred_polyhedron diff; "50% cheaper" | `normalize(vec3(map(p+e.xyy)-d, map(p+e.yxy)-d, map(p+e.yyx)-d))`, epsilon 0.006 | Auto |
| E4 | A looped function (fbm, inner march, shadow/AO loop) called from inside a loop | Not allowed | granular_nebula (45 x 4-octave 3D fbm) cut as unrecoverable; G-14 "no nested marching loops"; 0 of 18 nest | Remove the inner loop (single noise tap, drop soft shadows/AO), or rebuild as 2D layered noise (strata_drift pattern); otherwise reject | Judgement |
| E5a | Octave count of each fbm (loop bound or unrolled terms) | <= 2 octaves | deep_drift 3 -> 2; strata_drift 2 | Truncate the octave loop | Auto only for trailing octaves whose summed amplitude is <= 20% of the total (owner's criterion: 0.20 "not visible"; with gain 0.5 that keeps 3 octaves); beyond that, Judgement |
| E5b | Noise taps per pixel = sum over fbm calls of octaves, plus bare `noise()` calls | <= 3 for 1080p; <= 13 needs the 720p tier | deep_drift 6 -> 4 -> 3 taps gave 1.0x -> 1.5x -> 1.99x; strata_drift 13 taps needs 720p | Replace secondary fbm calls (those feeding only a blend or sparkle) with one tap; assign 720p tier if still over | Judgement (tier assignment Auto) |
| E6 | Same heavy scalar field evaluated 3+ times at offsets only to get a gradient | 1 evaluation | liquid_metal (3.1) | `dFdx/dFdy` of the single sample, scaled by pixels-per-unit | Judgement (2x2-pixel derivative blocks; works only for screen-space 2D fields) |
| E7 | `pow(x, c)` with literal `c` | 0 `pow` on the hit/shading path; <= 5 per pixel overall | Both raymarcher diffs; G-14 "prefer `smoothstep` over `pow`" | `c` in {2,3,4}: multiplies (Auto, exact for x >= 0). Other `c` (2.5, 6, 8, 16): nearest multiply chain or `smoothstep` (Judgement; the owner changed 2.5 -> 3 and 6 -> 2 and accepted the shift) | Auto / Judgement |
| E8 | `atan(sin(a), cos(a))` used to wrap an angle | 0 occurrences | particle_warp comment | `mod(a + PI, 2*PI) - PI` | Auto |
| E9 | `normalize()` / trig of literal-only arguments evaluated at run time | 0 | sacred_polyhedron diff (`const vec3` literals) | Precompute to `const` | Auto (benefit unmeasured; the compiler may already fold it) |
| E10 | Supersampling: `AA` / `SAMPLES` defines, a loop over sub-pixel offsets wrapping the whole image | 1 sample | 0 of 18 originals supersample | Set the define to 1 / remove the loop (WP-B `lite`) | Auto |
| E11 | Non-march loops: literal bound, and transc. calls inside the body | Bound <= 7; <= 4 pixel-varying transc. calls per iteration; <= 16 transc. calls per pixel in total for the 1080p tier | Envelope (section 2); earlier audio_wave_rings (8 x 4 trig) used the whole frame; particle_warp at 29 dropped to 0.7 scale | Reduce iterations; hoist uniform-only terms out of the loop; else assign the 720p tier | Judgement (hoisting Auto) |
| E12 | Whole-image work repeated per colour channel (chromatic aberration by re-running the scene 3 times) | Only if one evaluation costs <= ~5 transc. calls | vs_cyber_wormhole: 3 x 5 + 1 = 16 calls, 2x headroom, the most expensive full-1080p original | Evaluate once and offset the result, or drop the effect | Judgement |
| E13 | `if` inside `map()` / the SDF | 0 | glitch_core diff: per-step `if (mod(..) < 0.3)` replaced by `step(0.7, fract(..)) *` | Rewrite as `step`/`mix` | Judgement (benefit not separately measured; it changed together with the geometry) |
| E14 | Dynamically indexed uniform-array reads per pixel | <= 10 | multiband_rings 30 -> 10 | Fewer taps; compute the index once | Judgement (only relevant when adding spectrum reactivity) |
| E15 | `map()` content: `mod`/`fract` domain repetition, `smin`, noise or trig of position inside `map()` | Originals: none; <= 1 pixel-varying trig per `map()` | monolith used repetition + `smin` + 2 pixel-varying trig and missed by 3x at 70 steps | No standard fix; count it as a risk flag and measure | Judgement |
| E16 | Far-distance exit in the march loop | Present; cap about 3x camera distance (14.0 in the originals) | Both raymarchers; monolith capped at 40 | Add `if (t > FAR) break;` | Judgement (choosing FAR clips the scene; many bank shaders rely on infinite repetition) |
| E17 | Hash function style (`sin`-based vs multiply-based) | No change needed | deep_drift comment: zero difference measured | Do not spend a transform on it | - |
| E18 | Texture samplers, buffer passes, `discard` | 0 | 0 of 18; engine binds no channel textures (plan F6) | Classification only (plan R4) | Auto |

Derived tier guide for raymarchers, by linear scaling of the PERF figures
(inference, not measured): about 13 steps fit 1080p, about 20 fit 720p, 28 fit
540p, for a `map()` as light as the originals'.

### 4.2 General V3D / tile-GPU knowledge — unverified on this hardware

| # | Claim | Suggested handling |
|---|---|---|
| G1 | Fragment cost scales with pixel count, so 720p costs ~44% and 540p ~25% of 1080p. Loosely consistent with PERF (6/12/20 and 28.5/42/59.9+ fps) but the top values hit the vsync cap | Use the tier ladder; confirm scaling with `--bench` |
| G2 | V3D executes fragments in 2x2 quads; a `break` only stops work once all four pixels have exited, and non-uniform `if` bodies may be executed under a mask for all of them. Early exits help less than on desktop | Do not count on `break` to rescue a long loop; budget by the bound |
| G3 | Loops with literal bounds may be unrolled by Mesa; very long bodies can fail register allocation or compile slowly on V3D | Watch `compile_ms` and `compile_error` in bench output for shaders with bounds >= 100 |
| G4 | `mediump` may not reduce cost on V3D, and can break hash functions that rely on large `sin` arguments | Leave `highp`; do not add a precision-lowering transform |
| G5 | Uniform-only expressions inside `map()` may not be hoisted out of the loop. Weak counter-evidence: the two raymarchers measure about the same (28.5 vs 30.0 fps) with 14 vs 4 u-only trig per step | Hoisting to `main()` is look-preserving; apply it and measure |
| G6 | Dependent texture reads and `textureLod` in a march loop are expensive on a tile GPU | Relevant only once channel textures exist |

Platform facts that are recorded (PERF, old renderer): `OES_texture_float_linear`
not available; `MAX_TEXTURE_SIZE` 4096; `MAX_FRAGMENT_UNIFORM_VECTORS` 1024.

## 5. Audio-reactive conventions

**Uniforms** (`engine/src/shader.rs` `FLOAT_UNIFORMS`): levels `u_bass u_mid
u_treble u_volume u_energy u_sub u_low_mid u_high_mid`; transients `u_onset
u_kick u_beat_onset u_hit_low u_hit_mid u_hit_high`; tempo `u_beat_phase
u_bar_phase u_locked`; integrated clocks `u_flow u_flow_low u_flow_mid
u_flow_high`; arrays `u_spectrum[32] u_spec_flow[32]`. camelCase aliases exist
and the G-14 shaders read both with `max()`.

**How the originals use them**

1. No smoothing in the shader. Levels arrive already enveloped; shaders only
   clamp (`clamp(x, 0.0, 1.0..2.0)`) and saturate (`1.0 - exp(-x*1.2..1.6)`,
   `clamp(lvl*1.55..2.2, 0, 1)` so brightness is full by level 0.45-0.65 while
   motion keeps scaling).
2. Motion follows integrated clocks, not level-scaled time:
   `u_time*speed*0.20..0.35 + u_flow_low*2.2..3.2`. Level-scaled time
   (`u_time*(a + u_bass*b)`) was removed from four shaders because it jumps; it
   survives only in cyber_grid.
3. Band roles: bass -> size, zoom, travel, glow threshold; mid -> pattern
   frequency and secondary gain; treble -> fine detail, sparkle, highlights.
   Multiband shaders give each band its own spatial zone (ring, layer, column)
   rather than summing bands into brightness.
4. Geometry reacts, not only colour: zoom `uv *= 1.0 - bass*0.28..0.35 -
   kick*0.12..0.18` (beat_tunnel, liquid_metal, strobe_geometry, kaleidoscope).
5. Transients: additive white flash `col += vec3(u_onset * k)`, k = 0.22-0.5
   (cyber_grid 0.9); per-band hits fire discrete shaped events behind
   `if (hit > 0.01..0.02)`.
6. Master gain in the later shaders: `col *= 0.12..0.18 + energy*1.4..1.5 +
   kick*0.4..0.5`, i.e. nearly dark at silence. deep_drift is the calm one
   (`0.55 + volume*0.65 + sub*0.45`, no onset).
7. Tempo phases are multiplied by `u_locked` so a free-running clock never
   drives visuals.
8. Safety: strobe flash capped at 0.75 and output clamped to 0.92; wormhole
   uses a soft shoulder. G-14: no strobing above ~10 Hz, no full-frame white.

**Native engine caveats** (`engine/src/native/render.rs`, read from code):
`u_sub = bass`, `u_low_mid = u_high_mid = mid`, `u_hit_x = onset * band`,
`u_kick = u_beat_onset = u_onset`, `u_energy = u_volume`, and `u_locked` is
hard-coded `0.0`, so the five-band shaders see three bands and every
beat/bar-phase term is currently off. Clock rates: `flow += dt*(0.15 +
2.0*level)`, `spec_flow += dt*(0.05 + 2.0*level)`.

**The generated wrapper** (`engine/tools/make_native_shadertoy.py`) is only
partly consistent with the originals:

| Wrapper behaviour | Consistent? |
|---|---|
| Zoom `1 + 0.28*bass + 0.18*beat_onset` | Yes: same idea and magnitude as rule 4 |
| Brightness `clamp(0.65 + 0.55*volume + 0.20*bass, 0.25, 1.45)` | Partly: reacts, but the 0.65 floor keeps it bright at silence (originals: 0.12-0.18) |
| Rotation by `0.35*u_mid` (level sets the angle) | No: level-driven position is the "jump" the owner removed; originals would rotate on `u_flow_mid` |
| Shake `3.0*vec2(sin(u_time*5), cos(u_time*4))*u_bass` and wobble scaled by mid/treble/onset | No: fixed wall-clock rate scaled by level; no original displaces the whole frame |
| No `u_flow*`, no per-band hits, no onset flash, no per-shader mapping | No: every bank shader reacts identically; originals map bands to their own geometry |
| Gamma `pow(rgb, 0.70)` + luma gain, hard clamp to 1.0 | Not an original convention (they clamp 0.92 or soft-shoulder); also a cost |

Cost of the wrapper, counted from source: 2 pixel-varying trig + 5 uniform-only
trig + a 3-component `pow` per pixel, added to every bank shader. Against the
envelope's ~12-call comfortable budget this is not negligible; it should be
measured (bench a trivial `mainImage` with and without it).

## 6. Not found, and what must be measured

Not found: any benchmark of the 18 in the native engine; any figure for
spectrum_flow, multiband_rings (current version) or the rewritten
audio_wave_rings; the `shaderbench.log` that `mixer.js` cites; a per-change
history (single commit); a written decision retiring the two shaders; a stated
per-shader budget other than G-14's 4.2 ms / 4x target.

To confirm with `--bench` (WP-D) before these rules drive transforms:

1. p95 ms of all 18 at 720p and 1080p: fixes the reference envelope and settles
   the PERF / handoff conflicts (3.3).
2. E1/E2 step budget: one raymarcher at 13, 20 and 28 steps across tiers.
3. E5/E11 thresholds (3 taps, 16 calls): deep_drift and strata_drift at varied
   octave/layer counts.
4. Wrapper overhead (section 5).
5. G2, G5: early-exit benefit and uniform-hoisting benefit.
6. `docs/pi5-qualified-shaders.md` lists `33cGDj` (77 march steps, no early
   exit) at 60 fps 1080p in the native engine, far outside this envelope. Either
   the native renderer is much faster than the old Chromium path, or that
   benchmark is unreliable (plan findings F1-F3). Until measured, treat the
   limits above as conservative.
