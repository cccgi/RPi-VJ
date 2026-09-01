// AUDIO_BUILD is printed on every UNI diag line. Bump it with any change
// to the audio path, so the log itself says which code is live instead of
// us inferring it from the numbers.
const AUDIO_BUILD = 'aud9-lockwindow6s';
// gig/src/mixer.js
// Compositor: Deck × Shader × Blend with Adaptive Governor (G-07)
// ES Module, zero external dependencies.

import { ShaderHost } from './gl.js';
import { parseISF } from './isf.js';
import { AudioEngine } from './audio.js';
import { Deck } from './deck.js';
import { Choreo } from './choreo.js';
import { PostProcessingChain } from './post.js';

// Per-pixel blend modes, modelled on Resolume's set. All of these are a few
// ALU ops per pixel -- negligible next to the shader itself, so there is no
// performance reason to keep the list short.
// Resolume's transition modes (wipes/dissolves) are deliberately NOT here:
// they are animated state, not a blend function, and belong in the mixer.
export const BLEND_MODES = {
  normal: 0,
  add: 1,          // Resolume's default
  screen: 2,
  multiply: 3,
  darken: 4,
  lighten: 5,
  difference: 6,
  exclusion: 7,
  overlay: 8,
  softlight: 9,
  hardlight: 10,
  dodge: 11,
  burn: 12,
  subtract: 13,
  lumakey: 14,     // top's luminance drives visibility
  lumaalpha: 15,   // luminance becomes alpha
  alphacut: 16,    // hard cut, no blending
  displace: 17     // top's rg warps the base -- ripples/warps
};

export const BLEND_NAMES = Object.keys(BLEND_MODES);

// 1x1 default solid black RGBA texture for inactive/empty deck inputs
function createBlackTexture(gl) {
  const tex = gl.createTexture();
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.texImage2D(
    gl.TEXTURE_2D,
    0,
    gl.RGBA,
    1,
    1,
    0,
    gl.RGBA,
    gl.UNSIGNED_BYTE,
    new Uint8Array([0, 0, 0, 255])
  );
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
  gl.bindTexture(gl.TEXTURE_2D, null);
  return tex;
}

