# G-07 — Compositor: Deck × Shader × Blend
**Model:** mid · **Blocked by:** G-03, G-04, G-05, G-06 · **Critical path**

## Deliverable
`gig/src/mixer.js` + `gig/index.html` (the actual show page).

This is the task where the four independently-tested pieces become a VJ engine.

## Layer model (keep it this simple — 4 days)
```
  output = blend( deckLayer, shaderLayer )
```
- **deckLayer**: Deck A texture × opacity (Deck B exists for pre-loading the
  next clip; A/B crossfade is a stretch goal, not a requirement)
- **shaderLayer**: one active ISF shader, fed all the G-05 audio uniforms
- **blend**: `normal | add | screen | multiply`, plus a shader opacity 0..1

Implement blending in the *fragment shader*, not with `gl.blendFunc`, so the
mixer stays one pass:
```glsl
vec3 mixLayers(vec3 base, vec3 top, float a, int mode){
  vec3 r = mode==1 ? base+top
         : mode==2 ? 1.0-(1.0-base)*(1.0-top)
         : mode==3 ? base*top
         : top;
  return mix(base, r, a);
}
```

## Frame loop (owns `requestAnimationFrame`)
```
1. audio.sample()
2. deckA.update(), deckB.update()
3. render active shader into FBO_shader
4. render mixer pass: deck texture + FBO_shader -> screen (or -> G-12's chain)
5. update fps / governor
```

## Adaptive quality governor — required, not optional
Track median frame time over the last 60 frames.
- median > 20 ms for 60 consecutive frames → drop `scale` one step
  (1.0 → 0.85 → 0.7 → 0.55 → 0.4, floor 0.4)
- median < 13 ms for 300 consecutive frames → raise one step
- Log every change to console. Never change scale more than once per second.

A dropped frame on a kick is more visible than a soft pixel on a 100" TV from
3 metres. The governor protects the beat, which is the thing the audience feels.

## Blackout
`mixer.blackout(bool)` — hard-clears to black in one pass, bypassing everything
including a shader that is failing to compile or a deck that stalled. This must
work even if every other subsystem is broken. Test it that way.

## Shader hot-swap
Switching shaders must not stall the frame loop. Compile the new program, and
only swap the active pointer once `compile()` returns ok. On failure keep the
current shader and log. **A bad shader must never black the output mid-set.**

## Observable
On the **Pi, on the TV**: a video clip playing with an audio-reactive shader
composited over it in add mode, driven by live booth audio, holding ≥55 fps
with the governor active. Record the settled scale value and fps in
`../RESULTS.md`. This is the gig's core deliverable — when this is DONE the
show exists and everything after it is polish.
