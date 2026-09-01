# G-04 — ISF-lite Header Parser + GLSL Shim
**Model:** cheap · **Blocks:** G-07

## Why
`vj/shaders/*.fs` (14 files) are ISF-format: a JSON blob in a leading
`/*{ ... }*/` comment, then GLSL that uses ISF's implicit globals. Nothing in
the repo has ever parsed or compiled them. This task makes them runnable.

## Deliverable
`gig/src/isf.js` — ES module, no dependencies.

## API
```js
export function parseISF(source)
// -> { meta: {DESCRIPTION, CATEGORIES, INPUTS: [...], ...},
//      inputs: [{NAME, TYPE, DEFAULT, MIN, MAX, LABEL}],
//      glsl: "<#version 300 es fragment shader ready to compile>" }
// Throws only on malformed JSON. Missing header => treat whole file as GLSL
// with empty inputs.
```

## The shim you must prepend to the GLSL body
ISF shaders assume these exist. Emit them:
```glsl
#version 300 es
precision highp float;
uniform vec2 RENDERSIZE;
uniform float TIME;
uniform float TIMEDELTA;
uniform int FRAMEINDEX;
uniform vec4 DATE;
in  vec2 v_texCoord;      // supply from the vertex stage OR compute below
out vec4 out_FragColor;
#define gl_FragColor out_FragColor
#define isf_FragNormCoord (gl_FragCoord.xy / RENDERSIZE)
#define vv_FragNormCoord isf_FragNormCoord
```
Because G-03's vertex shader has no varyings, derive `isf_FragNormCoord` from
`gl_FragCoord` rather than adding an attribute.

**It MUST be a `#define`, not a global variable.** GLSL ES requires global
initializers to be constant expressions, so
`vec2 isf_FragNormCoord = gl_FragCoord.xy / RENDERSIZE;` fails to compile with
`'=' : global variable initializers must be constant expression` on every
shader. This was verified on the Pi on 2026-08-25 — do not "fix" it back.

Then, for each entry in `INPUTS`, emit a uniform of the mapped type:
`float→float`, `bool→bool`, `long→int`, `point2D→vec2`, `color→vec4`,
`image→sampler2D`, `event→bool`.

## GLSL ES 1.00 → 3.00 fixups (apply to the body)
The existing `.fs` files are written loosely. Rewrite in the body:
- `texture2D(` → `texture(`
- `varying ` → `in `  (only if it appears)
- leave `gl_FragColor` alone — the `#define` above handles it
- if the body already declares `#version`, strip that line

## Audio uniform convention (this project's own extension)
Also always emit these, whether or not the header declares them, since our
shaders and G-05 both depend on them:
```glsl
uniform float uSubBass, uBass, uLowMid, uHighMid, uTreble;
uniform float uEnergy;     // EMA 0..1
uniform float uKick;       // 1.0 on onset frame, decaying
uniform float uBPM, uBeatPhase, uBarPhase;   // phase in 0..1
```
Guard against redeclaration: if a name is already declared in the body or in
INPUTS, skip emitting it.

## Observable
A test page that loads **all 14** files from `vj/shaders/` (copy them to
`gig/shaders/` first — do not modify the originals), parses each, compiles each
via G-03's `ShaderHost.compile()`, and prints a pass/fail table with the
compile log for any failure.

Paste the table into `../RESULTS.md`. Failures are expected and fine — the
deliverable is knowing *which* 14 work and why the rest don't. Do not "fix" a
shader by weakening the shim; note it and move on.
