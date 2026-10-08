/* {"LABEL":"Shadertoy native"} */
#define HW_PERFORMANCE 1
precision highp float;
precision highp int;
#define iFrameRate 60.0
#define iDate vec4(2025.0, 1.0, 1.0, 0.0)
uniform vec4 iMouse;
uniform sampler2D iChannel0;
uniform sampler2D iChannel1;
uniform sampler2D iChannel2;
uniform sampler2D iChannel3;
uniform float iChannelTime[4];
uniform vec3 iChannelResolution[4];

// ============================================================================
// UNIFIED GROUND-STATE DIMENSIONAL ENGINE: FULL DUAL-SPECTRUM (-8 TO +8)
//
// NEGATIVE SPECTRUM (Collapse, Anti-Phase, Inversion & Entropy):
//  -8 : Quantum Phase Decoherence (Entanglement collapse into quantum noise)
//  -7 : Inverse Hyperbolic Event Horizon (Schwarzschild spatial metric warping)
//  -6 : Strange Repellor (Inverted non-linear attractor feedback)
//  -5 : Spatial Tearing / Phase Shear (Non-Euclidean metric tearing)
//  -4 : Temporal Asymmetry Mirror (Reverse time-dilation shear)
//  -3 : Destructive Anti-Phase Interference (Irrational inverse cancellation)
//  -2 : Dimensional Retrograde Inversion (2D -> 1D -> 0D space collapse)
//  -1 : Raw Unbuffered Fract Singularity (Bare cross-coupled phase snaps)
//
//   0 : Pure Ground-State Baseline (Polar atan branch-cut baseline)
//
// POSITIVE SPECTRUM (Expansion, Chirality, Fold Geometry & Resonance):
//   1 : Original High-Speed Engine (Standard baseline non-linear engine)
//   2 : Fully Chiral-Dampened Engine (Bilateral symmetry & low shear)
//   3 : Uncoupled Pan Engine (Forward drive without torque back-coupling)
//   4 : Slowed Smooth OG Engine (Original asymmetry + smooth slow drift)
//   5 : Continuous Chiral Blend (Real-time dynamic blend via mouse / uniform)
//   6 : Strange Attractor Resonance (Unpredictable non-linear feedback chaos)
//   7 : Hyperbolic Non-Euclidean Fold (Stable smooth metric metamorphosis)
//   8 : Holographic Phase Interference (Quantum lattice collapse & solitons)
// ============================================================================

// ----------------------------------------------------------------------------
// HARDCODED CONFIGURATION (Change this number -8 to +8 to switch modes):
// ----------------------------------------------------------------------------
#define RENDER_MODE 5

#define PHI   1.61803398875   // Golden Ratio
#define SQRT2 1.41421356237   // Silver Ratio
#define SQRT3 1.73205080757   // Bronze Ratio
#define SQRT5 2.23606797750   // Platinum Ratio
#define TAU   6.28318530718

uniform float u_chiralDampening; // Used in Mode 5

mat2 rot2D(float a) {
    float c = cos(a), s = sin(a);
    return mat2(c, -s, s, c);
}

