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
vec2 cmpmul(vec2 a, vec2 b) {
    float real = a.x*b.x - a.y*b.y;
    float imag = a.x*b.y + a.y*b.x;
    return vec2(real, imag);
}
vec2 cmpdiv(vec2 a, vec2 b) {
    float magsq = (b.x*b.x + b.y+b.y);
    float real = a.x*b.x + a.y*b.y;
    float imag = a.y*b.x - a.x*b.y;
    return vec2(real, imag) / magsq;
}


void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    float N = 2.7;
    vec2 root1 = vec2(.5,-1);
    vec2 root2 = vec2(-1,1);
    vec2 root3 = 7. *(iMouse.xy - .5*vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).x;
    
    vec2 x = 8. *(fragCoord.xy - .5*vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).x;
    
    for(int i = 0; i < 1000; i ++) {
        vec2 a = x - root1;
        vec2 b = x - root2;
        vec2 c = x - root3;
        vec2 f = cmpmul(cmpmul(a, b), c);
        vec2 dxdf = cmpmul(a, b + c - 2.*x) + cmpmul(b, c - 2.*x) + cmpmul(x, 3.*x - 2.*c);
        
        // Newton's Method Iteration
        x -=  cmpdiv(f, dxdf);
        if(distance(x,vec2(0,0)) > N){
            //fragColor = vec4(d1, d2, d3,1) / distance(x,vec2(0,0)); // (max3(d1,d2,d3));
            break;
        }
    }
    
    float d1 = distance(x, root1);
    float d2 = distance(x, root2);
    float d3 = distance(x, root3);
    float m = max(max(d1, d2),d3);
    vec3 col = 1.-1.2*vec3(0.7*tanh(0.9/(d1)), .1+.5*tanh(0.9/(d3)) , .6*tanh(1./(d2)));
    
     
    fragColor = vec4(col,1.0);
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
