/*{
    "DESCRIPTION": "5-band audio spectrum visualizer equalizer bars",
    "CATEGORIES": ["Generator", "Equalizer", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "bar_width",
            "TYPE": "float",
            "DEFAULT": 0.12,
            "MIN": 0.05,
            "MAX": 0.2
        },
        {
            "NAME": "glow",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.2,
            "MAX": 2.5
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
    vec2 uv = gl_FragCoord.xy / u_resolution.xy; // 0..1 coordinates

    // 5 bars mapped across X axis
    float heights[5];
    heights[0] = u_bass * 0.9 + u_onset * 0.3;     // Sub-bass
    heights[1] = u_bass * 0.7 + u_mid * 0.3;       // Bass
    heights[2] = u_mid * 0.85;                     // Mid
    heights[3] = u_mid * 0.4 + u_treble * 0.6;     // High-mid
    heights[4] = u_treble * 0.95;                  // Treble / High

    vec3 col = vec3(0.0);

    for (int i = 0; i < 5; i++) {
        float center_x = 0.15 + float(i) * 0.175;
        float dist_x = abs(uv.x - center_x);

        if (dist_x < bar_width * 0.5) {
            float h = clamp(heights[i], 0.05, 0.95);
            if (uv.y < h) {
                // Gradient from green at bottom to yellow/red at top
                vec3 bar_col = mix(vec3(0.0, 1.0, 0.6), vec3(1.0, 0.2, 0.4), uv.y / h);
                col = bar_col * glow;
            }
        }
    }

    gl_FragColor = vec4(col, length(col) > 0.1 ? 1.0 : 0.0);
}
