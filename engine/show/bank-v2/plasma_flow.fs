/*{
    "DESCRIPTION": "Organic fluid liquid plasma reacting to bass and mids",
    "CATEGORIES": ["Generator", "Liquid", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "flow_speed",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.2,
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
    vec2 uv = gl_FragCoord.xy / u_resolution.xy * 3.0;

    float t = u_time * flow_speed;
    float v1 = sin(uv.x + t + u_bass * 2.0);
    float v2 = sin(uv.y + t * 0.8 + u_mid * 2.0);
    float v3 = sin(uv.x + uv.y + t * 1.2 + u_treble);

    float cx = uv.x + 0.5 * sin(t * 0.3) - 1.5;
    float cy = uv.y + 0.5 * cos(t * 0.2) - 1.5;
    float v4 = sin(sqrt(cx * cx + cy * cy + 1.0) + t * 1.5);

    float v = v1 + v2 + v3 + v4;

    vec3 col = vec3(
        sin(v * 3.1415 + u_bass * 2.0) * 0.5 + 0.5,
        cos(v * 3.1415 + u_mid * 2.0) * 0.5 + 0.5,
        sin(v * 3.1415 + 1.5 + u_treble) * 0.5 + 0.5
    );

    col += vec3(u_onset * 0.25);

    gl_FragColor = vec4(col, 1.0);
}
