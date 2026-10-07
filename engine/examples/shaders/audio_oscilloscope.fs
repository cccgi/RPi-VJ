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
    // Amplitude now scales with level: near-silent input gives a nearly flat
    // line instead of a full-swing trace on noise.
    float _amp = clamp(u_volume * 1.3 + u_bass * 0.5, 0.0, 1.3);
    float _gate = smoothstep(0.02, 0.22, _amp);

    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / u_resolution.y;

    // Laser sine wave synthesis mimicking live PCM waveform
    // Each harmonic now scrolls on ITS OWN band clock, so loud bass makes the
    // fundamental surge while quiet hats leave the fine ripple almost still.
    // Previously every term used u_time, so the oscillation ran at a fixed
    // rate and audio could only change amplitude.
    float wave = (sin(uv.x * (8.0 + u_bass * 10.0) + u_flow_low * 5.0) * (0.08 + u_bass * 0.15)) * (0.12 + _amp * 1.1) * _gate;
    wave += sin(uv.x * (20.0 + u_mid * 18.0) - u_flow_mid * 7.0) * (0.04 + u_mid * 0.08);
    wave += sin(uv.x * (48.0 + u_treble * 40.0) + u_flow_high * 9.0) * (0.02 + u_treble * 0.05);

    float dist = abs(uv.y - wave);
    float beam = smoothstep(beam_thickness, 0.0, dist);
    float halo = (0.008 / (dist + 0.008)) * glow;

    vec3 beam_col = mix(vec3(0.0, 1.0, 0.8), vec3(1.0, 0.2, 0.6), abs(uv.x) * 2.0);
    vec3 col = beam_col * (beam + halo * (1.0 + u_onset));

    gl_FragColor = vec4(col, length(col) > 0.05 ? 1.0 : 0.0);
}
