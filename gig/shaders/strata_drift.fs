/*{
    "DESCRIPTION": "Parallax strata layers, each assigned to a frequency band. Slow and wide - the set needs somewhere to go that is not intense.",
    "CATEGORIES": ["Generator", "Audio-Reactive", "Multiband", "Ambient"],
    "INPUTS": [
        { "NAME": "layers", "TYPE": "float", "DEFAULT": 5.0, "MIN": 3.0, "MAX": 7.0 },
        { "NAME": "drift", "TYPE": "float", "DEFAULT": 1.0, "MIN": 0.0, "MAX": 3.0 }
    ]
}*/

uniform float u_time;
uniform vec2  u_resolution;
uniform float u_sub;
uniform float u_bass;
uniform float u_low_mid;
uniform float u_high_mid;
uniform float u_treble;
uniform float u_hit_high;
uniform float u_hit_low;
uniform float u_bar_phase;

// Cheap 2D value noise -- no raymarching. Depth comes from parallax between
// layers, which costs nothing and reads as 3D once trails are applied.
float hash(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float noise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), f.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), f.x), f.y);
}

void main() {
    vec2 uv = (gl_FragCoord.xy * 2.0 - u_resolution) / u_resolution.y;

    float b[5];
    b[0] = u_sub; b[1] = u_bass; b[2] = u_low_mid; b[3] = u_high_mid; b[4] = u_treble;

    vec3 tint[5];
    tint[0] = vec3(0.35, 0.10, 0.55);
    tint[1] = vec3(0.10, 0.30, 0.70);
    tint[2] = vec3(0.05, 0.60, 0.65);
    tint[3] = vec3(0.55, 0.30, 0.75);
    tint[4] = vec3(0.85, 0.65, 0.95);

    int n = int(clamp(layers, 3.0, 7.0));
    vec3 col = vec3(0.02, 0.02, 0.05);

    for (int i = 0; i < 7; i++) {
        if (i >= n) break;
        float fi = float(i);
        float lvl = b[int(mod(fi, 5.0))];

        // nearer layers move faster -- parallax without any 3D cost
        float depth = 1.0 - fi / float(n);
        float speed = 0.04 + depth * 0.16;
        vec2 p = uv * (1.2 + fi * 0.55);
        p.x += u_time * speed * drift;
        p.y += fi * 3.7 + sin(u_time * 0.08 * drift + fi) * 0.2;

        float v = noise(p * 1.5) * 0.65 + noise(p * 3.4) * 0.35;
        // each layer's band decides how far it opens up
        float band = smoothstep(0.62 - lvl * 0.30, 0.86, v);

        col += tint[i] * band * (0.10 + lvl * 0.55) * (0.35 + depth * 0.65);
    }

    // LIGHTNING. A spectral spike fires a branching flash inside the cloud
    // rather than a flat brightness bump -- the strike is shaped, brief and
    // lands somewhere different each time.
    float strike = max(u_hit_high, u_hit_low * 0.8);
    if (strike > 0.02) {
        // seed a horizontal position that changes per strike
        float seed = floor(u_time * 3.0);
        float sx = (hash(vec2(seed, 7.3)) - 0.5) * 1.6;
        // jagged vertical channel: noise displaces the bolt as it descends
        float jag = (noise(vec2(uv.y * 6.0 + seed, seed)) - 0.5) * 0.35
                  + (noise(vec2(uv.y * 22.0 + seed, seed * 1.7)) - 0.5) * 0.10;
        float d = abs(uv.x - (sx + jag));
        float bolt = exp(-d * 90.0) + exp(-d * 26.0) * 0.35;
        // the flash also lights the surrounding cloud
        float glow = exp(-d * 5.0) * 0.25;
        col += vec3(0.80, 0.88, 1.0) * (bolt + glow) * strike * 1.6;
    }
    // faint shimmer between strikes keeps it alive
    col += vec3(0.9, 0.95, 1.0) * u_treble * 0.04 * noise(uv * 40.0 + u_time);

    float vig = 1.0 - smoothstep(0.6, 1.6, length(uv));
    gl_FragColor = vec4(col * vig, 1.0);
}