// ----------------------------------------------------------------------------
// STEP 0: 0D SINGULARITY TENSOR
// ----------------------------------------------------------------------------
vec4 get0DSingularityTensor(float t, float dampFactor) {
    float stateA, stateB, stateC, phase;

#if RENDER_MODE == 0
    stateA = sin(t * PHI);
    stateB = cos(t * SQRT2);
    stateC = sin(t * SQRT3);
    phase  = fract(t * 0.1 * PHI);
    return vec4(stateA, stateB, stateC, phase);

// --- NEGATIVE MODES ---
#elif RENDER_MODE == -2
    // Mode -2: Retrograde Inversion Tensor
    stateA = sin(-t * PHI);
    stateB = cos(-t * SQRT2);
    stateC = sin(-t * SQRT3);
    phase  = 1.0 - fract(t * 0.1 * PHI);

#elif RENDER_MODE == -3
    // Mode -3: Destructive Anti-Phase Inverse Tensor
    stateA = sin(t / PHI);
    stateB = cos(t / SQRT2);
    stateC = sin(t / SQRT3);
    phase  = 0.5 + 0.5 * cos(t * PHI);

#elif RENDER_MODE == -4
    // Mode -4: Temporal Mirror Shear Tensor
    stateA = sin(t * PHI) * cos(-t * SQRT2);
    stateB = cos(t * SQRT2) * sin(-t * SQRT3);
    stateC = sin(t * SQRT3) * cos(-t * PHI);
    phase  = fract(-t * 0.2 * SQRT2);

#elif RENDER_MODE == -5
    // Mode -5: Phase Shear Discontinuity Tensor
    stateA = sign(sin(t * PHI)) * pow(abs(sin(t * PHI)), 0.3);
    stateB = sign(cos(t * SQRT2)) * pow(abs(cos(t * SQRT2)), 0.3);
    stateC = sin(t * SQRT3);
    phase  = fract(stateA * stateB * PHI);

#elif RENDER_MODE == -6
    // Mode -6: Strange Repellor Inverse Attractor
    stateA = sin(t * PHI - sin(t * SQRT2) * 2.0);
    stateB = cos(t * SQRT2 - cos(stateA * SQRT3) * 1.5);
    stateC = sin(t * SQRT3 - sin(stateB * PHI) * 1.5);
    phase  = fract(dot(vec3(stateA, stateB, stateC), vec3(-PHI, SQRT2, -SQRT3)));

#elif RENDER_MODE == -7
    // Mode -7: Gravitational Black Hole Event Horizon Tensor
    float gravityWell = 1.0 / (abs(sin(t * 0.2)) + 0.1);
    stateA = sin(t * PHI * gravityWell * 0.1);
    stateB = cos(t * SQRT2 * gravityWell * 0.1);
    stateC = sin(t * SQRT3 * gravityWell * 0.1);
    phase  = fract(gravityWell * 0.05);

#elif RENDER_MODE == -8
    // Mode -8: Quantum Decoherence Noise Tensor
    float noise = fract(sin(t * 1234.567) * 43758.5453);
    stateA = sin(t * PHI + noise * 0.5);
    stateB = cos(t * SQRT2 - noise * 0.5);
    stateC = sin(t * SQRT3 + noise * 0.5);
    phase  = noise;

// --- POSITIVE MODES ---
#elif RENDER_MODE == 6
    stateA = sin(t * PHI + sin(t * SQRT2) * cos(t * SQRT3) * 2.0);
    stateB = cos(t * SQRT2 + sin(stateA * PHI) * 1.5);
    stateC = sin(t * SQRT3 + cos(stateB * SQRT2) * 1.5);
    phase  = sin(t * 0.2 + stateA * 0.5) * 0.5 + 0.5;

#elif RENDER_MODE == 7
    stateA = sin(t * PHI * 0.5);
    stateB = cos(t * SQRT2 * 0.5);
    stateC = sin(t * SQRT3 * 0.5);
    phase  = 0.5 + 0.5 * cos(t * 0.1);

#elif RENDER_MODE == 8
    stateA = sin(t * PHI + cos(t * SQRT5) * 1.8);
    stateB = cos(t * SQRT2 + sin(t * PHI) * 1.8);
    stateC = sin(t * SQRT3 + cos(stateA * SQRT5) * 1.8);
    phase  = fract(dot(vec3(stateA, stateB, stateC), vec3(PHI, SQRT2, SQRT3)) * 0.25 + sin(t * 0.3) * PHI);

#elif RENDER_MODE == 3
    stateA = sin(t * PHI);
    stateB = cos(t * SQRT2);
    stateC = sin(t * SQRT3);
    phase  = 0.5 + 0.5 * sin(t * PHI);

#elif RENDER_MODE == -1 || RENDER_MODE == 1
    stateA = sin(t * PHI + cos(t * 0.3 * SQRT2) * 1.5);
    stateB = cos(t * SQRT2 + sin(stateA * SQRT3) * 1.2);
    stateC = sin(t * SQRT3 + cos(stateB * PHI) * 1.8);
    phase  = (RENDER_MODE == -1) ? fract(dot(vec3(stateA, stateB, stateC), vec3(PHI, SQRT2, SQRT3))) 
                                 : fract(dot(vec3(stateA, stateB, stateC), vec3(PHI, SQRT2, SQRT3)) * 0.1);

#else
    // Modes 2, 4, 5
    stateA = sin(t * PHI + cos(t * 0.15 * SQRT2) * 0.8);
    stateB = cos(t * SQRT2 + sin(stateA * SQRT3) * 0.6);
    stateC = sin(t * SQRT3 + cos(stateB * PHI) * 0.9);
    phase  = 0.5 + 0.5 * sin(dot(vec3(stateA, stateB, stateC), vec3(PHI, SQRT2, SQRT3)) * 0.5);
#endif

    if (dampFactor > 0.0) {
        float symmetricAB = (stateA + stateB) * 0.5;
        stateA = mix(stateA, symmetricAB, dampFactor);
        stateB = mix(stateB, symmetricAB, dampFactor);
    }

    return vec4(stateA, stateB, stateC, phase);
}

