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
float sdCircle(vec2 p, float radius) {
    return length(p) - radius;
}

vec3 palette(float t) {
    vec3 a = vec3(0.5, 0.5, 0.5);
    vec3 b = vec3(0.5, 0.5, 0.5);
    vec3 c = vec3(1.0, 1.0, 1.0);
    vec3 d = vec3(.263, .416, .557);
    
    return a + b*cos(6.283185*(c*t + d));
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Normalization
    //vec2 uv = vec2(fragCoord.x / vec3(u_resolution, 1.0).x, fragCoord.y / vec3(u_resolution, 1.0).y);
    //vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy;
    
    //vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy * 2.0 - 1.0;
    //uv.x *= vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y;
    vec2 uv = (fragCoord * 2.0 - vec3(u_resolution, 1.0).xy)/  vec3(u_resolution, 1.0).y;
    vec2 uv0 = uv;
    vec3 finalColor = vec3(0.0);
    
    for (float i = 0.0; i < 4.0; i++) {
        uv = fract(1.5 * uv) - 0.5;

        //float d = sdCircle(uv, 0.5);
        float d = length(uv) * exp(-length(uv0));

        //vec3 col = vec3(1.0, 2.0, 3.0);
        vec3 col = palette(length(uv0) + i*.4 + u_time * .4);

        //d -= 0.5;
        d = sin(d*8. + u_time)/20.;

        d = abs(d);

        //d = step(0.1, d);
        d = smoothstep(0.0, 0.1, d);

        d = pow(0.001 / d, 1.2);

        finalColor += col * d;
    }

    fragColor = vec4(finalColor, 1.0);
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
