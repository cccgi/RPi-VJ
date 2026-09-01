/*{
    "DESCRIPTION": "Audio-reactive neon speed tunnel with kick-reactive strobe",
    "CATEGORIES": ["Generator", "Tunnel", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "speed",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.1,
            "MAX": 4.0
        },
        {
            "NAME": "glow_intensity",
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
uniform float u_volume;
uniform float u_onset;
uniform float u_beat_phase;

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / u_resolution.y;

    float r = length(uv);
    float a = atan(uv.y, uv.x);

    // Tunnel coordinates
    float depth = 1.0 / (r + 0.05);
    float tunnel_speed = u_time * (speed + u_bass * 1.5);
    vec2 tunnel_uv = vec2(a / 3.14159265 * 3.0, depth + tunnel_speed);

    // Audio-reactive geometric rings
    float rings = sin(tunnel_uv.y * 10.0) * cos(tunnel_uv.x * 6.0);
    float glow = smoothstep(0.1, 0.9, rings + u_bass * 0.8) * glow_intensity;

    // Color gradient reacting to frequency bands
    vec3 col = vec3(0.0);
    col.r = sin(tunnel_uv.y * 2.0 + u_time) * 0.5 + 0.5 + u_onset * 0.6;
    col.g = sin(tunnel_uv.y * 3.0 + u_time * 1.2 + u_mid) * 0.5 + 0.5;
    col.b = cos(tunnel_uv.y * 1.5 + u_time * 0.8 + u_treble) * 0.5 + 0.5;

    // Center focal glow and strobe flash on kick
    col *= glow * (1.0 / (r * 3.0 + 0.2));
    col += vec3(u_onset * 0.4);

    gl_FragColor = vec4(col, 1.0);
}
