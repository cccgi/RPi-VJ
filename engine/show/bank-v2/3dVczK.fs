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
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    vec3 color = vec3(0.0);
    uv *= 1.25;

    uv.x = abs(uv.x);
    uv.y -= 0.28867513459;
    vec2 n = vec2(sin(2.61799387799), cos(2.61799387799));
    uv -= n * max(0.0, dot(uv - vec2(0.5, 0.0), n)) * 2.0;

    n = vec2(sin(2.09439510239), cos(2.09439510239));
    float scale = 1.0;
    uv.x += 0.5;
    for (int iters=0; iters < 10; iters++) {
        uv *= 3.0;
        scale *= 3.0;
        uv.x -= 1.5;

        uv.x = abs(uv.x);
        uv.x -= 0.5;
        uv -= n * min(0.0, dot(uv, n)) * 2.0;
    }

    color = texture(iChannel0, (uv / scale) * 2.0 + u_time * 0.05).rgb;
    //float distLine = length(uv - vec2(clamp(uv.x, -1.0, 1.0), 0.0));
    //color = vec3(smoothstep(1.0 / vec3(u_resolution, 1.0).y, 0.0, distLine / scale));

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
