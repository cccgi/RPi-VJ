/*{
    "DESCRIPTION": "32-bin spectrum comets - each bin launches a light trail when its frequency spikes.",
    "CATEGORIES": ["Generator", "Audio-Reactive", "Multiband"],
    "INPUTS": [
        { "NAME": "trail", "TYPE": "float", "DEFAULT": 1.0, "MIN": 0.3, "MAX": 2.0 },
        { "NAME": "gain", "TYPE": "float", "DEFAULT": 1.0, "MIN": 0.4, "MAX": 2.5 }
    ]
}*/

uniform float u_time;
uniform vec2  u_resolution;
uniform float u_spectrum[32];     // 32 log-spaced bins, 30Hz .. 16kHz
uniform float u_hit_low;
uniform float u_hit_mid;
uniform float u_hit_high;
uniform float u_volume;

float hash(float n){ return fract(sin(n) * 43758.5453123); }

// Interpolate between neighbouring bins so 32 columns read as a continuum.
float specAt(float t){
    float x = clamp(t, 0.0, 0.999) * 31.0;
    float i = floor(x);
    float f = smoothstep(0.0, 1.0, fract(x));
    int lo = int(i), hi = int(min(i + 1.0, 31.0));
    return mix(u_spectrum[lo], u_spectrum[hi], f);
}

void main(){
    vec2 uv = gl_FragCoord.xy / u_resolution;

    // One column per bin: real spectral resolution, not five blocks.
    const float N = 32.0;
    float ci = floor(uv.x * N);
    float cx = (ci + 0.5) / N;

    float lvl = clamp(specAt(cx) * gain, 0.0, 1.4);
    float hit = cx < 0.33 ? u_hit_low : (cx < 0.66 ? u_hit_mid : u_hit_high);

    // Comet: bright head at the bin's level, decaying tail beneath it.
    float head = 0.08 + lvl * 0.82;
    float d = uv.y - head;
    float core = exp(-abs(d) * mix(80.0, 30.0, clamp(lvl, 0.0, 1.0))) * (0.45 + lvl * 1.2);
    float tail = (d < 0.0)
        ? exp(d * mix(16.0, 6.0, clamp(lvl, 0.0, 1.0)) / max(0.35, trail)) * (0.12 + lvl * 0.40)
        : 0.0;

    float burst = 0.0;
    if (hit > 0.01) {
        float bh = head + hit * 0.30 * (0.6 + hash(ci) * 0.8);
        burst = exp(-abs(uv.y - bh) * 45.0) * hit * 0.85;
    }

    // Spectral colour ramp across the full width.
    vec3 c1 = vec3(1.00, 0.18, 0.28);
    vec3 c2 = vec3(1.00, 0.62, 0.10);
    vec3 c3 = vec3(0.20, 1.00, 0.55);
    vec3 c4 = vec3(0.30, 0.70, 1.00);
    vec3 c5 = vec3(0.70, 0.40, 1.00);
    vec3 tint = cx < 0.25 ? mix(c1, c2, cx * 4.0)
              : cx < 0.50 ? mix(c2, c3, (cx - 0.25) * 4.0)
              : cx < 0.75 ? mix(c3, c4, (cx - 0.50) * 4.0)
                          : mix(c4, c5, (cx - 0.75) * 4.0);

    float colMask = smoothstep(0.47, 0.28, abs(fract(uv.x * N) - 0.5));

    vec3 col = tint * (core + tail) * colMask;
    col += vec3(1.0) * burst * colMask * 0.8;
    col += tint * 0.015;

    gl_FragColor = vec4(col, 1.0);
}