// Compositor fragment shader: Deck × Shader × Blend mode (spec equation)
const MIXER_FRAG_SRC = `#version 300 es
precision highp float;
out vec4 fragColor;

uniform vec2 u_resolution;
uniform sampler2D u_deck_tex;
uniform sampler2D u_shader_tex;
uniform float u_deck_opacity;
uniform float u_shader_opacity;
uniform int u_blend_mode; // 0=normal, 1=add, 2=screen, 3=multiply
uniform int u_blackout;
uniform int u_invert_order; // 0 = Clip base / Shader top, 1 = Shader base / Clip top
uniform int u_deck_layout;  // 0 = 1:1 direct, 1 = mirror_hv_tl (quad unfold)
uniform int u_deck_flip_y;  // 1 = flip Y (default for DOM video textures)
uniform int u_fx;          // video effect applied to the CLIP layer
uniform float u_fx_amount; // 0..1 intensity
uniform float u_fx_time;
uniform int u_sfx;          // video effect applied to the SHADER layer
uniform float u_sfx_amount;

float luma(vec3 c) { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }
float fxhash(vec2 p){ return fract(sin(dot(p, vec2(127.1,311.7)))*43758.5453); }

vec3 hueShift(vec3 c, float h) {
  const vec3 k = vec3(0.57735);
  float ca = cos(h * 6.2831853);
  return c * ca + cross(k, c) * sin(h * 6.2831853) + k * dot(k, c) * (1.0 - ca);
}

// STAGE 1: geometry. Runs BEFORE the clip texture is sampled.
vec2 fxUV(vec2 uv, int fx, float a) {
  if (fx == 9)  { uv.x = abs(uv.x - 0.5) + 0.5; }                       // mirror H
  else if (fx == 10) { uv.y = abs(uv.y - 0.5) + 0.5; }                  // mirror V
  else if (fx == 11) { uv = abs(uv - 0.5) + 0.5; }                      // quad mirror
  else if (fx == 12) {                                                  // kaleidoscope
    vec2 p = uv - 0.5;
    float ang = atan(p.y, p.x), r = length(p);
    float seg = 6.2831853 / max(2.0, floor(3.0 + a * 9.0));
    ang = abs(mod(ang, seg) - seg * 0.5);
    uv = 0.5 + vec2(cos(ang), sin(ang)) * r;
  }
  else if (fx == 13) { uv = 0.5 + (uv - 0.5) / (1.0 + a * 1.5); }       // zoom in
  else if (fx == 14) { uv = 0.5 + (uv - 0.5) * (1.0 + a * 1.5); }       // zoom out
  else if (fx == 15) { uv.x += sin(uv.y * 18.0 + u_fx_time * 2.0) * 0.03 * a; } // wave
  else if (fx == 16) {                                                  // twirl
    vec2 p = uv - 0.5; float r = length(p);
    float ang = atan(p.y, p.x) + a * 3.0 * (0.35 - min(r, 0.35));
    uv = 0.5 + vec2(cos(ang), sin(ang)) * r;
  }
  else if (fx == 17) {                                                  // pixelate
    float n = mix(160.0, 12.0, a);
    uv = (floor(uv * n) + 0.5) / n;
  }
  return uv;
}

// STAGE 2: colour. Runs on the sampled clip.
vec3 fxColor(vec3 c, vec2 uv, int fx, float a) {
  if (fx == 1) return mix(c, 1.0 - c, a);                                // invert
  if (fx == 2) return hueShift(c, a);                                    // hue shift
  if (fx == 3) return mix(c, clamp((c - 0.5) * 1.8 + 0.5, 0.0, 1.0), a);  // saturate/contrast
  if (fx == 4) return mix(c, vec3(luma(c)), a);                          // black & white
  if (fx == 5) { float n = mix(24.0, 3.0, a); return floor(c * n) / n; }  // posterize
  if (fx == 6) return mix(c, step(vec3(0.5 - a * 0.2), c), a);           // threshold
  if (fx == 7) return mix(c, c * vec3(1.25, 0.95, 0.7), a);              // warm tint
  if (fx == 8) return mix(c, c * vec3(0.7, 0.95, 1.3), a);               // cool tint
  if (fx == 18) return c * (1.0 - a * 0.6 * step(0.5, fract(uv.y * u_resolution.y * 0.5))); // scanlines
  if (fx == 20) return mix(c, c * (1.0 - smoothstep(0.35, 0.85, length(uv - 0.5))), a);     // vignette
  if (fx == 21) return c + (fxhash(uv * u_resolution.xy + u_fx_time) - 0.5) * a * 0.35;     // grain
  if (fx == 22) {                                                        // halftone dots
    vec2 g = fract(uv * u_resolution.xy / mix(24.0, 6.0, a)) - 0.5;
    return mix(c, vec3(step(length(g), luma(c) * 0.55)), a);
  }
  if (fx == 23) return c * (1.0 - a * step(0.5, fract(u_fx_time * 8.0))); // strobe
  return c;
}

vec3 bOverlay(vec3 b, vec3 t) {
  return mix(2.0 * b * t, 1.0 - 2.0 * (1.0 - b) * (1.0 - t), step(0.5, b));
}
vec3 bSoftLight(vec3 b, vec3 t) {
  return mix(2.0 * b * t + b * b * (1.0 - 2.0 * t),
             sqrt(max(b, 0.0)) * (2.0 * t - 1.0) + 2.0 * b * (1.0 - t),
             step(0.5, t));
}
vec3 bDodge(vec3 b, vec3 t) { return min(vec3(1.0), b / max(1.0 - t, 0.0015)); }
vec3 bBurn(vec3 b, vec3 t)  { return 1.0 - min(vec3(1.0), (1.0 - b) / max(t, 0.0015)); }

vec3 mixLayers(vec3 base, vec3 top, float a, int mode) {
  vec3 r;
  float amt = a;
  if      (mode ==  1) r = base + top;
  else if (mode ==  2) r = 1.0 - (1.0 - base) * (1.0 - top);
  else if (mode ==  3) r = base * top;
  else if (mode ==  4) r = min(base, top);
  else if (mode ==  5) r = max(base, top);
  else if (mode ==  6) r = abs(base - top);
  else if (mode ==  7) r = base + top - 2.0 * base * top;
  else if (mode ==  8) r = bOverlay(base, top);
  else if (mode ==  9) r = bSoftLight(base, top);
  else if (mode == 10) r = bOverlay(top, base);            // hard light = overlay reversed
  else if (mode == 11) r = bDodge(base, top);
  else if (mode == 12) r = bBurn(base, top);
  else if (mode == 13) r = max(vec3(0.0), base - top);
  else if (mode == 14) { r = top; amt = a * luma(top); }   // luma key
  else if (mode == 15) { r = top * luma(top); }            // luma is alpha
  else if (mode == 16) { r = top; amt = a * step(0.5, luma(top)); }  // hard cut
  else                 r = top;
  return mix(base, clamp(r, 0.0, 1.0), clamp(amt, 0.0, 1.0));
}

void main() {
  if (u_blackout == 1) {
    fragColor = vec4(0.0, 0.0, 0.0, 1.0);
    return;
  }

  vec2 uv = gl_FragCoord.xy / u_resolution.xy;

  // The same effect bank, applied independently to the SHADER layer. Two
  // separate slots (u_fx for the clip, u_sfx here) so you can, say, kaleido
  // the footage while pixelating the generative layer over it.
  vec2 suv = (u_sfx > 0) ? fxUV(uv, u_sfx, u_sfx_amount) : uv;
  vec4 shaderSample = texture(u_shader_tex, clamp(suv, 0.0, 1.0));
  if (u_sfx == 19) {
    float so = u_sfx_amount * 0.02;
    shaderSample.r = texture(u_shader_tex, clamp(suv + vec2(so, 0.0), 0.0, 1.0)).r;
    shaderSample.b = texture(u_shader_tex, clamp(suv - vec2(so, 0.0), 0.0, 1.0)).b;
  } else if (u_sfx == 24) {
    vec2 spx = 1.0 / u_resolution.xy;
    float s0 = luma(texture(u_shader_tex, clamp(suv, 0.0, 1.0)).rgb);
    float sx = luma(texture(u_shader_tex, clamp(suv + vec2(spx.x, 0.0), 0.0, 1.0)).rgb);
    float sy = luma(texture(u_shader_tex, clamp(suv + vec2(0.0, spx.y), 0.0, 1.0)).rgb);
    float se = clamp(abs(s0 - sx) + abs(s0 - sy), 0.0, 1.0) * 6.0;
    shaderSample.rgb = mix(shaderSample.rgb, vec3(se), u_sfx_amount);
  }
  if (u_sfx > 0) shaderSample.rgb = clamp(fxColor(shaderSample.rgb, uv, u_sfx, u_sfx_amount), 0.0, 1.0);

  // Displace warps the base using the top layer's red/green, so it must offset
  // the lookup rather than blend colours.
  vec2 duv = uv;
  if (u_blend_mode == 17) {
    duv += (shaderSample.rg - 0.5) * 0.15 * clamp(u_shader_opacity, 0.0, 1.0);
  }
  // Video effects operate on the CLIP layer only, leaving the shader untouched.
  if (u_fx > 0) duv = fxUV(duv, u_fx, u_fx_amount);

  // DECK ASSET RECONSTRUCTION & ORIENTATION
  // 1. Quad Unfold (mirror_hv_tl layout): reconstructs full frame from 1/4 TL crop
  vec2 deckUV = duv;
  if (u_deck_layout == 1) {
    // Source is the original TOP-LEFT quadrant. X is reconstructed directly.
    // Y is reconstructed in pre-flip texture space because DOM video textures
    // receive the orientation correction immediately below. Using the X
    // formula for Y as well swaps the source's top and bottom halves.
    deckUV.x = 1.0 - abs(deckUV.x * 2.0 - 1.0);
    deckUV.y = abs(deckUV.y * 2.0 - 1.0);
  }
  // 2. DOM Video Texture Y-Flip correction (ensures right-side-up playback)
  if (u_deck_flip_y == 1) {
    deckUV.y = 1.0 - deckUV.y;
  }

  vec4 deckSample = texture(u_deck_tex, clamp(deckUV, 0.0, 1.0));

  if (u_fx == 19) {           // RGB shift needs its own taps
    float o = u_fx_amount * 0.02;
    deckSample.r = texture(u_deck_tex, clamp(deckUV + vec2(o, 0.0), 0.0, 1.0)).r;
    deckSample.b = texture(u_deck_tex, clamp(deckUV - vec2(o, 0.0), 0.0, 1.0)).b;
  } else if (u_fx == 24) {    // edge detect
    vec2 px = 1.0 / u_resolution.xy;
    float l0 = luma(texture(u_deck_tex, clamp(deckUV, 0.0, 1.0)).rgb);
    float lx = luma(texture(u_deck_tex, clamp(deckUV + vec2(px.x, 0.0), 0.0, 1.0)).rgb);
    float ly = luma(texture(u_deck_tex, clamp(deckUV + vec2(0.0, px.y), 0.0, 1.0)).rgb);
    float e = clamp(abs(l0 - lx) + abs(l0 - ly), 0.0, 1.0) * 6.0;
    deckSample.rgb = mix(deckSample.rgb, vec3(e), u_fx_amount);
  }
  if (u_fx > 0) deckSample.rgb = clamp(fxColor(deckSample.rgb, uv, u_fx, u_fx_amount), 0.0, 1.0);

  vec3 base = (u_invert_order == 1) ? shaderSample.rgb * clamp(u_shader_opacity, 0.0, 1.0)
                                     : deckSample.rgb * clamp(u_deck_opacity, 0.0, 1.0);
  vec3 top  = (u_invert_order == 1) ? deckSample.rgb * clamp(u_deck_opacity, 0.0, 1.0)
                                     : shaderSample.rgb;
  float a   = (u_invert_order == 1) ? clamp(u_deck_opacity, 0.0, 1.0)
                                     : clamp(u_shader_opacity, 0.0, 1.0);

  vec3 blended = (u_blend_mode == 17) ? mix(base, base + top * 0.35, a)
                                      : mixLayers(base, top, a, u_blend_mode);
  fragColor = vec4(blended, 1.0);
}
`;

