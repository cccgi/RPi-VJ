/*{
    "DESCRIPTION": "Retro synthwave 3D wireframe grid with audio-reactive terrain pulse",
    "CATEGORIES": ["Generator", "Synthwave", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "grid_speed",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.2,
            "MAX": 3.0
        },
        {
            "NAME": "mountain_height",
            "TYPE": "float",
            "DEFAULT": 1.5,
            "MIN": 0.5,
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
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / u_resolution.y;

    if (uv.y > 0.0) {
        // Horizon / Sky with audio sun
        float sun_dist = length(uv - vec2(0.0, 0.15));
        float sun = smoothstep(0.18 + u_bass * 0.05, 0.16, sun_dist);
        vec3 sun_col = mix(vec3(1.0, 0.1, 0.4), vec3(1.0, 0.8, 0.0), (uv.y - 0.15) / 0.18);
        sun_col += vec3(u_onset * 0.3);
        gl_FragColor = vec4(sun_col * sun, sun);
        return;
    }

    // 3D Perspective Plane
    float depth = -0.4 / uv.y;
    float x = uv.x * depth;
    float z = depth + u_time * (grid_speed + u_bass * 0.8);

    // Audio-reactive terrain elevation
    float terrain = sin(x * 1.5) * cos(z * 0.8) * (u_bass * mountain_height);

    // Grid lines
    vec2 grid_uv = fract(vec2(x, z + terrain)) - 0.5;
    float line_x = smoothstep(0.06, 0.0, abs(grid_uv.x));
    float line_z = smoothstep(0.06, 0.0, abs(grid_uv.y));
    float grid = max(line_x, line_z) * (1.0 / (depth * 0.4 + 1.0));

    vec3 col = mix(vec3(0.0, 0.8, 1.0), vec3(1.0, 0.1, 0.6), sin(z * 0.2 + u_time) * 0.5 + 0.5);
    col *= grid * (1.5 + u_mid);
    col += vec3(u_onset * 0.3);

    gl_FragColor = vec4(col, grid > 0.05 ? 1.0 : 0.0);
}
