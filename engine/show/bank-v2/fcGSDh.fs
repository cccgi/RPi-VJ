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
void mainImage(out vec4 O, vec2 U) {
    float p1Dur   = 32.0 * (60.0 / 138.0);
    float p2Dur   = 32.0 * (60.0 / 138.0);
    float p2bDur  = 16.0 * (60.0 / 138.0);
    float pBrkDur = 12.0 * (60.0 / 69.0);
    float pXfdDur =  4.0 * (60.0 / 69.0);
    float p3Dur   = 32.0 * (60.0 / 120.0);
    float p4Dur   = 32.0 * (60.0 / 138.0);
    float p5Dur   = 36.0 * (60.0 / 138.0);
    float p6Dur   =  4.0 * (60.0 / 138.0);

    float t1       = p1Dur;
    float t2       = t1 + p2Dur;
    float t2b      = t2 + p2bDur;
    float tBrk     = t2b + pBrkDur;
    float tXfd     = tBrk + pXfdDur;
    float tMainEnd = tXfd + p3Dur + p4Dur + p5Dur;
    float tEnd     = tMainEnd + p6Dur;

    float time = mod(u_time, tEnd);

    float zoom = 2.5;
    float xOffset = 0.5;

    if (time < t1) {
        zoom = 2.5;
        xOffset = 0.5;
    } 
    else if (time < t2) {
        zoom = 1.0;
        xOffset = 0.711;
    } 
    else if (time < t2b) {
        float progress = (time - t2) / p2bDur;
        zoom = mix(0.5, 4.0, progress);
        xOffset = mix(2.0, 0.5, progress);
    } 
    else if (time < tXfd) {
        float progress = (time - t2b) / (tXfd - t2b);
        zoom = mix(4.0, 99999999999999.0, pow(progress, 5.0));
        xOffset = 0.5;
    } 
    else if (time < tMainEnd) {
        float progress = (time - tXfd) / (tMainEnd - tXfd);
        float rawZoom = mix(0.01, 0.75, progress);
        zoom = min(rawZoom, 0.75); 
        xOffset = 0.5;
    } 
    else {
        float progress = clamp((time - tMainEnd) / p6Dur, 0.0, 1.0);
        zoom = mix(0.75, 2.25, smoothstep(0.0, 1.0, progress));
        xOffset = 0.5;
    }

    // 
    vec3 p = vec3((U - xOffset * vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y * zoom, sin(time * 0.2));
    
    float i = 0.0, a = 2.5029, d = 4.6692;

    for (; i < 16.0; i++) {
        p = abs(p);
        if (p.x < p.y) p.xy = p.yx;
        if (p.x < p.z) p.xz = p.zx;
        if (p.y < p.z) p.yz = p.zy;
        
        p = p * 3.0 - 2.0;
        if (p.z < -1.0) p.z += 2.0;
    }

    vec2 z = p.xy;
    float r = log(length(z) * d + 1.0) / log(d),
          theta = atan(z.y, z.x) * a;

    vec3 col = vec3(
        sin(i * 0.3 + r), 
        cos(theta - r) * 0.5 + 0.5, 
        sin(i * 0.15 + theta)
    );

       float border = smoothstep(0.05, 0.1, abs(dot(col, vec3(0.35, 0.25, -0.40))));
    O = 1.0 - vec4(vec3(pow(border, 12.0)), 1.0);
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