// ----------------------------------------------------------------------------
// STEP 1: CONTINUOUS 1D HARMONIC STREAM
// ----------------------------------------------------------------------------
float project1DStream(vec2 dir2D, float r, vec4 pointTensor0D, float dampFactor) {
    vec2 dirProj = dir2D;
    dirProj.x = mix(dirProj.x, abs(dirProj.x), dampFactor);

    float linearCoord = dot(dirProj, vec2(PHI, SQRT2)) * 2.0 + r * 3.0;
    
#if RENDER_MODE == -3
    // Destructive Anti-Phase Stream Cancellation
    float wave1 = sin(linearCoord * PHI + pointTensor0D.x * TAU);
    float wave2 = -sin(linearCoord * (1.0 / PHI) - pointTensor0D.x * TAU) * 0.95;
    float wave3 = cos(linearCoord * SQRT2 + pointTensor0D.y * TAU) * 0.3;
    return wave1 + wave2 + wave3;
#else
    float wave1 = sin(linearCoord * PHI + pointTensor0D.x * TAU);
    float wave2 = cos(linearCoord * SQRT2 + pointTensor0D.y * TAU + wave1 * 0.5) * 0.5;
    float wave3 = sin(linearCoord * SQRT3 + pointTensor0D.z * TAU - wave2 * 0.3) * 0.25;
    return wave1 + wave2 + wave3;
#endif
}

float project1DStreamMode0(float coord1D, vec4 pointTensor0D) {
    float wave1 = sin(coord1D * PHI + pointTensor0D.x * TAU);
    float wave2 = cos(coord1D * SQRT2 + pointTensor0D.y * TAU) * 0.5;
    float wave3 = sin(coord1D * SQRT3 + pointTensor0D.z * TAU) * 0.25;
    return wave1 + wave2 + wave3;
}

// ----------------------------------------------------------------------------
// EMERGENT SPECTRAL GENERATOR
// ----------------------------------------------------------------------------
vec3 getEmergentSpectrum(float wave1D, vec2 uv2D, vec4 pointTensor0D) {
    vec3 freq = vec3(PHI, SQRT2, SQRT3);
    vec3 shift = vec3(0.0, 0.333 * TAU, 0.666 * TAU);
    
#if RENDER_MODE < 0 && RENDER_MODE != -1
    // Negative Modes: Inverted spectral color palette
    freq = vec3(1.0 / PHI, 1.0 / SQRT2, 1.0 / SQRT3);
    shift = vec3(0.666 * TAU, 0.333 * TAU, 0.0);
#endif

    vec3 state = vec3(uv2D, wave1D) + pointTensor0D.xyz * 0.2;
    vec3 harmonic = 0.5 + 0.5 * cos(wave1D * freq * 3.14159 + state * PHI + shift);
    return clamp(harmonic, 0.0, 1.0);
}

