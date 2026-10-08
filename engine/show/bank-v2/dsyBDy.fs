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
/*
    Some interesting patterns emerge when using mod twice

    Inspired by Martin Kleppe: X.com/aemkei/status/1378106731386040322
    
    Also see
    Alien Xor Carpet: shadertoy.com/view/dsVBRd
    
    
    <100 chars playlist: shadertoy.com/playlist/f3lGDN
*/

void mainImage( out vec4 O, in vec2 I )
{
    //Double mod
    O = vec4((int(I+=u_time*1e2)^int(I.y))%99%45<9);
    
    //Just 99
    //O = vec4((int(I+=u_time*1e2)^int(I.y))%99<9);
    
    //Just 45
    //O = vec4((int(I+=u_time*1e2)^int(I.y))%45<9);
    
    //Triple mod
    //O = vec4(4,5,3,0) - vec4((int(I+=u_time*1e2)^int(I.y))%99%45%31);
    
    //Animated
    //O = vec4(((int(I+=u_time*1e2)^int(I.y))+FRAMEINDEX/9)%233%97%45<9);
    
    //Blue Test
    //O = vec4(3,4,5,0) / vec4((int(I+=u_time*1e2)^int(I.y))%157%73%31)-1.;
    
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
