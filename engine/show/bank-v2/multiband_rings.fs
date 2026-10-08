/*{
    "DESCRIPTION": "Five concentric ring zones, each driven by its own frequency band. Transients burst independently.",
    "CATEGORIES": ["Generator", "Audio-Reactive", "Multiband"],
    "INPUTS": [
        { "NAME": "ring_count", "TYPE": "float", "DEFAULT": 5.0, "MIN": 3.0, "MAX": 9.0 },
        { "NAME": "spin", "TYPE": "float", "DEFAULT": 1.0, "MIN": 0.0, "MAX": 3.0 }
    ]
}*/

uniform float u_time;
uniform vec2  u_resolution;
uniform float u_sub;
uniform float u_bass;
uniform float u_low_mid;
uniform float u_high_mid;
uniform float u_treble;
uniform float u_hit_low;
uniform float u_hit_mid;
uniform float u_hit_high;
uniform float u_bar_phase;
uniform float u_spec_flow[32];   // per-bin integrated clocks: a bin only
                                 // advances while that frequency is loud

// Each band owns a radial zone. Layering bands across space -- rather than
// summing them into one brightness -- is what makes a mix read as "alive"
// instead of "pumping on the kick".
void main() {
    vec2 uv = (gl_FragCoord.xy * 2.0 - u_resolution) / u_resolution.y;
    float r = length(uv);
    float a = atan(uv.y, uv.x);

    float bands[5];
    bands[0] = u_sub; bands[1] = u_bass; bands[2] = u_low_mid;
    bands[3] = u_high_mid; bands[4] = u_treble;

    float hits[5];
    hits[0] = u_hit_low; hits[1] = u_hit_low; hits[2] = u_hit_mid;
    hits[3] = u_hit_mid; hits[4] = u_hit_high;

    vec3 tint[5];
    tint[0] = vec3(1.00, 0.15, 0.35);
    tint[1] = vec3(1.00, 0.45, 0.10);
    tint[2] = vec3(0.20, 1.00, 0.55);
    tint[3] = vec3(0.25, 0.65, 1.00);
    tint[4] = vec3(0.75, 0.35, 1.00);

    vec3 col = vec3(0.0);
    for (int i = 0; i < 5; i++) {
        float fi = float(i);
        float lvl = bands[i];
        float hit = hits[i];

        // zone radius grows outward; transient pushes the zone outward briefly
        float rc = 0.22 + fi * 0.20 + hit * 0.06;
        float w  = 0.035 + lvl * 0.055;

        // Each ring's ROTATION SPEED is driven by its own band, so a spike in
        // one part of the spectrum races that ring ahead while the others
        // hold. Uniform motion is what made this read as a single organism.
        float seg = 6.0 + fi * 4.0;
        float dir = mod(fi, 2.0) < 1.0 ? 1.0 : -1.0;

        // WAS: ang = a*seg + dir * u_time * spin * spd, with spd floored near
        // 0.25 -- so every ring span constantly and fast, and changing spd
        // made the ring jump rather than accelerate. Now each ring rides the
        // integrated clock of ITS OWN six spectrum bins, which advances only
        // while that band is actually loud. A spike lurches one ring forward;
        // the rest hold still.
        // Two representative bins per band, not six: the six-tap average cost
        // this shader its 1080p verdict -- 30 dynamic uniform-array reads
        // per pixel -- and the extra taps changed the motion imperceptibly.
        int b0 = i * 6 + 1;
        int b1 = i * 6 + 4;
        float bandFlow = 0.5 * (u_spec_flow[b0 > 31 ? 31 : b0]
                              + u_spec_flow[b1 > 31 ? 31 : b1]);

        float ang = a * seg + dir * spin * (bandFlow + hit * 1.2);
        float teeth = 0.55 + 0.45 * cos(ang);
        // and its band also sharpens the teeth, so a loud band looks busier
        teeth = pow(teeth, 1.0 + lvl * 2.0);

        float ring = smoothstep(w, 0.0, abs(r - rc)) * teeth;
        float briL = clamp(lvl * 1.55, 0.0, 1.0);    // full brightness by ~0.65
        col += tint[i] * ring * (0.25 + briL * 2.6 + hit * 1.6);
    }

    // subtle centre glow tied to overall low end
    col += vec3(0.4, 0.5, 1.0) * (0.05 + u_sub * 0.35) / (r * 6.0 + 0.6);
    col *= 1.0 - smoothstep(1.1, 1.9, r);

    gl_FragColor = vec4(col, 1.0);
}