// Shaders that miss 60fps at native (1080p) render scale even with zero
// composition on top of them -- see gig/probe/PERF.md / shaderbench.log.
// The mixer's only scaling mechanism is the reactive global governor below
// (canvas-wide, steps once/sec based on measured frame time); it has no
// per-shader FBO resolution table. Left alone, switching live into one of
// these means ~4 seconds of visible stutter on the TV while the governor
// steps down one notch/second from 1.0 to whatever actually fits. Loading
// one of these jumps straight to its known-safe scale instead. Keyed by
// the shader's basename (no path, no .fs).
const SHADER_SCALE_HINTS = {
  vs_sacred_polyhedron: 0.4,      // bench: 12.0fps@1080p -- needs 540p to hit 59.9
  vs_shattered_glitch_core: 0.4,  // bench: 15.0fps@1080p -- needs 540p to hit 59.9
  strata_drift: 0.7,              // bench: 20.0fps@1080p -- needs 720p to hit 59.9
  particle_warp: 0.7,             // bench: fits 1080p alone but only 1.0x headroom
                                   // (uses the whole frame budget; none left for a
                                   // clip underneath or the post chain)
};

export class Mixer {
  /**
   * @param {HTMLCanvasElement} canvas Target output canvas element
   * @param {Object} [options]
   * @param {number} [options.scale=1.0] Initial render scale
   * @param {boolean} [options.governor=true] Enable adaptive governor
   */
  constructor(canvas, options = {}) {
    if (!canvas) {
      throw new Error('Mixer requires a canvas element');
    }

    this.canvas = canvas;
    this.initialScale = options.scale !== undefined ? options.scale : 1.0;
    this.governorEnabled = options.governor !== false;

    // Scale stepping table per spec: 1.0 -> 0.85 -> 0.7 -> 0.55 -> 0.4 (floor 0.4)
    this.scaleSteps = [1.0, 0.85, 0.7, 0.55, 0.4];
    this.currentScaleIndex = 0;
    // Find closest initial scale index
    const closestIdx = this.scaleSteps.findIndex(s => Math.abs(s - this.initialScale) < 0.01);
    if (closestIdx >= 0) {
      this.currentScaleIndex = closestIdx;
    }

    // Initialize ShaderHost
    this.host = new ShaderHost(this.canvas, { scale: this.scaleSteps[this.currentScaleIndex] });
    const gl = this.host.gl;
    if (!gl) {
      throw new Error('Mixer failed to initialize WebGL2 ShaderHost');
    }
    this.gl = gl;

    // Subsystems
    this.audio = new AudioEngine();
    this.choreo = new Choreo({ clock: this.audio.clock });
    this.deckA = new Deck(this.gl, 'A');
    this.deckB = new Deck(this.gl, 'B');

    // Solid black fallback texture
    this._blackTexture = createBlackTexture(this.gl);

    // State
    this._isBlackout = false;
    this._fx = 0;
    this._fxAmount = 1.0;
    this._sfx = 0;
    this._sfxAmount = 1.0;
    this._deckOpacity = 1.0;
    this._shaderOpacity = 1.0;
    this._blendMode = BLEND_MODES.normal; // 0=normal, 1=add, 2=screen, 3=multiply

    // Shader layer compilation & hot-swap state
    this._activeProgram = null;
    this._activeShaderSrc = '';
    this._shaderUniformDefaults = {};

    // Shader FBO and Texture
    this._fbo = null;
    this._fboTexture = null;
    this._fboWidth = 0;
    this._fboHeight = 0;

    // Mixer composition program
    const mixComp = this.host.compile(MIXER_FRAG_SRC);
    if (!mixComp.ok) {
      throw new Error(`Mixer composite shader failed to compile: ${mixComp.log}`);
    }
    this._mixerProgram = mixComp.program;

    // Adaptive Quality Governor State
    this._frameHistory = []; // Rolling window of frame time deltas (ms)
    this._lastFrameTime = 0;
    this._highFramesCount = 0; // Consecutive frames with median > 20ms
    this._lowFramesCount = 0;  // Consecutive frames with median < 13ms
    this._lastScaleChangeTime = 0; // Timestamp in ms (rate limit <= 1 change per sec)

    // Animation frame loop
    this._rafId = null;
    this._running = false;
    this._startTime = performance.now();
    this._onFrame = this._onFrame.bind(this);

    // G-12 post chain (feedback/trails -> bloom -> CA/grain/vignette finish).
    // Constructing it here compiles all four of its programs and allocates
    // its FBOs immediately, on real hardware, at boot -- so a broken post
    // chain shows up as a boot-time error instead of silent dead code (it
    // was previously never imported anywhere: written, never wired in, so
    // "never verified" in HANDOFF item 4 was literal). Disabled by default:
    // its per-frame cost has not been benched yet, and every shader's
    // headroom budget (PERF.md) was measured assuming no post chain running.
    // Flip on with setPostEnabled(true) once benched. RGBA8/UNSIGNED_BYTE
    // throughout (post.js's shared createTexture()) -- OES_texture_float_linear
    // is not available on this Pi, so this must never grow a float FBO.
    // NOT constructed here. The constructor runs _initQuad, _initPrograms
    // (four compiled programs) and _initFBOs -- full-resolution feedback
    // textures, quarter-res bloom textures, the lot -- and it was doing all of
    // that at boot while postEnabled stayed false. That is several full-size
    // RGBA8 allocations plus four programs held for a feature that never ran.
    // On a 2GB Pi that is worth reclaiming. Built on demand instead.
    this.post = null;
    this.postEnabled = false;
    this._compositeFBO = null;
    this._compositeTexture = null;
    this._compositeWidth = 0;
    this._compositeHeight = 0;

    // Initial FBO allocation
    this._updateFBO();
    this._updateCompositeFBO();
  }

