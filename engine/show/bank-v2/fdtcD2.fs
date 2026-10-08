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
float tricorn(vec2 z, vec2 c) {
    float l = 0.0;
    for (l = 0.0; l < 100.0; l += 1.0) {
        z = vec2(z.x * z.x - z.y * z.y, -2.0 * z.x * z.y) + c;
        if(dot(z, z) > 65536.0) break;
    }
    return l - log2(log2(dot(z,z))) + 4.0;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = ((2.0 * fragCoord - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).x) * 4.0;
    vec2 ms = ((2.0 * iMouse.xy - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).x) * 4.0;
    
    if (iMouse.xy == vec2(0.0)) ms = vec2(-1.5, -0.4);
   
    vec2 mnduv = uv + vec2(1.5, 0);
    vec2 julms = ms + vec2(1.5, 0);
    vec2 juluv = uv / 2.0 - vec2(1.0, 0);
    
    vec3 col = vec3(0);
    
    if (fragCoord.x > vec3(u_resolution, 1.0).x / 2.0) {
        float j = tricorn(juluv * 2.0, julms);
        col = vec3((j > 99.0) ? 0.0 : sin(j / 20.0)) * vec3(0.4, 1.0, 0.4);
    } else {
        float m = tricorn(vec2(0), mnduv);
        col = vec3((m > 99.0) ? 0.0 : sin(m / 20.0)) * vec3(0.4, 1.0, 0.4);
    }
    
    float p = length(uv - ms) - 0.025;
    
    if (p < 0.0) fragColor = vec4(vec3(1.0, 0.0, 0.0), 1.0);
    else fragColor = vec4(col, 1.0);
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