// ----------------------------------------------------------------------------
// STEP 2: 2D PLANAR FOLD & SURFACE MANIFOLD
// ----------------------------------------------------------------------------
vec3 render2DManifold(vec2 uv, vec4 pointTensor0D, float dampFactor) {
    float r = length(uv);

#if RENDER_MODE == 0
    float angle = atan(uv.y, uv.x);
    float linearCoord0D = angle * 2.0 + r * 3.0;
    float stream1D = project1DStreamMode0(linearCoord0D, pointTensor0D);

    vec2 p = uv * 2.5;
    float accum = 0.0;
    float scale = 1.0;

    for (int i = 0; i < 4; i++) {
        p = abs(p) / clamp(dot(p, p), 0.2, 2.0) - vec2(0.6, 0.6);
        p *= rot2D(0.3 * PHI + stream1D * 0.1);
        accum += length(p) * scale;
        scale *= 0.65;
    }

    float pattern = sin((accum + stream1D) * 12.0 + u_time * 2.0);
    float lines = smoothstep(0.02, 0.98, abs(pattern));

    vec3 col = getEmergentSpectrum(stream1D, p, pointTensor0D);
    vec3 accent = getEmergentSpectrum(accum * 0.5, uv, pointTensor0D);
    col = mix(col, accent, lines * 0.75);

    float boundary = smoothstep(0.0, 0.02, 1.2 - r);
    float ringGlow = smoothstep(0.08, 0.0, abs(r - 1.2));
    vec3 rimCol = getEmergentSpectrum(angle + u_time, vec2(cos(angle), sin(angle)), pointTensor0D);

    return (col * boundary) + (rimCol * ringGlow * 2.0);

// --- NEGATIVE MODES RENDER LOGIC ---
#elif RENDER_MODE == -2
    // Mode -2: Dimensional Retrograde Inversion (2D -> 1D -> 0D Collapse)
    vec2 p = uv * (1.0 / max(r, 0.01)) * 0.5; // Collapse space inward
    float stream1D = project1DStream(uv, r, pointTensor0D, dampFactor);
    float accum = 0.0;
    for (int i = 0; i < 4; i++) {
        p = abs(p) - vec2(0.5, 0.5);
        p *= rot2D(-0.3 * PHI - u_time * 0.2);
        accum += dot(p, p);
    }
    vec3 col = getEmergentSpectrum(stream1D + accum, p, pointTensor0D);
    float horizon = smoothstep(0.0, 0.02, 1.2 - r);
    return col * horizon;

#elif RENDER_MODE == -5
    // Mode -5: Spatial Tearing / Phase Shear Discontinuity
    vec2 dir = uv / max(r, 0.0001);
    float stream1D = project1DStream(dir, r, pointTensor0D, dampFactor);
    vec2 p = uv * 2.5;
    float accum = 0.0;
    for (int i = 0; i < 4; i++) {
        p = sign(p) * floor(abs(p) * 4.0) / 4.0; // Spatial quantization tearing
        p *= rot2D(0.5 * PHI + stream1D);
        accum += length(p);
    }
    vec3 col = getEmergentSpectrum(stream1D, p, pointTensor0D);
    float boundary = smoothstep(0.0, 0.02, 1.2 - r);
    return col * boundary;

#elif RENDER_MODE == -7
    // Mode -7: Inverse Hyperbolic Event Horizon (Schwarzschild Gravity Well)
    float eventHorizon = 0.35;
    float gravity = eventHorizon / max(abs(r - eventHorizon), 0.001);
    vec2 warpedUV = uv * (1.0 + gravity * 0.2);
    
    vec2 dir = warpedUV / max(length(warpedUV), 0.0001);
    float stream1D = project1DStream(dir, r, pointTensor0D, 1.0);
    vec2 p = warpedUV * 2.0;
    float accum = 0.0;

    for (int i = 0; i < 4; i++) {
        p = abs(p) / clamp(dot(p, p), 0.05, 3.0) - vec2(0.4, 0.4);
        p *= rot2D(gravity * 0.05 + 0.2 * PHI);
        accum += length(p);
    }

    vec3 col = getEmergentSpectrum(stream1D + gravity * 0.1, p, pointTensor0D);
    float ringGlow = smoothstep(0.12, 0.0, abs(r - eventHorizon));
    float boundary = smoothstep(0.0, 0.02, 1.2 - r);
    vec3 shadow = vec3(smoothstep(0.0, 0.05, r - eventHorizon));

    return (col * shadow * boundary) + (vec3(0.2, 0.6, 1.0) * ringGlow * 3.0);

#elif RENDER_MODE == -8
    // Mode -8: Quantum Phase Decoherence Noise
    vec2 dir = uv / max(r, 0.0001);
    float stream1D = project1DStream(dir, r, pointTensor0D, dampFactor);
    vec2 p = uv * 3.0;

    float accum = 0.0;
    for (int i = 0; i < 5; i++) {
        vec2 noiseVec = vec2(
            fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453),
            fract(cos(dot(p, vec2(4.898, 38.233))) * 23421.6312)
        );
        p = abs(p + noiseVec * 0.1) - vec2(0.6, 0.6);
        p *= rot2D(0.3 * PHI);
        accum += length(p);
    }

    vec3 col = getEmergentSpectrum(stream1D + accum, p, pointTensor0D);
    float boundary = smoothstep(0.0, 0.02, 1.2 - r);
    return col * boundary;

// --- STANDARD MODES LOGIC ---
#else
    vec2 dir = (r < 0.0001) ? vec2(1.0, 0.0) : uv / r;
    float stream1D = project1DStream(dir, r, pointTensor0D, dampFactor);
    vec2 p = uv * 2.5;

#if RENDER_MODE == 3
    p *= rot2D(u_time * 0.05 + pointTensor0D.w * TAU * 0.1);
