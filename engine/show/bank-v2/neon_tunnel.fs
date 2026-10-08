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
    // WAS: u_time * (speed + u_bass * 5.5). Scaling u_time makes the whole
    // tunnel jump whenever the level changes, so in practice this had to stay
    // tiny and the shader only ever changed BRIGHTNESS. u_flow_low is a clock
    // whose RATE is the bass, so travel genuinely accelerates on the low end.
    float tunnel_speed = u_time * speed * 0.20 + u_flow_low * 2.4;
    vec2 tunnel_uv = vec2(a / 3.14159265 * 3.0, depth + tunnel_speed);

    // Audio-reactive geometric rings
    float rings = sin(tunnel_uv.y * (7.0 + u_mid * 9.0)) * cos(tunnel_uv.x * (4.0 + u_treble * 6.0));
    float glow = smoothstep(0.1 - u_bass * 0.30, 0.9 - u_bass * 0.35, rings + u_bass * 1.8) * glow_intensity * (1.0 + u_bass * 1.4);

    // Color gradient reacting to frequency bands
    vec3 col = vec3(0.0);
    col.r = sin(tunnel_uv.y * 2.0 + u_flow) * 0.5 + 0.5 + u_onset * 1.4;
    col.g = sin(tunnel_uv.y * 3.0 + u_flow_mid * 1.1 + u_mid) * 0.5 + 0.5;
    col.b = cos(tunnel_uv.y * 1.5 + u_flow_high * 0.9 + u_treble) * 0.5 + 0.5;

    // Center focal glow and strobe flash on kick
    col *= glow * (1.0 / (r * 3.0 + 0.2));
    col += vec3(u_onset * 0.4);

    gl_FragColor = vec4(col, 1.0);
}
