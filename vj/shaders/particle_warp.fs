/*{
    "DESCRIPTION": "Hyperspace particle warp field with kick transient bursts",
    "CATEGORIES": ["Generator", "Space", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "warp_speed",
            "TYPE": "float",
            "DEFAULT": 2.0,
            "MIN": 0.5,
            "MAX": 6.0
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

    float r = length(uv);
    float a = atan(uv.y, uv.x);

    float speed = u_time * (warp_speed + u_bass * 4.0);
    vec3 col = vec3(0.0);

    for (float i = 1.0; i <= 3.0; i += 1.0) {
        float layer_z = fract(speed * 0.2 + i * 0.33);
        float size = (1.0 - layer_z);
        vec2 p = vec2(cos(a * 4.0 + i), sin(a * 4.0 + i)) * (r / (layer_z + 0.01));

        float star = smoothstep(0.08 * (1.0 + u_treble), 0.0, length(fract(p) - 0.5));
        vec3 star_color = mix(vec3(0.2, 0.6, 1.0), vec3(1.0, 0.4, 0.8), fract(i * 0.7));
        col += star_color * star * size;
    }

    // Center focal streak and kick strobe flash
    col += vec3(u_onset * 0.5);

    gl_FragColor = vec4(col, length(col) > 0.05 ? 1.0 : 0.0);
}
