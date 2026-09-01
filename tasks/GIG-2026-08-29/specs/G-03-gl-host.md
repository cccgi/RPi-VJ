# G-03 — WebGL2 Fullscreen-Quad Shader Host
**Model:** cheap · **Blocked by:** G-01 (use its RECOMMENDED size) · **Blocks:** G-04, G-07

## Deliverable
`gig/src/gl.js` — an ES module. No dependencies. No framework.
Plus `gig/probe/gl-test.html` proving it works.

## API (implement exactly this)
```js
export class ShaderHost {
  constructor(canvas, { scale = 1.0 } = {})   // scale = internal render scale
  compile(fragSrc)        // -> {ok:true, program} | {ok:false, log:string}
                          // NEVER throws. A bad shader must not kill the show.
  use(program)
  setUniforms(obj)        // {name: number | [numbers] | WebGLTexture}
                          // silently ignores names absent from the program
  render()                // draws the fullscreen quad
  resize(cssW, cssH)      // backing store = css size * scale, rounded
  get fps()               // median of last 60 frame times, as fps
  setScale(s)             // live-adjustable, reallocates buffers
}
```

## Vertex shader (fixed, use verbatim)
```glsl
#version 300 es
in vec2 a_pos;
void main(){ gl_Position = vec4(a_pos, 0.0, 1.0); }
```
Quad = two triangles covering `[-1,1]²`, one static VBO, one VAO.

## Uniform lookup
Cache `getUniformLocation` per program in a `Map` at compile time by walking
`ACTIVE_UNIFORMS`. Do **not** call `getUniformLocation` per frame — it is a
sync call and will cost frames on the Pi.

Dispatch by the type reported in `getActiveUniform`: `FLOAT`, `FLOAT_VEC2/3/4`,
`INT`, `BOOL`, `SAMPLER_2D`. For `SAMPLER_2D`, assign texture units in
declaration order and bind.

## Non-negotiable robustness
- Context creation: request `{ alpha:false, antialias:false, depth:false,
  stencil:false, powerPreference:'high-performance',
  preserveDrawingBuffer:false }`. Antialias off is a real perf win on V3D.
- Handle `webglcontextlost` — `preventDefault()` and re-init on
  `webglcontextrestored`. A lost context mid-set must not be a black screen
  for the rest of the night.
- `compile()` returning `{ok:false}` is a normal outcome, not an exception.

## Observable
`gl-test.html` compiles a plasma shader, renders it fullscreen with a live fps
readout and a scale slider (0.25–1.0). On the **Pi**, confirm the fps at
scale 1.0 matches G-01's number for an equivalent workload within ~10%.
Paste both numbers into `../RESULTS.md`.

## Do not
Do not add a render loop scheduler here. The caller owns `requestAnimationFrame`.