  /**
   * Enable/disable the G-12 post chain. When off (default) Step 4 renders
   * straight to the screen exactly as before this existed -- zero behaviour
   * change. When on, Step 4 renders into an internal composite FBO and the
   * post chain (post.js) reads that as its input, writing the final frame.
   * @param {boolean} on
   */
  setPostEnabled(on) {
    on = !!on;
    if (on && !this.post) {
      this.post = new PostProcessingChain(this.gl, this.canvas.width, this.canvas.height);
      this._updateCompositeFBO();
    }
    if (!on && this.post) {
      try { this.post.destroy(); } catch (_) {}
      this.post = null;
    }
    this.postEnabled = on;
    if (typeof window !== 'undefined' && window.VJ_DIAG)
      window.VJ_DIAG('post chain ' + (on ? 'ENABLED (allocated)' : 'DISABLED (freed)'));
  }

  // Ensures the composite FBO (Step 4's target when the post chain is on)
  // matches the current scaled backing store, and keeps the post chain's
  // own FBOs in step with it. Mirrors _updateFBO()'s pattern exactly.
  _updateCompositeFBO() {
    const gl = this.gl;
    if (!gl) return;

    const w = this.canvas.width;
    const h = this.canvas.height;

    if (this.post) this.post.resize(w, h);

    if (this._compositeFBO && this._compositeWidth === w && this._compositeHeight === h) {
      return;
    }

    this._compositeWidth = w;
    this._compositeHeight = h;

    if (!this._compositeFBO) {
      this._compositeFBO = gl.createFramebuffer();
    }
    if (!this._compositeTexture) {
      this._compositeTexture = gl.createTexture();
    }

    gl.bindTexture(gl.TEXTURE_2D, this._compositeTexture);
    // RGBA8 -- see the constructor comment on this.post.
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);

