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
    // Bass now LAUNCHES material rather than just accelerating everything.
    // Pure speed scaling reads as chaos at high level; discrete comets read
    // as impact.
    // WAS BROKEN: _drive referenced _speedL one line before _speedL was
    // declared -- a hard GLSL compile error, so this shader rendered nothing
    // at all. _drive comes from the low end, as intended.
    float _drive = clamp(u_bass * 1.61 + u_onset * 1.2, 0.0, 2.0);
    float _speedL = 1.0 - exp(-_drive * 1.2);      // saturates: no ludicrous mode

    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / u_resolution.y;

    float r = length(uv);
    float a = atan(uv.y, uv.x);

    float speed = u_time * warp_speed * 0.25 + u_flow_low * 3.2;
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
    // ---- BASS COMETS ------------------------------------------------------
    {
        vec2 _p = (gl_FragCoord.xy * 2.0 - u_resolution) / u_resolution.y;
        float _r = length(_p);
        float _a = atan(_p.y, _p.x);
        for (int _i = 0; _i < 4; _i++) {
            float _fi = float(_i);
            // each comet keeps its own launch epoch and direction
            float _seed = floor(u_flow_low * 1.1 + _fi * 13.0);
            float _dir = fract(sin(_seed * 12.9898 + _fi * 78.233) * 43758.5453) * 6.2831853;
            float _age = fract(u_flow_low * 1.1 + _fi * 13.0);
            float _reach = _age * (0.5 + _drive * 1.6);
            // angular proximity to this comet's heading
            // atan(sin,cos) is an expensive way to wrap an angle into -PI..PI;
            // mod does the same job far cheaper, and this ran once per comet
            // per pixel.
            float _da = abs(mod(_a - _dir + 3.14159265, 6.28318531) - 3.14159265);
            float _beam = exp(-_da * 14.0) * exp(-abs(_r - _reach) * 9.0);
            // head is fatter and brighter as bass rises
            float _size = 0.35 + _drive * 0.9;
            _beam += exp(-_da * 5.0) * exp(-abs(_r - _reach) * 26.0) * _size;
            vec3 _ct = mix(vec3(1.0, 0.45, 0.15), vec3(0.55, 0.75, 1.0), fract(_fi * 0.37));
            col += _ct * _beam * _drive * (1.0 - _age) * 1.5;
        }
    }


    gl_FragColor = vec4(col, length(col) > 0.05 ? 1.0 : 0.0);
}
