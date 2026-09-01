/*{
    "DESCRIPTION": "Concentric geometric audio wave rings",
    "CATEGORIES": ["Generator", "Geometric", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "ring_count",
            "TYPE": "float",
            "DEFAULT": 8.0,
            "MIN": 2.0,
            "MAX": 20.0
        },
        {
            "NAME": "audio_reactivity",
            "TYPE": "float",
            "DEFAULT": 1.2,
            "MIN": 0.0,
            "MAX": 3.0
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
    float dist = length(uv);

    vec3 final_color = vec3(0.0);

    for (float i = 1.0; i <= 8.0; i += 1.0) {
        float radius = i * 0.08 + (u_bass * 0.04 * audio_reactivity);
        float pulse = sin(u_time * 2.0 + i * 0.5) * 0.01;
        float ring_thickness = 0.005 + (u_onset * 0.008);

        float edge = abs(dist - radius - pulse);
        float ring_alpha = smoothstep(ring_thickness, 0.0, edge);

        vec3 ring_color = vec3(
            0.5 + 0.5 * sin(i * 0.7 + u_time + u_bass),
            0.5 + 0.5 * cos(i * 0.5 + u_time * 0.8 + u_mid),
            0.8 + 0.2 * sin(i * 1.1 + u_treble)
        );

        final_color += ring_color * ring_alpha;
    }

    // Add subtle ambient kick pulse
    final_color += vec3(u_onset * 0.2);

    gl_FragColor = vec4(final_color, max(max(final_color.r, final_color.g), final_color.b));
}
