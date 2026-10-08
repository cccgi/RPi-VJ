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
//2D hash function:
vec2 hash2(vec2 p)
{
 	return fract(cos(p*mat2(56.,37.,81.,-26.))*28.9);   
}
//Simple Worley noise function:
float worley(vec2 p)
{
    vec3 s = vec3(-1,0,1);
 	vec2 f = floor(p);
    
    vec2 v = hash2(f+s.xx)-p+f+s.xx;
    float d = dot(v,v);
    v = hash2(f+s.yx)-p+f+s.yx;
    d = min(d,dot(v,v));
    v = hash2(f+s.zx)-p+f+s.zx;
    d = min(d,dot(v,v));
    
    v = hash2(f+s.xy)-p+f+s.xy;
    d = min(d,dot(v,v));
    v = hash2(f+s.yy)-p+f+s.yy;
    d = min(d,dot(v,v));
    v = hash2(f+s.zy)-p+f+s.zy;
    d = min(d,dot(v,v));

    v = hash2(f+s.xz)-p+f+s.xz;
    d = min(d,dot(v,v));
    v = hash2(f+s.yz)-p+f+s.yz;
    d = min(d,dot(v,v));
    v = hash2(f+s.zz)-p+f+s.zz;
    d = min(d,dot(v,v));
    return sqrt(d);
}

void mainImage( out vec4 color, in vec2 coord)
{
    float noise = worley(coord/64.+u_time);
    color = vec4(noise,noise,noise,1.);
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
