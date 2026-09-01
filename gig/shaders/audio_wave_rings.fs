/*{
    "DESCRIPTION": "Concentric wave rings that expand on transients, ripple per band and leave decaying wakes.",
    "CATEGORIES": ["Generator", "Audio-Reactive", "Multiband"],
    "INPUTS": [
        { "NAME": "ring_density", "TYPE": "float", "DEFAULT": 14.0, "MIN": 5.0, "MAX": 30.0 },
        { "NAME": "expand", "TYPE": "float", "DEFAULT": 1.0, "MIN": 0.2, "MAX": 2.5 }
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
uniform float u_volume;

void main(){
    vec2 res = u_resolution;
    vec2 uv = (gl_FragCoord.xy * 2.0 - res) / res.y;
    float r = length(uv);
    float a = atan(uv.y, uv.x);

    vec3 col = vec3(0.0);

    // THREE independent wavefronts, one per frequency zone. Each expands
    // outward on its own transient and decays, so the surface is genuinely
    // busy instead of one ring pumping on the kick.
    float bands[3];
    bands[0] = u_sub + u_bass;
    bands[1] = u_low_mid + u_high_mid;
    bands[2] = u_treble;
    float hits[3];
    hits[0] = u_hit_low; hits[1] = u_hit_mid; hits[2] = u_hit_high;
    vec3 tints[3];
    tints[0] = vec3(1.00, 0.25, 0.42);
    tints[1] = vec3(0.20, 1.00, 0.70);
    tints[2] = vec3(0.45, 0.65, 1.00);

    for (int i = 0; i < 3; i++) {
        float fi = float(i);
        float lvl = clamp(bands[i], 0.0, 1.5);
        float hit = hits[i];

        // travelling wave: phase advances with time and jumps on a transient
        float speed = 0.35 + fi * 0.22 + (1.0 - exp(-lvl * 1.6)) * 1.0;
        float phase = u_time * speed * expand + hit * 1.8;

        // integer angular ripple keeps the wrap seamless
        float seg = 5.0 + fi * 6.0;
        float ripple = 1.0 + 0.22 * lvl * cos(a * seg + u_time * (0.4 + fi * 0.3));

        float w = sin((r * ripple) * ring_density - phase * 3.0);
        // sharpen the crests as the band gets louder
        float crest = pow(max(w, 0.0), mix(6.0, 1.6, clamp(lvl, 0.0, 1.0)));

        // radial envelope: the wavefront lives in a moving shell
        float shell = exp(-abs(r - fract(phase * 0.25) * 1.7) * 2.2);

        // Brightness saturates early (full by ~0.45) so this reads well in
        // the 30-60% fader range; motion keeps scaling above that.
        float briL = clamp(lvl * 2.2, 0.0, 1.0);
        col += tints[i] * crest * (0.14 + briL * 1.5) * (0.35 + shell * 1.1);

        // transient throws a bright expanding ring
        if (hit > 0.02) {
            float rr = hit * 1.4;
            col += tints[i] * exp(-abs(r - (1.5 - rr)) * 16.0) * hit * 0.9;
        }
    }

    // centre bloom tied to overall level
    col += vec3(0.6, 0.75, 1.0) * (0.02 + u_volume * 0.25) / (r * 5.0 + 0.5);
    col *= 1.0 - smoothstep(1.15, 1.9, r);

    gl_FragColor = vec4(col, 1.0);
}
