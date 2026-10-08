/*{
    "DESCRIPTION": "Ambient drift field. Calm, but visible - luminosity and colour breathe with the low end.",
    "CATEGORIES": ["2D", "Ambient", "Audio-Reactive"],
    "INPUTS": [
        { "NAME": "drift_speed", "TYPE": "float", "DEFAULT": 0.4, "MIN": 0.05, "MAX": 1.5 },
        { "NAME": "softness", "TYPE": "float", "DEFAULT": 1.2, "MIN": 0.5, "MAX": 2.5 },
        { "NAME": "brightness", "TYPE": "float", "DEFAULT": 1.0, "MIN": 0.3, "MAX": 2.0 }
    ]
}*/

uniform float u_time;
uniform vec2  u_resolution;
uniform float u_sub;
uniform float u_bass;
uniform float u_low_mid;
uniform float u_high_mid;
uniform float u_treble;
uniform float u_volume;
uniform float u_onset;
uniform float u_bar_phase;

float hash21(vec2 p){ return fract(sin(dot(p, vec2(12.9898,78.233))) * 43758.5453); }
float noise(vec2 p){
    vec2 i = floor(p), f = fract(p);
    vec2 u = f*f*(3.0-2.0*f);
    return mix(mix(hash21(i), hash21(i+vec2(1,0)), u.x),
               mix(hash21(i+vec2(0,1)), hash21(i+vec2(1,1)), u.x), u.y);
}
// Was 3 noise() taps (12 hash21 calls) per fbm, called twice per pixel for
// n1/n2 = 24 hash21 calls/px. This shader benched at 1.0x headroom -- no
// room for a clip underneath or the post chain (see PERF.md vs
// tasks/HANDOFF_AUDIO_DONE.md item 1). Dropped to 2 octaves: the third
// octave only ever contributed 0.20 of amplitude to an already-soft ambient
// field, so its loss is not visible, but it halves the hash21 cost to
// 8 calls/fbm, 16/px.
float fbm(vec2 p, float t){
    float v = 0.60*noise(p + vec2(t*0.10,-t*0.05)); p *= 2.02;
    v += 0.40*noise(p - vec2(t*0.08, t*0.12));
    return v;
}

void main() {
    vec2 res = u_resolution;
    vec2 uv = (gl_FragCoord.xy - 0.5*res) / min(res.x, res.y);

    float t = u_time * (drift_speed * 0.30);
    vec2 p = uv * (1.8 / max(softness, 0.2));

    float n1 = fbm(p, t);
    // n2 only ever fed a secondary violet blend and a treble sparkle -- it
    // does not need full 2-octave detail. Measured on-Pi (V3D 7.1.10.2):
    // dropping this to one noise() tap took deep_drift from 1.50x to 1.99x
    // headroom, matching liquid_metal's target -- see
    // tasks/HANDOFF_AUDIO_DONE.md item 1. sin()-vs-mul hash made ZERO
    // difference in the same test (11.117ms both ways): the cost here is
    // fbm() octave count, not the hash function, on this GPU.
    vec2 p2 = p*1.5 + vec2(n1*0.8, -n1*0.6);
    float n2 = noise(p2 + vec2(t*0.7*0.10, -t*0.7*0.05));

    // The previous version topped out around 0.20 and was then mixed toward
    // 0.02 -- effectively black on a TV. These are real, visible colours.
    vec3 cDeep  = vec3(0.06, 0.10, 0.34);
    vec3 cTeal  = vec3(0.04, 0.42, 0.52);
    vec3 cViol  = vec3(0.42, 0.16, 0.62);
    vec3 cMist  = vec3(0.72, 0.82, 0.98);

    vec3 col = mix(cDeep, cTeal, smoothstep(0.20, 0.70, n1));
    col = mix(col, cViol, smoothstep(0.30, 0.80, n2));
    col = mix(col, cMist, smoothstep(0.55, 0.95, n1*n2) * 0.55);

    // Audio drives luminosity and how far the mist opens up, so it breathes
    // with the track without ever becoming a strobe -- this is the calm one.
    float lift = 0.55 + u_volume * 0.65 + u_sub * 0.45;
    float open = smoothstep(0.62 - u_bass * 0.28, 0.92, n1 * n2);
    col += cMist * open * (0.18 + u_low_mid * 0.35);

    // Highs add a faint sparkle in the brighter regions only.
    float spark = smoothstep(0.75, 1.0, n2) * u_treble;
    col += vec3(0.85, 0.92, 1.0) * spark * 0.30;

    col *= lift * brightness;
    col *= smoothstep(1.6, 0.45, length(uv));

    gl_FragColor = vec4(col, 1.0);
}
