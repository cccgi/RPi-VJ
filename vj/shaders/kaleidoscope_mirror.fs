/*{
    "DESCRIPTION": "8-fold kaleidoscopic mirror geometry pulsing to music",
    "CATEGORIES": ["Generator", "Kaleidoscope", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "segments",
            "TYPE": "float",
            "DEFAULT": 8.0,
            "MIN": 4.0,
            "MAX": 16.0
        },
        {
            "NAME": "zoom",
            "TYPE": "float",
            "DEFAULT": 2.0,
            "MIN": 0.5,
            "MAX": 5.0
        }
    ]
}*/

uniform float u_time;
uniform vec2  u_resolution;
uniform float u_bass;
uniform float u_mid;
uniform float u_treble;
uniform float u_onset;

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / min(u_resolution.x, u_resolution.y);

    float r = length(uv) * (zoom - u_bass * 0.5);
    float a = atan(uv.y, uv.x);

    // Segment reflection
    float seg = 6.2831853 / segments;
    a = mod(a, seg) - seg * 0.5;
    a = abs(a);

    vec2 p = vec2(cos(a), sin(a)) * r;
    p += vec2(sin(u_time * 0.5 + u_mid), cos(u_time * 0.4 + u_treble));

    // Audio-modulated fractal pattern
    float pattern = sin(p.x * 12.0) * cos(p.y * 12.0) + sin(r * 20.0 - u_time * 2.0);
    float glow = smoothstep(0.2, 0.8, pattern + u_bass);

    vec3 col = vec3(
        0.5 + 0.5 * sin(p.x * 5.0 + u_time + u_bass),
        0.5 + 0.5 * cos(p.y * 5.0 + u_time * 1.3 + u_mid),
        0.5 + 0.5 * sin(r * 8.0 + u_treble)
    );

    col *= glow * (1.2 + u_mid * 0.8);
    col += vec3(u_onset * 0.35);

    gl_FragColor = vec4(col, glow > 0.05 ? 1.0 : 0.0);
}