    gl.bindFramebuffer(gl.FRAMEBUFFER, this._compositeFBO);
    gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, this._compositeTexture, 0);

    const status = gl.checkFramebufferStatus(gl.FRAMEBUFFER);
    if (status !== gl.FRAMEBUFFER_COMPLETE) {
      console.warn(`[Mixer] Composite FBO incomplete: 0x${status.toString(16)}`);
    }

    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  /**
   * Current settled render scale.
   * @returns {number}
   */
  get scale() {
    return this.scaleSteps[this.currentScaleIndex];
  }

  /**
   * Current rolling median FPS.
   * @returns {number}
   */
  get fps() {
    return this.host.fps;
  }

  /**
   * Start the mixer frame loop.
   */
  start() {
    if (this._running) return;
    this._running = true;
    this._lastFrameTime = performance.now();
    this._rafId = requestAnimationFrame(this._onFrame);
  }

  /**
   * Stop the mixer frame loop.
   */
  stop() {
    this._running = false;
    if (this._rafId !== null) {
      cancelAnimationFrame(this._rafId);
      this._rafId = null;
    }
  }

  /**
   * Hard blackout switch. Hard-clears to black in one pass, bypassing everything
   * including broken/stalled decks or shaders.
   * @param {boolean} bool
   */
  blackout(bool) {
    this._isBlackout = Boolean(bool);
  }

  /**
   * Set active blend mode: 'normal' | 'add' | 'screen' | 'multiply' (or 0..3)
   * @param {string|number} mode
   */
  setBlendMode(mode) {
    if (typeof mode === 'string') {
      const lower = mode.toLowerCase();
      if (BLEND_MODES[lower] !== undefined) {
        this._blendMode = BLEND_MODES[lower];
      }
    } else if (typeof mode === 'number' && mode >= 0 && mode <= 3) {
      this._blendMode = Math.floor(mode);
    }
  }

  /**
   * Set shader layer opacity (0..1)
   * @param {number} opacity
   */
  setShaderOpacity(opacity) {
    this._shaderOpacity = Math.max(0, Math.min(1, opacity));
  }

  /**
   * Set deck layer opacity (0..1)
   * @param {number} opacity
   */
  setDeckOpacity(opacity) {
    this._deckOpacity = Math.max(0, Math.min(1, opacity));
  }

  /**
   * Flip/invert layer order between Clip on top vs Shader on top.
   * @param {boolean} invert If true, Shader is bottom and Clip is top.
   */
  setInvertLayerOrder(invert) {
    this._invertOrder = Boolean(invert);
  }

  /**
   * Set deck asset layout reconstruction mode.
   * 0 = normal 1:1 direct (default)
   * 1 = mirror_hv_tl (quad unfold 1/4 top-left crop to full screen)
   * @param {string|number} layout
   */
  setDeckLayout(layout) {
    if (layout === 'mirror_hv_tl' || layout === 1 || layout === 'quad') {
      this._deckLayout = 1;
    } else {
      this._deckLayout = 0;
    }
  }

  get deckLayout() {
    return this._deckLayout || 0;
  }

  toggleLayerOrder() {
    this._invertOrder = !this._invertOrder;
    return this._invertOrder;
  }

  get isInvertLayerOrder() {
    return Boolean(this._invertOrder);
  }

  /**
   * Hot-swap active ISF fragment shader without stalling the frame loop.
   * Compiles first; on success swaps pointer. On failure keeps current shader and logs.
   * A bad shader never blacks out the output.
   * @param {string} isfSource
   * @returns {boolean} True if successfully compiled and swapped
   */
  /**
   * Drop the active shader. u_shader_opacity is already forced to 0 when
   * _activeProgram is null, so the layer contributes nothing.
   */
  /** Video effect applied to the CLIP layer. 0 = none. */
  setVideoFx(id, amount) {
    this._fx = Math.max(0, Math.min(24, id | 0));
    if (amount !== undefined) this._fxAmount = Math.max(0, Math.min(1, amount));
    return this._fx;
  }
  setVideoFxAmount(a) { this._fxAmount = Math.max(0, Math.min(1, a)); }

  /** Audio response exponent. 1.0 = raw, higher = more gradual at low levels. */
  setResponseCurve(e) { this._respCurve = Math.max(1.0, Math.min(3.0, Number(e) || 1.8)); return this._respCurve; }
  getResponseCurve() { return this._respCurve === undefined ? 1.8 : this._respCurve; }

  /** Same effect bank, applied to the SHADER layer instead of the clip. */
  setShaderFx(id, amount) {
    this._sfx = Math.max(0, Math.min(24, id | 0));
    if (amount !== undefined) this._sfxAmount = Math.max(0, Math.min(1, amount));
    return this._sfx;
  }
  setShaderFxAmount(a) { this._sfxAmount = Math.max(0, Math.min(1, a)); }
  get shaderFx() { return this._sfx || 0; }
  get videoFx() { return this._fx || 0; }

  clearShader() {
    // Do NOT deleteProgram here: the program belongs to the cache and will be
    // reused when this shader is selected again. Dropping the reference is
    // all that is wanted.
    this._activeProgram = null;
    this._activeShaderSrc = null;
    this._shaderUniformDefaults = {};
    this._activeShaderName = '';
    this._activePreset = -1;
  }

  /** Base name of the running shader, e.g. 'beat_tunnel'. '' if none. */
  getActiveShaderName() {
    return this._activeShaderName || '';
  }

  /**
   * Merge input values into the running shader's uniforms.
   *
   * These live in the same object the ISF defaults were parsed into, which is
   * spread into every frame's uniform block -- so a preset is just a different
   * set of defaults and survives until the shader is swapped.
   * Unknown names are ignored: the shader simply has no such uniform.
   */
  setShaderInputs(values) {
    if (!values || !this._activeProgram) return false;
    for (const k of Object.keys(values)) {
      this._shaderUniformDefaults[k] = values[k];
    }
    return true;
  }

  getShaderInputs() {
    return { ...this._shaderUniformDefaults };
  }

  loadShader(isfSource, shaderName) {
    try {
      // PROGRAM CACHE.
      //
      // Every press used to compile a fresh GL program and simply overwrite
      // _activeProgram; the old one was never passed to deleteProgram(), and
      // clearShader() only nulled the reference. WebGL programs are reclaimed
      // when their JS wrapper is collected, but the wrapper is a tiny object
      // holding a large driver allocation -- exactly the case GC is worst at.
      // With the JS heap sitting at 2-3MB there is almost no pressure to
      // collect, so compiled programs, uniform metadata and V3D shader
      // binaries pile up across a set.
      //
      // Caching by shader name fixes the accumulation AND removes the repeated
      // GLSL compile on every press, which is not cheap on this GPU. The cache
      // is bounded by the number of shaders in the show (18), so it cannot
      // grow without limit.
      if (!this._programCache) this._programCache = new Map();
      const cacheKey = shaderName
        ? String(shaderName).replace(/^.*\//, '').replace(/\.fs$/, '')
        : null;

      const cached = cacheKey ? this._programCache.get(cacheKey) : null;
      if (cached && cached.program) {
        this._activeProgram = cached.program;
        this._activeShaderSrc = isfSource;
        this._shaderUniformDefaults = { ...(cached.defaults || {}) };
        this._activeShaderName = cacheKey;
        this._activePreset = -1;
        const hint = SHADER_SCALE_HINTS[cacheKey];
        if (hint !== undefined) this.setScale(hint);
        return true;
      }

      const parsed = parseISF(isfSource);
      const res = this.host.compile(parsed.glsl);
      if (!res.ok) {
        // With no monitor, console.error is invisible. Route it to the diag log.
        if (typeof window !== 'undefined' && window.VJ_DIAG)
          window.VJ_DIAG('SHADER COMPILE FAIL: ' + String(res.log || '').split('\n')[0]);
        console.error('[Mixer] Shader compilation failed, keeping previous shader:', res.log);
        return false;
      }

      this._activeProgram = res.program;
      this._activeShaderSrc = isfSource;
      if (cacheKey) {
        this._programCache.set(cacheKey, { program: res.program, defaults: null });
      }

      // Cache default input values
      const defaults = {};
      for (const d of parsed.defaults) {
        defaults[d.name] = d.val;
      }
      this._shaderUniformDefaults = defaults;
      if (cacheKey && this._programCache.has(cacheKey)) {
        this._programCache.get(cacheKey).defaults = { ...defaults };
      }
      // Remember the shader's base name so the preset system can look it up,
      // and reset the preset slot: a new shader starts on its own defaults.
      this._activeShaderName = shaderName
        ? String(shaderName).replace(/^.*\//, '').replace(/\.fs$/, '')
        : '';
      this._activePreset = -1;

      if (shaderName && this.governorEnabled) {
        const base = this._activeShaderName;
        const hint = SHADER_SCALE_HINTS[base];
        if (hint !== undefined) this.setScale(hint);
      }

      return true;
    } catch (err) {
      if (typeof window !== 'undefined' && window.VJ_DIAG)
        window.VJ_DIAG('SHADER PARSE FAIL: ' + (err && err.message));
      console.error('[Mixer] Shader parse error, keeping previous shader:', err);
      return false;
    }
  }

  /**
   * Resize mixer viewport and FBO.
   * @param {number} cssW
   * @param {number} cssH
   */
  resize(cssW, cssH) {
    this.host.resize(cssW, cssH);
    this._updateFBO();
    this._updateCompositeFBO();
  }

  /**
   * Explicitly set render scale and adjust FBO and host.
   * @param {number} newScale
   */
  setScale(newScale) {
    const closestIdx = this.scaleSteps.reduce((prev, curr, idx) => {
      return Math.abs(curr - newScale) < Math.abs(this.scaleSteps[prev] - newScale) ? idx : prev;
    }, 0);
    this._setScaleIndex(closestIdx);
  }

  _setScaleIndex(index) {
    const clamped = Math.max(0, Math.min(this.scaleSteps.length - 1, index));
    if (clamped !== this.currentScaleIndex) {
      this.currentScaleIndex = clamped;
      const s = this.scaleSteps[this.currentScaleIndex];
      this.host.setScale(s);
      this._updateFBO();
      this._updateCompositeFBO();
      this._lastScaleChangeTime = performance.now();
      console.log(`[Mixer Governor] Render scale adjusted to ${s.toFixed(2)} (median fps: ${this.host.fps.toFixed(1)})`);
    }
  }

  // Ensures FBO matches current scaled backing store dimensions
  _updateFBO() {
    const gl = this.gl;
    if (!gl) return;

    const w = this.canvas.width;
    const h = this.canvas.height;

    if (this._fbo && this._fboWidth === w && this._fboHeight === h) {
      return;
    }

    this._fboWidth = w;
    this._fboHeight = h;

    if (!this._fbo) {
      this._fbo = gl.createFramebuffer();
    }
    if (!this._fboTexture) {
      this._fboTexture = gl.createTexture();
    }

    gl.bindTexture(gl.TEXTURE_2D, this._fboTexture);
    // Use RGBA8 (color renderable and filterable on Pi 5 / Mesa)
    gl.texImage2D(
      gl.TEXTURE_2D,
      0,
      gl.RGBA,
      w,
      h,
      0,
      gl.RGBA,
      gl.UNSIGNED_BYTE,
      null
    );
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);

    gl.bindFramebuffer(gl.FRAMEBUFFER, this._fbo);
    gl.framebufferTexture2D(
      gl.FRAMEBUFFER,
      gl.COLOR_ATTACHMENT0,
      gl.TEXTURE_2D,
      this._fboTexture,
      0
    );

    const status = gl.checkFramebufferStatus(gl.FRAMEBUFFER);
    if (status !== gl.FRAMEBUFFER_COMPLETE) {
      console.warn(`[Mixer] Shader FBO incomplete: 0x${status.toString(16)}`);
    }

    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    gl.bindTexture(gl.TEXTURE_2D, null);
  }

  /**
   * Adaptive quality governor step.
   * Tracks median frame time over the last 60 frames:
   * - median > 20ms for 60 consecutive frames -> drop scale one step (floor 0.4)
   * - median < 13ms for 300 consecutive frames -> raise scale one step (ceil 1.0)
   * - Log every change to console. Never change scale more than once per second.
   */
  _updateGovernor(deltaMs, now) {
    if (!this.governorEnabled) return;

    this._frameHistory.push(deltaMs);
    if (this._frameHistory.length > 60) {
      this._frameHistory.shift();
    }

    if (this._frameHistory.length < 60) {
      return;
    }

    // Compute rolling median of last 60 frames
    const sorted = [...this._frameHistory].sort((a, b) => a - b);
    const mid = Math.floor(sorted.length / 2);
    const medianMs = sorted.length % 2 === 0 ? (sorted[mid - 1] + sorted[mid]) / 2 : sorted[mid];

    if (medianMs > 20.0) {
      this._highFramesCount++;
      this._lowFramesCount = 0;
    } else if (medianMs < 13.0) {
      this._lowFramesCount++;
      this._highFramesCount = 0;
    } else {
      this._highFramesCount = 0;
      this._lowFramesCount = 0;
    }

    const timeSinceLastChange = now - this._lastScaleChangeTime;
    const canChange = timeSinceLastChange >= 1000; // Never change scale more than once per second

    if (canChange) {
      if (this._highFramesCount >= 60) {
        // Drop scale one step
        if (this.currentScaleIndex < this.scaleSteps.length - 1) {
          this._setScaleIndex(this.currentScaleIndex + 1);
        }
        this._highFramesCount = 0;
      } else if (this._lowFramesCount >= 300) {
        // Raise scale one step
        if (this.currentScaleIndex > 0) {
          this._setScaleIndex(this.currentScaleIndex - 1);
        }
        this._lowFramesCount = 0;
      }
    }
  }

  /**
   * Main frame loop step.
   */
  _onFrame(now) {
    if (!this._running) return;

    const deltaMs = this._lastFrameTime > 0 ? (now - this._lastFrameTime) : 16.67;
    this._lastFrameTime = now;
    const markFramePresented = () => {
      const t = (typeof performance !== 'undefined' && performance.now) ? performance.now() : now;
      this.lastFrameAt = t;
      if (typeof window !== 'undefined') window.lastFrameAt = t;
    };

    // Fast-path blackout execution: clear canvas to black in one pass and bypass everything
    if (this._isBlackout) {
      const gl = this.gl;
      gl.bindFramebuffer(gl.FRAMEBUFFER, null);
      gl.viewport(0, 0, this.canvas.width, this.canvas.height);
      gl.clearColor(0.0, 0.0, 0.0, 1.0);
      gl.clear(gl.COLOR_BUFFER_BIT);
      markFramePresented();

      // Still update governor / fps tracking
      this._updateGovernor(deltaMs, now);
      this._rafId = requestAnimationFrame(this._onFrame);
      return;
    }

    // Step 1: Audio sample
    let audioData = null;
    try {
      audioData = this.audio.sample();
    } catch (_) {}
    this._lastAudioData = audioData;

    // Fallback clock/synth if audio engine is not ok or absent
    const timeSec = (now - this._startTime) * 0.001;
    // RESPONSE CURVE. Shaders multiply band values by large factors, so a band
    // at 0.3 already saturates them -- which is why the visuals "came out of
    // the gate swinging" a few percent above mute and then barely changed.
    // Raising the value to a power >1 pushes low levels down, spreading the
    // musical range across the whole fader instead of the bottom of it.
    const RESP = this._respCurve === undefined ? 1.8 : this._respCurve;
    const curve = (v) => (v > 0 ? Math.pow(v, RESP) : 0);

    const synthBass = curve(audioData?.ok ? audioData.bass : 0.2 + 0.15 * Math.sin(timeSec * 4.0));
    const synthMid = curve(audioData?.ok ? audioData.lowMid : 0.2 + 0.15 * Math.sin(timeSec * 2.5 + 1.0));
    const synthTreble = curve(audioData?.ok ? audioData.treble : 0.2 + 0.15 * Math.cos(timeSec * 5.0));
    const synthOnset = audioData?.ok ? audioData.kick : 0.0;
    const synthVolume = curve(audioData?.ok ? audioData.energy : 0.3);
    const synthBeatPhase = (timeSec * 2.0) % 1.0;

    // ------------------------------------------------------------------
    // AUDIO-INTEGRATED FLOW CLOCKS
    //
    // Shaders drive their motion from u_time, so audio could only ever
    // change how a thing LOOKS (brightness, size, colour) -- never how fast
    // it MOVES. Scaling u_time directly is not an option: changing the
    // multiplier makes the whole animation jump.
    //
    // The fix is to integrate speed here instead. u_flow is a monotonically
    // increasing clock whose RATE follows the music, so a shader that swaps
    // u_time for u_flow speeds up and slows down with the track and never
    // discontinuities. Same per band, plus one accumulator per spectrum bin
    // so a shader can give every element its own independently-racing clock.
    // ------------------------------------------------------------------
    const dtSec = Math.max(0.0001, Math.min(0.1, deltaMs * 0.001));
    if (this._flow === undefined) {
      this._flow = 0; this._flowLow = 0; this._flowMid = 0; this._flowHigh = 0;
      this._specFlow = new Float32Array(32);
    }
    const aEnergy = synthVolume;
    const aKick   = synthOnset;
    const hitLow  = audioData?.ok ? audioData.hitLow  : 0.0;
    const hitMid  = audioData?.ok ? audioData.hitMid  : 0.0;
    const hitHigh = audioData?.ok ? audioData.hitHigh : 0.0;

    // Small idle term keeps things breathing in silence; the audio terms
    // dominate hard once the track is playing.
    this._flow     += dtSec * (0.10 + aEnergy * 2.6 + aKick * 3.0);
    this._flowLow  += dtSec * (0.08 + synthBass * 3.2 + hitLow * 6.0);
    this._flowMid  += dtSec * (0.10 + synthMid * 3.0 + hitMid * 5.0);
    this._flowHigh += dtSec * (0.12 + synthTreble * 3.4 + hitHigh * 6.0);

    // Per-bin flow: each spectrum bin advances only while that bin is loud,
    // so an element bound to bin N lurches on ITS OWN transients instead of
    // every element racing together off one global clock.
    {
      const sp = (audioData && audioData.spectrum) ? audioData.spectrum : null;
      for (let i = 0; i < 32; i++) {
        const v = sp ? sp[i] : 0.0;
        const cv = v > 0 ? Math.pow(v, RESP) : 0;
        this._specFlow[i] += dtSec * (0.05 + cv * cv * 9.0);
      }
    }

    // Step 2: Deck updates
    try {
      this.deckA.update();
    } catch (_) {}
    try {
      this.deckB.update();
    } catch (_) {}

    const gl = this.gl;
    const cw = this.canvas.width;
    const ch = this.canvas.height;

    // Step 3: Render active shader into FBO_shader
    if (this._fbo && this._activeProgram) {
      gl.bindFramebuffer(gl.FRAMEBUFFER, this._fbo);
      gl.viewport(0, 0, cw, ch);
      gl.clearColor(0.0, 0.0, 0.0, 1.0);
      gl.clear(gl.COLOR_BUFFER_BIT);

      this.host.use(this._activeProgram);

      // Supply default ISF input uniforms + standard audio uniforms
      const baseUniforms = {
        ...this._shaderUniformDefaults,
        u_time: timeSec,
        TIME: timeSec,
        u_resolution: [cw, ch],
        RENDERSIZE: [cw, ch],
        u_bass: synthBass,
        u_mid: synthMid,
        u_treble: synthTreble,
        u_onset: synthOnset,
        u_volume: synthVolume,
        // ALIASES: several shaders declare u_energy / u_kick rather than
        // u_volume / u_onset. Unsupplied uniforms default to 0, so those
        // shaders looked "not audio-reactive" while actually being fed silence.
        u_energy: synthVolume,
        u_kick: synthOnset,
        // All five bands, so a shader can give each visual layer its own band
        // instead of every element following the kick.
        u_sub: audioData?.ok ? curve(audioData.subBass) : synthBass * 0.9,
        u_low_mid: audioData?.ok ? curve(audioData.lowMid) : synthMid,
        u_high_mid: audioData?.ok ? curve(audioData.highMid) : synthTreble,
        // Independent transients: kick / snare-clap / hats.
        u_spectrum: (audioData && audioData.spectrum) ? audioData.spectrum : new Float32Array(32),
        u_hit_low: audioData?.ok ? audioData.hitLow : synthOnset,
        u_hit_mid: audioData?.ok ? audioData.hitMid : 0.0,
        u_hit_high: audioData?.ok ? audioData.hitHigh : 0.0,
        u_beat_phase: audioData ? audioData.beatPhase : synthBeatPhase,
        u_bar_phase: audioData ? audioData.barPhase : (synthBeatPhase / 4.0),
        // 1 while the tempo clock is phase-locked to real onsets, 0 when
        // it is free-running. Shaders blend beat-driven motion out when low.
        u_locked: audioData && audioData.locked !== undefined ? audioData.locked : 0.0,
        // Audio-integrated clocks -- use these instead of u_time for anything
        // whose SPEED should follow the music.
        u_flow: this._flow,
        u_flow_low: this._flowLow,
        u_flow_mid: this._flowMid,
        u_flow_high: this._flowHigh,
        u_spec_flow: this._specFlow,
        uBass: synthBass,
        uMid: synthMid,
        uTreble: synthTreble,
        uKick: synthOnset,
        uEnergy: synthVolume,
        uSubBass: audioData?.subBass || 0.0,
        uLowMid: audioData?.lowMid || synthMid,
        uHighMid: audioData?.highMid || synthTreble,
        uBeatPhase: audioData ? audioData.beatPhase : synthBeatPhase,
        uBarPhase: audioData ? audioData.barPhase : (synthBeatPhase / 4.0),
        uBPM: audioData?.bpm || 120.0
      };

      // Modulate uniforms via Choreo matrix
      const modulatedUniforms = this.choreo.apply(baseUniforms, audioData, deltaMs * 0.001);

      const U = modulatedUniforms;
      // ---- UNIFORM DUMP ---------------------------------------------------
      // The bench feeds synthetic values (u_bass sweeping 0..1, u_volume 0.6,
      // u_onset as k^3) and the shaders look great on it. If the same shader
      // is flat in performance, the shaders are not the variable -- the values
      // are. This prints exactly what the shader is handed, so we compare
      // numbers instead of impressions. Runs during the same window as the
      // audio measure() button.
      // ALWAYS ON, every 2s. This was gated behind the MEASURE button, which
      // needs a mouse or a keyboard -- neither of which exists at the gig, and
      // pressing M did nothing because the kiosk window had no focus. The diag
      // log already carries an fps line every 5s; one more line every 2s costs
      // nothing and means the audio path can be inspected at any time.
      {
        // PEAK-HOLD between dumps. onset decays in 100ms and the hit envelopes
        // in 90ms, but the dump samples every 2s -- so a detector can be firing
        // on every beat and still log 0.000 about 95% of the time. Sampling an
        // instantaneous transient on a 2s clock measures nothing. Track the max
        // since the last line instead.
        if (this._pk === undefined) this._pk = { on: 0, hl: 0, hm: 0, hh: 0, tr: 0, hi: 0 };
        const _n = (x) => (typeof x === 'number' ? x : 0);
        this._pk.on = Math.max(this._pk.on, _n(U.u_onset));
        this._pk.hl = Math.max(this._pk.hl, _n(U.u_hit_low));
        this._pk.hm = Math.max(this._pk.hm, _n(U.u_hit_mid));
        this._pk.hh = Math.max(this._pk.hh, _n(U.u_hit_high));
        this._pk.tr = Math.max(this._pk.tr, _n(U.u_treble));
        this._pk.hi = Math.max(this._pk.hi, _n(U.u_high_mid));

        if (!this._uniNext || timeSec > this._uniNext) {
          this._uniNext = timeSec + 2.0;
          const f = (x) => (typeof x === 'number' ? x.toFixed(3) : '?');
          const dbg = (this.audio && this.audio._dbg) || {};
          const rawDbg = ['sub', 'bass', 'lowMid', 'highMid', 'treble']
            .map((k) => {
              const x = dbg[k];
              return k + '=' + (x ? x.raw.toFixed(3) + ':' + (x.why || 'ok') : '?');
            }).join(' ');
          const sp = U.u_spectrum || [];
          let spMax = 0, spSum = 0, spNonZero = 0;
          for (let i = 0; i < 32; i++) {
            const v = sp[i] || 0;
            if (v > spMax) spMax = v;
            spSum += v;
            if (v > 0.01) spNonZero++;
          }
          if (typeof window !== 'undefined' && window.VJ_DIAG) {
            window.VJ_DIAG(
              'UNI bass=' + f(U.u_bass) +
              ' mid=' + f(U.u_mid) +
              ' treb=' + f(U.u_treble) +
              ' vol=' + f(U.u_volume) +
              ' onset=' + f(U.u_onset) +
              ' | sub=' + f(U.u_sub) +
              ' lowmid=' + f(U.u_low_mid) +
              ' himid=' + f(U.u_high_mid) +
              ' | hit L/M/H=' + f(U.u_hit_low) + '/' +
                f(U.u_hit_mid) + '/' + f(U.u_hit_high) +
              ' | spec max=' + spMax.toFixed(3) + ' avg=' + (spSum / 32).toFixed(3) +
                ' live=' + spNonZero + '/32' +
              ' | flow=' + f(this._flow) + ' low=' + f(this._flowLow) +
              ' | locked=' + f(U.u_locked) +
              ' | build=' + AUDIO_BUILD +
              // peak-since-last-line: what the shaders actually saw
              ' || PEAK onset=' + this._pk.on.toFixed(2) +
              ' hit=' + this._pk.hl.toFixed(2) + '/' + this._pk.hm.toFixed(2) +
                '/' + this._pk.hh.toFixed(2) +
              ' treb=' + this._pk.tr.toFixed(3) +
              ' himid=' + this._pk.hi.toFixed(3) +
              // raw pre-gate readings, so a dead band can be told apart from a
              // band with genuinely no content at the input
              ' || RAW ' + rawDbg
            );
          }
          this._pk = { on: 0, hl: 0, hm: 0, hh: 0, tr: 0, hi: 0 };
        }
      }

      this.host.setUniforms(modulatedUniforms);
      this.host.render();
    } else if (this._fbo) {
      // Clear shader FBO to black if no active shader
      gl.bindFramebuffer(gl.FRAMEBUFFER, this._fbo);
      gl.viewport(0, 0, cw, ch);
      gl.clearColor(0.0, 0.0, 0.0, 1.0);
      gl.clear(gl.COLOR_BUFFER_BIT);
    }

    // Step 4: Render mixer pass: deck texture + FBO_shader -> composite
    // (post chain on) or straight to screen (post chain off, unchanged
    // behaviour from before G-12 was wired in).
    gl.bindFramebuffer(gl.FRAMEBUFFER, this.postEnabled ? this._compositeFBO : null);
    gl.viewport(0, 0, cw, ch);

    this.host.use(this._mixerProgram);

    // Deck texture resolution: use deckA if ready, else black fallback texture
    const hasDeckTex = this.deckA.ready && this.deckA.texture;
    const deckTexture = hasDeckTex ? this.deckA.texture : this._blackTexture;
    const shaderTexture = this._fboTexture || this._blackTexture;

    this.host.setUniforms({
      u_resolution: [cw, ch],
      u_deck_tex: deckTexture,
      u_shader_tex: shaderTexture,
      u_deck_opacity: hasDeckTex ? this._deckOpacity : 0.0,
      u_shader_opacity: this._activeProgram ? this._shaderOpacity : 0.0,
      u_blend_mode: this._blendMode,
      u_invert_order: this._invertOrder ? 1 : 0,
      u_deck_layout: this._deckLayout || 0,
      u_deck_flip_y: 1, // Fixes DOM video texture orientation
      u_fx: this._fx || 0,
      u_fx_amount: (this._fxAmount === undefined ? 1.0 : this._fxAmount),
      u_fx_time: timeSec,
      u_sfx: this._sfx || 0,
      u_sfx_amount: (this._sfxAmount === undefined ? 1.0 : this._sfxAmount),
      u_blackout: 0
    });

    this.host.render();

    // Step 4b: G-12 post chain (feedback/trails -> bloom -> CA/grain/vignette)
    // reads Step 4's composite and writes the actual final frame to the
    // screen. No-op when postEnabled is false (the default) -- Step 4 above
    // already went straight to the screen in that case.
    if (this.postEnabled) {
      this.post.render(this._compositeTexture, null, {
        kick: synthOnset,
        time: timeSec
      });
    }

    // Step 5: Update fps & governor
    this._updateGovernor(deltaMs, now);

    // Heartbeat health is based on reaching the end of a render iteration,
    // not merely entering rAF. If a GL/upload exception aborts the frame, the
    // timestamp remains stale and the external watchdog replaces Chromium.
    markFramePresented();

    this._rafId = requestAnimationFrame(this._onFrame);
  }

  /**
   * Destroy resources cleanly.
   */
  destroy() {
    this.stop();
    this.audio.stop();
    this.deckA.destroy();
    this.deckB.destroy();

    const gl = this.gl;
    if (gl) {
      if (this._fbo) gl.deleteFramebuffer(this._fbo);
      if (this._fboTexture) gl.deleteTexture(this._fboTexture);
      if (this._compositeFBO) gl.deleteFramebuffer(this._compositeFBO);
      if (this._compositeTexture) gl.deleteTexture(this._compositeTexture);
      if (this._blackTexture) gl.deleteTexture(this._blackTexture);
      if (this._mixerProgram) gl.deleteProgram(this._mixerProgram);
      if (this._programCache) {
        for (const entry of this._programCache.values()) {
          if (entry && entry.program) gl.deleteProgram(entry.program);
        }
        this._programCache.clear();
      }
    }
    if (this.post) this.post.destroy();
  }
}
