/* {"LABEL":"Shadertoy native"} */
#define HW_PERFORMANCE 1
precision highp float;
precision highp int;
#define iFrameRate 60.0
#define iDate vec4(2025.0, 1.0, 1.0, 0.0)
uniform vec4 iMouse;
uniform sampler2D iChannel0;
uniform sampler2D iChannel1;
uniform sampler2D iChannel2;
uniform sampler2D iChannel3;
uniform float iChannelTime[4];
uniform vec3 iChannelResolution[4];
const vec3 BLACK = vec3(0.0, 0.0, 0.0);
const vec3 WHITE = vec3(1.0, 1.0, 1.0);
const vec3 RED = vec3(1.0, 0.0, 0.0);
const vec3 GREEN = vec3(0.0, 1.0, 0.0);
const vec3 BLUE = vec3(0.0, 0.0, 1.0);
const vec3 BACKGROUND = BLACK;

const float MAX_ITERATIONS = 10.0;
const float PI = 3.14159265358979323846;
const float INF = 1e6;
const float RADIUS = 0.015;

vec3 hsv2rgb(vec3 c) {
    vec3 k = vec3(1.0, 2.0 / 3.0, 1.0 / 3.0);
    vec3 p = abs(fract(c.xxx + k) * 6.0 - 3.0);
    return c.z * mix(k.xxx, clamp(p - k.xxx, 0.0, 1.0), c.y);
}

mat2 rotate2d(float angle) {
    float c = cos(angle), s = sin(angle);
    return mat2(c, -s, s, c);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord * 2.0 - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    float blur = 1.0 / vec3(u_resolution, 1.0).y;
    float scale = 1.0;

    int iterations = int(MAX_ITERATIONS * (1.0 - abs(2.0 * fract(0.1 * u_time) - 1.0)));
    for (int _ = 0; _ < iterations; _++) {
        if (uv.y >= 0.0) {
            scale *= 2.0, uv *= 2.0;
            uv.x = -uv.x, uv.y -= 0.75;
        } else if (((uv + vec2(0.0, 0.25)) * rotate2d(PI * 2.0 / 3.0)).y >= 0.25) {
            scale *= 2.0, uv *= 2.0;
            uv.x -= sqrt(3.0) / 2.0, uv.y += 0.75;

            uv.y += 0.25, uv *= rotate2d(PI * 2.0 / 3.0);
            uv.x = -uv.x;
            uv *= rotate2d(-PI * 2.0 / 3.0), uv.y -= 0.25;
        } else if (((uv + vec2(0.0, 0.25)) * rotate2d(-PI * 2.0 / 3.0)).y >= 0.25) {
            scale *= 2.0, uv *= 2.0;
            uv.x += sqrt(3.0) / 2.0, uv.y += 0.75;

            uv.y += 0.25, uv *= rotate2d(-PI * 2.0 / 3.0);
            uv.x = -uv.x;
            uv *= rotate2d(PI * 2.0 / 3.0), uv.y -= 0.25;
        }
    }

    float t = fract(u_time * 0.1);
    vec3 hues = vec3(t, fract(t + 0.33), fract(t + 0.66));
    const vec2 CENTOR[] =
        vec2[](vec2(0.0, 0.75), vec2(sqrt(3.0) / 2.0, -0.75), vec2(-sqrt(3.0) / 2.0, -0.75));
    vec3 color = BACKGROUND;
    for (int i = 0; i < 3; i++) {
        float mask = 1.0 - smoothstep(-blur, blur, length(uv - CENTOR[i]) / scale - RADIUS);
        vec3 col = hsv2rgb(vec3(hues[i], 0.8, 1.0));
        color = mix(color, col, mask);
    }
    fragColor = vec4(color, 1.0);
}

void main(){
    vec2 center = 0.5 * u_resolution;
    vec2 q = gl_FragCoord.xy - center;
    float audio_zoom = 1.0 + 0.28*u_bass + 0.18*u_beat_onset;
    float audio_angle = 0.35*u_mid + 0.12*sin(u_time*1.7)*u_treble;
    float audio_cs = cos(audio_angle), audio_sn = sin(audio_angle);
    q = mat2(audio_cs,-audio_sn,audio_sn,audio_cs) * q / audio_zoom;
    q += vec2(sin(q.y*0.018 + u_time*2.0), cos(q.x*0.014 - u_time*1.4))
         * (7.0*u_mid + 4.0*u_treble + 18.0*u_beat_onset);
    q += 3.0 * vec2(sin(u_time*5.0), cos(u_time*4.0)) * u_bass;
    q += center;
    vec4 c=vec4(0.0);
    mainImage(c, q);
    float e=clamp(0.65+0.55*u_volume+0.20*u_bass,0.25,1.45);
    c.rgb*=e;
    float luma = dot(clamp(c.rgb, vec3(0.0), vec3(1e9)), vec3(0.2126, 0.7152, 0.0722));
    float gain = mix(1.55, 0.95, smoothstep(0.16, 0.82, luma));
    c.rgb = clamp(pow(clamp(c.rgb, vec3(0.0), vec3(1e9)), vec3(0.70)) * gain, 0.0, 1.0);
    gl_FragColor=c;
}