#endif

    float accum = 0.0;
    float scale = 1.0;
    float rotStrength = mix(0.15, 0.03, dampFactor);

    for (int i = 0; i < 4; i++) {
        p = abs(p) / clamp(dot(p, p), 0.2, 2.0) - vec2(0.6, 0.6);

#if RENDER_MODE == 3
        float foldRotation = 0.3 * PHI + pointTensor0D.w * 0.5;
#elif RENDER_MODE == -1
        float foldRotation = 0.3 * PHI + stream1D * 0.15 + pointTensor0D.w * TAU;
#elif RENDER_MODE == 1
        float foldRotation = 0.3 * PHI + stream1D * 0.15 + pointTensor0D.w;
#else
        float foldRotation = 0.3 * PHI + stream1D * rotStrength + pointTensor0D.w * 0.5;
#endif

        p *= rot2D(foldRotation);
        accum += length(p) * scale;
        scale *= 0.65;
    }

    float patternSpeed = (RENDER_MODE == -1 || RENDER_MODE == 1) ? 1.5 : 0.3;
    float pattern = sin((accum + stream1D) * 12.0 + u_time * patternSpeed);
    float lines = smoothstep(0.02, 0.98, abs(pattern));

    vec3 col = getEmergentSpectrum(stream1D, p, pointTensor0D);
    vec3 accent = getEmergentSpectrum(accum * 0.5, uv, pointTensor0D);
    col = mix(col, accent, lines * 0.75);

    float boundary = smoothstep(0.0, 0.02, 1.2 - r);
    float ringGlow = smoothstep(0.08, 0.0, abs(r - 1.2));

    vec2 rimDir = mix(dir, vec2(abs(dir.x), dir.y), dampFactor);
    float rimTimeSpeed = (RENDER_MODE == -1 || RENDER_MODE == 1) ? 1.0 : 0.2;
    vec3 rimCol = getEmergentSpectrum(rimDir.x * 3.0 + u_time * rimTimeSpeed, rimDir, pointTensor0D);

    return (col * boundary) + (rimCol * ringGlow * 2.0);
#endif
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;

    float dampFactor = 0.0;
#if RENDER_MODE == 2
    dampFactor = 1.0;
#elif RENDER_MODE == 5
    dampFactor = u_chiralDampening;
    if (iMouse.z > 0.0) {
        dampFactor = clamp(iMouse.x / vec3(u_resolution, 1.0).x, 0.0, 1.0);
    }
#endif

#if RENDER_MODE == 0
    float t = u_time * 0.5;
#elif RENDER_MODE < 0
    float t = -u_time * 0.3; // Reversed time flow for negative spectrum
#else
    float t = u_time * 0.08;
#endif

    vec4 pointTensor0D = get0DSingularityTensor(t, dampFactor);

    if (iMouse.z > 0.0 && RENDER_MODE <= 1) {
        vec2 m = iMouse.xy / vec3(u_resolution, 1.0).xy;
        pointTensor0D.xy += m * TAU;
    }

    vec3 finalCol = render2DManifold(uv, pointTensor0D, dampFactor);

    float bgGlow = smoothstep(1.5, 0.2, length(uv));
    vec3 backDrop = vec3(0.002, 0.005, 0.01) * bgGlow;

    finalCol = mix(backDrop, finalCol, smoothstep(0.0, 0.01, length(finalCol)));
    finalCol = pow(finalCol, vec3(0.85));

    fragColor = vec4(finalCol, 1.0);
}
void main(){
    vec2 center = 0.5 * u_resolution;
    vec2 q = gl_FragCoord.xy - center;
    float audio_zoom = 1.0 + 0.28*u_bass + 0.18*u_beat_onset;
    float audio_angle = 0.35*u_mid + 0.12*sin(u_time*1.7)*u_treble;
    float audio_cs = cos(audio_angle), audio_sn = sin(audio_angle);
    q = mat2(audio_cs,-audio_sn,audio_sn,audio_cs) * q / audio_zoom;
    q += vec2(sin(q.y*0.018 + u_time*2.0), cos(q.x*0.014 - u_time*1.4))
         * (7.0*u_mid + 4.0*u_treble + 18.0*u_beat_onset);
    q += 3.0 * vec2(sin(u_time*5.0), cos(u_time*4.0)) * u_bass;
    q += center;
    vec4 c=vec4(0.0);
    mainImage(c, q);
    float e=clamp(0.65+0.55*u_volume+0.20*u_bass,0.25,1.45);
    c.rgb*=e;
    float luma = dot(clamp(c.rgb, vec3(0.0), vec3(1e9)), vec3(0.2126, 0.7152, 0.0722));
    float gain = mix(1.55, 0.95, smoothstep(0.16, 0.82, luma));
    c.rgb = clamp(pow(clamp(c.rgb, vec3(0.0), vec3(1e9)), vec3(0.70)) * gain, 0.0, 1.0);
    gl_FragColor=c;
}
