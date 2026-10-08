#version 300 es
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
uniform sampler2D u_hud_tex;
uniform int u_geo_on;   // shader-layer geometry active
uniform vec4 u_geo;     // zoom scale, angle, twist, stretch
uniform vec4 u_geo2;    // ripple amplitude, ripple phase, mirror segments, brightness
uniform int u_hud;

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

// Shader-layer geometry: mirror fold, rotation that differs from centre to edge, zoom,
// stretch and a radial ripple. Edges mirror so a rotated layer never shows a border.
vec2 geoUV(vec2 uv) {
  float aspect = u_resolution.x / u_resolution.y;
  vec2 p = (uv - 0.5) * vec2(aspect, 1.0);
  if (u_geo2.z > 1.5) {
    float seg = 3.14159265 / u_geo2.z;
    float a = abs(mod(atan(p.y, p.x), 2.0 * seg) - seg);
    p = length(p) * vec2(cos(a), sin(a));
  }
  float r = length(p);
  float ang = u_geo.y + u_geo.z * (0.5 - r);
  float c = cos(ang), s = sin(ang);
  p = mat2(c, -s, s, c) * p * u_geo.x;
  p *= vec2(1.0 + u_geo.w, 1.0 - u_geo.w);
  if (r > 0.0001) p += (p / r) * sin(r * 36.0 - u_geo2.y) * u_geo2.x;
  uv = p / vec2(aspect, 1.0) + 0.5;
  return 1.0 - abs(mod(uv, 2.0) - 1.0);
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
  vec2 guv = (u_geo_on == 1) ? geoUV(uv) : uv;
  vec2 suv = (u_sfx > 0) ? fxUV(guv, u_sfx, u_sfx_amount) : guv;
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

  shaderSample.rgb = clamp(shaderSample.rgb * u_geo2.w, 0.0, 1.0);

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
  vec2 hudPixel=(gl_FragCoord.xy-vec2(20.0))/2.0;
  if(u_hud==1 && all(greaterThanEqual(hudPixel,vec2(0.0))) && all(lessThan(hudPixel,vec2(768.0,16.0)))) {
    float text=texture(u_hud_tex,hudPixel/vec2(768.0,16.0)).r;
    fragColor=vec4(mix(blended*0.15,vec3(1.0),text),1.0);
  }
}

