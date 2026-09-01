#!/usr/bin/env python3
"""Generate gig/presets.json — 5 presets for each of the 18 shaders.

Design: the five slots mean the SAME THING on every shader, so the surface is
muscle memory rather than 18 separate vocabularies.

    1 CALM   barely moving, wide and soft — under a busy clip, or an intro
    2 FLOW   gentle motion, still readable as a background
    3 DRIVE  the default working setting; what the shader was tuned for
    4 HARD   fast and dense, front-of-mix
    5 PEAK   maximum everything, for a drop

Values are hand-picked per parameter rather than a linear sweep, because
"more intense" is not always "higher": deep_drift's `softness` and
spectrum_bars' `bar_width` both get SMALLER as the preset gets harder.

Regenerate:  python3 gig/tools/build-presets.py
"""
import json, os

NAMES = ["CALM", "FLOW", "DRIVE", "HARD", "PEAK"]

# shader -> { param: [calm, flow, drive, hard, peak] }
P = {
 "audio_oscilloscope":       {"beam_thickness":[0.030,0.022,0.015,0.010,0.006],
                              "glow":[1.0,1.6,2.2,3.0,4.0]},
 "audio_wave_rings":         {"ring_density":[7,11,14,20,28],
                              "expand":[0.4,0.8,1.2,1.7,2.4]},
 "beat_tunnel":              {"speed":[0.5,1.0,1.6,2.5,3.6],
                              "sides":[4,5,6,8,12],
                              "kick_response":[0.6,1.0,1.5,2.2,3.0],
                              "color_shift":[0.0,0.25,0.5,0.75,0.9]},
 "cyber_grid":               {"grid_speed":[0.35,0.7,1.1,1.8,2.8],
                              "mountain_height":[0.7,1.1,1.5,2.2,3.0]},
 "deep_drift":               {"drift_speed":[0.08,0.20,0.45,0.80,1.40],
                              "softness":[2.3,1.8,1.2,0.9,0.55],
                              "brightness":[0.6,0.85,1.1,1.5,2.0]},
 "kaleidoscope_mirror":      {"segments":[4,6,8,12,16],
                              "zoom":[1.0,1.6,2.2,3.2,4.6]},
 "liquid_metal":             {"viscosity":[0.35,0.6,1.0,1.8,2.7],
                              "warp_depth":[0.6,1.0,1.4,2.1,2.9],
                              "metallic":[0.9,0.85,0.8,0.6,0.4],
                              "iridescence":[0.15,0.35,0.6,0.8,1.0]},
 "multiband_rings":          {"ring_count":[3,4,5,7,9],
                              "spin":[0.3,0.7,1.1,1.9,2.8]},
 "neon_tunnel":              {"speed":[0.35,0.7,1.2,2.2,3.5],
                              "glow_intensity":[0.8,1.2,1.6,2.2,2.9]},
 "particle_warp":            {"warp_speed":[0.8,1.4,2.2,3.6,5.6]},
 "plasma_flow":              {"flow_speed":[0.3,0.6,1.0,1.8,2.8]},
 "spectrum_bars":            {"bar_width":[0.18,0.15,0.12,0.08,0.05],
                              "glow":[0.4,0.8,1.2,1.8,2.4]},
 "spectrum_flow":            {"trail":[0.45,0.8,1.1,1.5,1.9],
                              "gain":[0.6,0.9,1.2,1.8,2.4]},
 "strata_drift":             {"layers":[3,4,5,6,7],
                              "drift":[0.3,0.7,1.1,1.9,2.8]},
 # pattern_mode: <1 box, 1..2 diamond, >=2 crosshair. color_mode 0..2.
 "strobe_geometry":          {"grid_density":[2,3,5,8,11],
                              "flash_intensity":[0.25,0.45,0.65,0.85,1.0],
                              "pattern_mode":[0,1,2,1,2],
                              "color_mode":[0,0,1,1,2]},
 "vs_cyber_wormhole":        {"warp_intensity":[0.4,0.7,1.2,1.9,2.8],
                              "ring_density":[6,9,12,17,23]},
 "vs_sacred_polyhedron":     {"rotation_speed":[0.35,0.7,1.2,2.2,3.6],
                              "shard_dispersion":[0.7,1.1,1.6,2.4,3.3]},
 "vs_shattered_glitch_core": {"glitch_intensity":[0.25,0.5,1.0,1.9,2.9],
                              "core_speed":[0.35,0.7,1.1,1.9,2.8]},
}

root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")

# Validate against the shaders' own ISF headers: a preset naming a parameter
# the shader does not have, or a value outside its declared MIN/MAX, is silently
# ignored by the engine and would be invisible until it looked wrong on stage.
problems = []
for shader, params in P.items():
    path = os.path.join(root, "shaders", shader + ".fs")
    if not os.path.exists(path):
        problems.append("%s: no such shader file" % shader); continue
    src = open(path).read()
    meta = json.loads(src[src.index("/*") + 2: src.index("*/")])
    declared = {i["NAME"]: i for i in meta.get("INPUTS", [])}
    for name in declared:
        if name not in params:
            problems.append("%s: input '%s' has no preset values" % (shader, name))
    for name, vals in params.items():
        if name not in declared:
            problems.append("%s: preset param '%s' is not an input" % (shader, name)); continue
        if len(vals) != 5:
            problems.append("%s.%s: %d values, need 5" % (shader, name, len(vals))); continue
        lo, hi = declared[name].get("MIN"), declared[name].get("MAX")
        for v in vals:
            if lo is not None and (v < lo or v > hi):
                problems.append("%s.%s: %s outside %s..%s" % (shader, name, v, lo, hi))

if problems:
    print("FAIL:")
    for p in problems: print("  " + p)
    raise SystemExit(1)

out = {"_comment": "Generated by tools/build-presets.py — do not hand-edit.",
       "slotNames": NAMES, "shaders": {}}
for shader, params in P.items():
    out["shaders"][shader] = [
        {"name": NAMES[i], "inputs": {k: v[i] for k, v in params.items()}}
        for i in range(5)
    ]

with open(os.path.join(root, "presets.json"), "w") as f:
    json.dump(out, f, indent=1)
print("OK: %d shaders x 5 presets, all params validated against ISF headers"
      % len(out["shaders"]))
