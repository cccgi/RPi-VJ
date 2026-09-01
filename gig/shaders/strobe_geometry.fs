/*{
    "DESCRIPTION": "Beat-Synced Hard-Edged Geometric Strobe Matrix with Built-in Photosafety Limiter",
    "CATEGORIES": ["2D", "Geometric", "Strobe", "Beat-Synced", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "grid_density",
            "TYPE": "float",
            "DEFAULT": 4.0,
            "MIN": 1.0,
            "MAX": 12.0
        },
        {
            "NAME": "flash_intensity",
            "TYPE": "float",
            "DEFAULT": 0.8,
            "MIN": 0.0,
            "MAX": 1.0
        },
        {
            "NAME": "pattern_mode",
            "TYPE": "float",
            "DEFAULT": 0.0,
            "MIN": 0.0,
            "MAX": 3.0
        },
        {
            "NAME": "color_mode",
            "TYPE": "float",
            "DEFAULT": 0.0,
            "MIN": 0.0,
            "MAX": 2.0
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
uniform float u_hit_mid;
uniform float u_hit_high;

uniform float uBass;
uniform float uKick;
uniform float uBeatPhase;
uniform float uBarPhase;

void main() {
    vec2 res = u_resolution;
    vec2 uv = (gl_FragCoord.xy - 0.5 * res) / min(res.x, res.y);
    
    float kick   = max(u_kick, max(u_onset, uKick));
    float bass   = max(u_bass, uBass);
    float energy = max(u_energy, u_volume);
    float hitLo  = max(u_hit_low, kick);
    float hitMid = u_hit_mid;
    float hitHi  = max(u_hit_high, u_treble);
    // Only trust the tempo clock while it is phase-locked to real onsets.
    float lock = clamp(u_locked, 0.0, 1.0);
    float beat = max(u_beat_phase, uBeatPhase) * lock;
    float bar  = max(u_bar_phase, uBarPhase) * lock;
    
    // Beat-synchronized step quantizer (snaps 4 or 8 times per bar)
    float snapBeat = floor(beat * 4.0) / 4.0;
    float snapBar = floor(bar * 8.0) / 8.0;
    
    // Subtle rotation step per beat division
    // Step the whole matrix on the audio-integrated clock: it advances a
    // notch per burst of energy instead of ticking at a fixed wall-clock
    // rate, so the geometry moves WITH the track rather than beside it.
    float angle = snapBar * 3.14159265 * 0.5 + floor(u_flow * 2.0) * 0.19634;
    float cosA = cos(angle);
    float sinA = sin(angle);
    vec2 rotUV = vec2(uv.x * cosA - uv.y * sinA, uv.x * sinA + uv.y * cosA);
    
    // Repeating geometric grid
    float density = floor(grid_density);
    rotUV *= 1.0 - bass * 0.30 - hitLo * 0.12;   // matrix zooms on the kick
    vec2 gridUV = fract(rotUV * density + 0.5) - 0.5;
    vec2 cellID = floor(rotUV * density + 0.5);
    
    // Shape SDFs
    // 1. Box / Rect
    vec2 boxD = abs(gridUV) - vec2(0.14 + 0.30 * energy + 0.12 * hitLo + 0.06 * sin(snapBeat * 6.283));
    float boxShape = step(max(boxD.x, boxD.y), 0.0);
    float boxEdge = step(abs(max(boxD.x, boxD.y)), 0.05);
    
    // 2. Diamond / Rhombus
    float diamondD = (abs(gridUV.x) + abs(gridUV.y)) - (0.18 + 0.34 * energy + 0.12 * hitMid);
    float diamondShape = step(diamondD, 0.0);
    float diamondEdge = step(abs(diamondD), 0.04);
    
    // 3. Concentric Crosshair
    float crossD = min(abs(gridUV.x), abs(gridUV.y)) - 0.06;
    float crossEdge = step(crossD, 0.0) * step(length(gridUV), 0.45);
    
    // Pattern selection
    float shape = mix(boxEdge, diamondEdge, step(1.0, pattern_mode));
    shape = mix(shape, crossEdge, step(2.0, pattern_mode));
    
    // Checkerboard invert pulse on 1/4 beats
    float checker = mod(cellID.x + cellID.y, 2.0);
    // Transient-driven flash. hitLo/hitMid are per-band onset envelopes that
    // decay to zero, so this genuinely strobes on the music. The locked beat
    // phase is layered on top only when the tempo clock is actually locked.
    float beatPulse = max(hitLo, hitMid * 0.8);
    beatPulse = max(beatPulse, lock * smoothstep(0.3, 0.0, fract(beat * 2.0)));
    beatPulse = clamp(beatPulse, 0.0, 1.0);
    
    // Photosafety limiter: ensure strobing is localized / bordered, never full-screen 100% white
    // Safe pulse envelope: maximum flash clamped to 0.75 luminance, decayed smoothly
    float safeFlash = min(flash_intensity, 0.75) * beatPulse * (0.6 + kick * 0.4);
    
    // Color palettes
    vec3 colA = vec3(0.05, 0.95, 1.0);  // High-voltage Cyan
    vec3 colB = vec3(1.0, 0.1, 0.45);   // High-voltage Pink
    vec3 colC = vec3(1.0, 0.85, 0.1);   // High-voltage Yellow
    
    vec3 activeCol = mix(colA, colB, checker);
    activeCol = mix(activeCol, colC, step(1.0, color_mode) * (1.0 - checker));
    
    // Center focal geometry (large HUD rings)
    float centerR = length(uv);
    float centerRing1 = step(abs(centerR - 0.35 - 0.05 * bass), 0.015);
    float centerRing2 = step(abs(centerR - 0.65), 0.01);
    
    // Combine shape edges, safe flash inverted zones, and center rings
    vec3 col = activeCol * shape * (0.15 + hitHi * 1.4 + u_mid * 0.8);
    col += vec3(0.9, 0.95, 1.0) * (centerRing1 + centerRing2) * (0.2 + bass * 1.6);
    
    // Invert cell flashes safely
    if (checker > 0.5) {
        col += activeCol * safeFlash * 0.5 * (1.0 - shape);
    } else {
        col = mix(col, vec3(1.0) - col * 0.7, safeFlash * 0.4);
    }
    
    // Hard vignette
    col *= smoothstep(1.3, 0.3, centerR);
    
    // Hard photosafety clamp on final output luminance
    col = clamp(col, 0.0, 0.92);
    
    gl_FragColor = vec4(col, 1.0);
}
