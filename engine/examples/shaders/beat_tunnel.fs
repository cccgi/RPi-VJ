/*{
    "DESCRIPTION": "Beat-Gated 2D Polar Tunnel with Ring Emission & Bar-Synchronized Rotation",
    "CATEGORIES": ["2D", "Tunnel", "Audio-Reactive", "Geometric"],
    "INPUTS": [
        {
            "NAME": "speed",
            "TYPE": "float",
            "DEFAULT": 1.5,
            "MIN": 0.1,
            "MAX": 4.0
        },
        {
            "NAME": "sides",
            "TYPE": "float",
            "DEFAULT": 6.0,
            "MIN": 3.0,
            "MAX": 12.0
        },
        {
            "NAME": "kick_response",
            "TYPE": "float",
            "DEFAULT": 1.2,
            "MIN": 0.0,
            "MAX": 3.0
        },
        {
            "NAME": "color_shift",
            "TYPE": "float",
            "DEFAULT": 0.0,
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
uniform float u_sub;
uniform float u_hit_low;
uniform float u_hit_mid;
uniform float u_hit_high;

// Compatibility aliases for camelCase uniforms
uniform float uBass;
uniform float uKick;
uniform float uOnset;
uniform float uEnergy;
uniform float uBeatPhase;
uniform float uBarPhase;

void main() {
    vec2 res = u_resolution;
    vec2 p = (gl_FragCoord.xy - 0.5 * res) / min(res.x, res.y);
    
    float bass = max(u_bass, uBass);
    float kick = max(max(u_kick, uKick), max(u_onset, uOnset));
    float energy = max(u_energy, u_volume);
    float hitHi = max(u_hit_high, u_treble);
    // Beat/bar phase is only trustworthy while the tempo clock is locked.
    // Fade it out otherwise so the tunnel is never driven by a free-running
    // clock that has nothing to do with the music.
    float lock = clamp(u_locked, 0.0, 1.0);
    float bar = max(u_bar_phase, uBarPhase) * lock;
    float beat = max(u_beat_phase, uBeatPhase) * lock;
    
    // Bar-synchronized discrete & continuous rotation
    float barSnap = floor(bar * 4.0) * 0.392699;
    float rotAngle = u_time * 0.2 + barSnap + bar * 0.5;
    float cosA = cos(rotAngle);
    float sinA = sin(rotAngle);
    p = vec2(p.x * cosA - p.y * sinA, p.x * sinA + p.y * cosA);
    
    float r = length(p);
    float angle = atan(p.y, p.x);

    // Bass punch: the whole tunnel mouth breathes with the low end.
    r *= 1.0 - bass * 0.28 - kick * 0.12;
    
    // Polygonal tunnel shape modulation (n sides)
    float seg = 6.28318530718 / max(sides, 3.0);
    float polyMod = cos(mod(angle + 0.5 * seg, seg) - 0.5 * seg);
    r *= mix(1.0, polyMod, 0.45);
    
    // Depth mapping
    float depth = 1.0 / (r + 0.035);
    float tunnelZ = depth * 0.6 - (u_time * speed * 2.0 + beat * 0.5);
    
    // Longitudinal stripes & ring structure
    float stripeAngle = angle * (sides * 0.5) / 3.14159265;
    float stripes = smoothstep(0.15, 0.35, abs(fract(stripeAngle) - 0.5));
    stripes *= 0.35 + hitHi * 1.2 + u_mid * 0.6;
    
    // Rings along Z axis
    float ringCoord = fract(tunnelZ);
    // Ring width opens up with energy: near-silence gives thin dim threads,
    // a loud passage gives fat blazing hoops.
    float ringW = 0.035 + energy * 0.14 + kick * 0.06;
    float ringPulse = smoothstep(ringW, 0.0, abs(ringCoord - 0.5));
    ringPulse *= 0.25 + energy * 1.5;
    
    // Ring emission on kick
    float kickWave = sin(depth * 2.5 - u_time * 6.0 - kick * 3.0) * 0.5 + 0.5;
    float kickEmission = kick * kick_response * smoothstep(0.4, 1.0, kickWave) * min(r * 2.5, 1.2);
    
    // Neon tunnel coloring
    vec3 colCyan = vec3(0.0, 0.85, 1.0);
    vec3 colMagenta = vec3(1.0, 0.08, 0.55);
    vec3 colAmber = vec3(1.0, 0.6, 0.1);
    
    float hueMix = fract(tunnelZ * 0.1 + color_shift + bar * 0.25);
    vec3 baseCol = mix(colCyan, colMagenta, smoothstep(0.2, 0.8, hueMix));
    baseCol = mix(baseCol, colAmber, bass * 0.6);
    
    // Lighting & fog
    vec3 col = baseCol * (stripes * 0.6 + 0.4) * (ringPulse * 1.5 + 0.3);
    col += vec3(0.9, 0.95, 1.0) * ringPulse * (0.8 + bass * 0.8);
    col += colMagenta * kickEmission * 1.5;
    
    // Depth fog towards center
    float fog = smoothstep(0.0, 0.45, r);
    col *= fog;
    
    // Vignette
    col *= smoothstep(1.4, 0.4, r);

    // Master audio gain: dark at silence, blazing at full.
    col *= 0.12 + energy * 1.5 + kick * 0.5;
    
    gl_FragColor = vec4(col, 1.0);
}
