/*{
    "DESCRIPTION": "Laser-style audio waveform oscilloscope beam",
    "CATEGORIES": ["Generator", "Oscilloscope", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "beam_thickness",
            "TYPE": "float",
            "DEFAULT": 0.015,
            "MIN": 0.005,
            "MAX": 0.05
        },
        {
            "NAME": "glow",
            "TYPE": "float",
            "DEFAULT": 2.0,
            "MIN": 0.5,
            "MAX": 4.0
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
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / u_resolution.y;

    // Laser sine wave synthesis mimicking live PCM waveform
    float wave = sin(uv.x * 12.0 + u_time * 8.0) * (0.08 + u_bass * 0.15);
    wave += sin(uv.x * 28.0 - u_time * 14.0) * (0.04 + u_mid * 0.08);
    wave += sin(uv.x * 64.0 + u_time * 20.0) * (0.02 + u_treble * 0.05);

    float dist = abs(uv.y - wave);
    float beam = smoothstep(beam_thickness, 0.0, dist);
    float halo = (0.008 / (dist + 0.008)) * glow;

    vec3 beam_col = mix(vec3(0.0, 1.0, 0.8), vec3(1.0, 0.2, 0.6), abs(uv.x) * 2.0);
    vec3 col = beam_col * (beam + halo * (1.0 + u_onset));

    gl_FragColor = vec4(col, length(col) > 0.05 ? 1.0 : 0.0);
}
