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
#define PI 3.1415
#define N_ITER 4

mat2 rot2d(float a) {
    float s = sin(a), c = cos(a);
    return mat2(c,-s,s,c);
}

float sdSegment(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p-a, ba = b-a;
    float h = clamp(dot(pa,ba)/dot(ba,ba),0.,1.);
    return length(pa-ba*h);
}

float scene(inout vec2 uv) {
    vec2 m = iMouse.xy/vec3(u_resolution, 1.0).xy;
    
    float s = 1.;
    
    uv *= 1.3+m.y;
    
    float a = PI*5./6.-m.x*PI/12.;
    vec2 n = vec2(sin(a),cos(a));
    uv.x = abs(uv.x);
    uv.y += tan(a)*.5;
    float ax = dot(uv-vec2(.5,0),n);
    uv -= n*max(.0,ax)*2.;
    
    
    a = PI*2./3.+m.x*PI/3.;
    n = vec2(sin(a),cos(a));
    
    uv.x += .5;
    for (int i=0;i<N_ITER;i++) {
        uv *= 3.;
        uv.x -= 1.5;
        uv.x = abs(uv.x);
        uv.x -= .5;
        float ax = dot(uv,n);
        uv -= n*min(.0,ax)*2.;
    }
    
    s = min(s,sdSegment(uv,vec2(-1.,0),vec2(1.,0.))-.002*pow(3.,float(N_ITER)));
    
    return s;
}

vec3 paint(vec2 uv) {
    vec3 col = vec3(0);
    
    float s = scene(uv);
    
    // Koch fractal outline
    // col += smoothstep(1./vec3(u_resolution, 1.0).y,.0,s);
    
    col += texture(iChannel0,uv/pow(3.,float(N_ITER))-u_time*.02).rgb;
    
    return col;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord ) {
    vec2 uv = (fragCoord.xy-.5*vec3(u_resolution, 1.0).xy)/vec3(u_resolution, 1.0).y;
    uv *= rot2d(u_time*.1);
    
    vec3 col = vec3(0);
    
    col += paint(uv);
    
    fragColor = vec4(col,1.);
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
