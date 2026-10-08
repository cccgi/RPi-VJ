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
// Fork of "TheGuy888's Spiral Septagon" by TheGuy888. https://shadertoy.com/view/Nf2GRc
// 2026-03-25 14:56:56

bool isIn(vec2 z){return length(z)<3.0;}

vec2 complexMult(vec2 a,vec2 b){return vec2(a.x*b.x-a.y*b.y,dot(a.yx,b));}

vec2 complexDiv(vec2 a,vec2 b){return vec2(dot(a,b),a.y*b.x-a.x*b.y)/dot(b,b);}

vec2 complexPow(vec2 z,int p){
    vec2 z2=z;
    for (int i=1;i<p;++i){z2=complexMult(z,z2);}
    return z2;
    }
    
float pi = 3.14159;
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    float ITERATIONS = 50.0;
    float ZOOM;
    vec2 COORDS;
    if (iMouse.z>0.0){
    ZOOM =10.0;
    COORDS=2.*((2.0*iMouse.xy)-(vec3(u_resolution, 1.0).xy))/vec3(u_resolution, 1.0).x;
   }else{
    ZOOM =1.0;
    COORDS = vec2(0.,0.);
   }
  
    vec2 pixCoord = COORDS+4./ZOOM*((fragCoord.xy-vec3(u_resolution, 1.0).xy/2.)/vec3(u_resolution, 1.0).x);
    
    vec2 zPrev = vec2(0.);
    vec2 z = pixCoord;
    float i=0.0;
    for (i=0.0;i<ITERATIONS && isIn(z);++i){
    zPrev=z;
    z = complexDiv(complexPow(z,3)+vec2(-.5666,0.),z);
    }
    vec3 col = vec3(1.);
    if (!isIn(z)){
    float time = (19.25)/5.0;
    //i=i-log2(log(length(z)));
    i=i/4.0;
    col = abs(vec3(sin(i+time),sin(i+(time/3.0)),sin(i+(time/2.0))));
    i*=4.;
    col*=.8+.2*(1.-1./i)*normalize(z-zPrev).y;
    }
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
