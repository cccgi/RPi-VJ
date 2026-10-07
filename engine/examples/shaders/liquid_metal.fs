/*{
    "DESCRIPTION": "Liquid Metal Domain-Warped Surface with Fake Specular & Audio Modulation",
    "CATEGORIES": ["2D", "Liquid", "Domain Warping", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "viscosity",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.2,
            "MAX": 3.0
        },
        {
            "NAME": "warp_depth",
            "TYPE": "float",
            "DEFAULT": 1.3,
            "MIN": 0.4,
            "MAX": 3.0
        },
        {
            "NAME": "metallic",
            "TYPE": "float",
            "DEFAULT": 0.85,
            "MIN": 0.2,
            "MAX": 1.0
        },
        {
            "NAME": "iridescence",
            "TYPE": "float",
            "DEFAULT": 0.6,
            "MIN": 0.0,
            "MAX": 1.0
        }
    ]
}*/

uniform float u_time;
uniform vec2  u_resolution;
uniform float u_bass;
uniform float u_mid;
uniform float u_treble;
uniform float u_onset;
uniform float u_volume;
uniform float u_beat_phase;
uniform float u_bar_phase;
uniform float u_kick;
uniform float u_energy;
uniform float u_locked;
uniform float u_hit_low;
uniform float u_hit_high;

uniform float uBass;
uniform float uKick;
uniform float uEnergy;
uniform float uBeatPhase;

// Cheap 2D domain warping function (sine octaves)
float liquidHeight(vec2 p, float t, float bass) {
    vec2 q = vec2(
        sin(p.x * 2.1 + t * 0.7) + cos(p.y * 1.8 - t * 0.5),
        cos(p.x * 1.7 - t * 0.6) + sin(p.y * 2.3 + t * 0.8)
    );
    
    vec2 r = vec2(
        sin(p.x + 3.2 * q.x + t * 0.9 + bass * 0.4),
        cos(p.y + 3.0 * q.y + t * 0.8 + bass * 0.3)
    );
    
    float h = sin(r.x * 2.5 + r.y * 2.5 + t * 1.1) * 0.5 + 0.5;
    h += 0.25 * (sin((p.x + r.x) * 5.0 - t * 1.5) * cos((p.y + r.y) * 5.0 + t * 1.2));
    return h;
}

void main() {
    vec2 res = u_resolution;
    vec2 uv = (gl_FragCoord.xy - 0.5 * res) / min(res.x, res.y);
    
    float bass = max(u_bass, uBass);
    float kick = max(u_kick, max(u_onset, u_hit_low));
    // BUG: energy was floored at 0.5, so half the dynamic range was thrown
    // away before it reached anything and the surface looked constant.
    float energy = max(u_energy, u_volume);
    float hitHi  = max(u_hit_high, u_treble);
    
    float t = u_time * (0.6 * viscosity) + bass * 1.2 + kick * 0.6;
    // Zoom breathes on the low end: the metal heaves rather than simmering.
    float zoom = 2.8 * warp_depth * (1.0 - bass * 0.35 - kick * 0.15);
    vec2 p = uv * zoom;
    
    // ONE height sample, not three. liquidHeight() costs ~9 trig ops and was
    // evaluated three times per pixel purely to finite-difference the gradient
    // -- which is why this shader benched at 1.0x headroom, leaving nothing for
    // a clip underneath or the post chain. Hardware derivatives give the same
    // gradient from the single sample we already have.
    //
    // The constant reproduces the old finite difference: the previous code took
    // (h0 - h(p + 0.012)) in p-space while dFdx works in pixel-space, so convert
    // with the pixels-per-p-unit factor and keep the same 0.012 step.
    float eps = 0.012;
    float h0 = liquidHeight(p, t, bass);

    float relief = 2.0 + energy * 9.0 + kick * 4.0;
    float pixToP = zoom / min(res.x, res.y);
    float k = -eps * relief / max(1e-6, pixToP);
    vec3 normal = normalize(vec3(dFdx(h0) * k, dFdy(h0) * k, eps * 4.0));
    
    // Light directions
    vec3 lightDir1 = normalize(vec3(0.5, 0.8, 0.6));
    vec3 lightDir2 = normalize(vec3(-0.7, -0.4, 0.5));
    vec3 viewDir = vec3(0.0, 0.0, 1.0);
    
    // Diffuse & Specular (Phong with smoothstep)
    float diff1 = max(dot(normal, lightDir1), 0.0);
    float diff2 = max(dot(normal, lightDir2), 0.0) * 0.5;
    
    vec3 ref1 = reflect(-lightDir1, normal);
    vec3 ref2 = reflect(-lightDir2, normal);
    float spec1 = smoothstep(0.7, 0.98, max(dot(ref1, viewDir), 0.0));
    float spec2 = smoothstep(0.8, 0.99, max(dot(ref2, viewDir), 0.0));
    
    // Fresnel rim
    float fresnel = 1.0 - max(dot(normal, viewDir), 0.0);
    fresnel = fresnel * fresnel;
    
    // Chrome / Mercury base palette
    vec3 chrome = vec3(0.75, 0.78, 0.82) * metallic;
    
    // Audio-reactive iridescent sheen
    float iriPhase = h0 * 3.14159 + u_time * 0.4 + fresnel * 2.0;
    vec3 iriColor = vec3(
        sin(iriPhase) * 0.5 + 0.5,
        sin(iriPhase + 2.094) * 0.5 + 0.5,
        sin(iriPhase + 4.188) * 0.5 + 0.5
    );
    
    vec3 baseColor = mix(chrome, iriColor, iridescence * (0.4 + energy * 0.5));
    
    // Shading composition
    vec3 col = baseColor * (diff1 * 0.7 + diff2 * 0.3 + 0.2);
    col += vec3(1.0, 0.98, 0.95) * (spec1 * 1.8 + spec2 * 1.2)
           * (0.25 + energy * 1.6 + hitHi * 1.2 + kick * 0.8);
    col += mix(vec3(0.2, 0.6, 1.0), vec3(1.0, 0.3, 0.8), h0) * (fresnel * 1.2 + kick * 0.4);
    
    // Edge vignette
    col *= smoothstep(1.3, 0.5, length(uv));

    // Master audio gain so silence reads as dark cold chrome.
    col *= 0.18 + energy * 1.4 + kick * 0.4;
    
    gl_FragColor = vec4(col, 1.0);
}
