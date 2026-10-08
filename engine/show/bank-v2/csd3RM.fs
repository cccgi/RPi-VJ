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
vec2 N(float angle){
    return vec2(sin(angle), cos(angle));
}

vec3 fractal(vec2 uv, float a, float b, float dist){
    uv.x = abs(uv.x);
    uv.y += tan(5./6. * 3.1415) * 0.5;
    
    vec2 n = N(a * 3.1415);
    float d = dot(uv-vec2(.5, 0.), n);
    uv -= n*max(0., d) * 2.;
    
    
    n = N(b * 3.1415);
    float scale = 1.;
    
    uv.x += 0.5;
    for(int i=0;i<4;i++){
        uv*=3.;
        scale *=3.;
        uv.x-=1.5;
        
        uv.x = abs(uv.x);
        uv.x -= dist;
        uv -= n*min(0., dot(uv, n)) * 2.;
   } 
   
   return vec3(uv, scale);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = (fragCoord - 0.5*vec3(u_resolution, 1.0).xy)/vec3(u_resolution, 1.0).y;
    vec3 col = vec3(0.);
    vec2 mouse = iMouse.xy/vec3(u_resolution, 1.0).xy;
    uv*=1.2;
   
   vec3 f1 = fractal(uv, 5.5/6.7, 2./3.3, 1.);
   vec3 f2 = fractal(uv, 5./6., 2./3., 0.5);
   
   float d1 = length(f1.xy-vec2(clamp(f1.x, -1., 1.), 0.));
   float d2 = length(f2.xy-vec2(clamp(f2.x, -1., 1.), 0.));
   
   
   float s = mix(d1, d2, cos(u_time/0.5));
   
   //col += smoothstep(0.01, 0., s/f1.z);
   
   f1.xy /=f1.z;
   f1.xy /= 2.;
   col += texture(iChannel0, f1.xy*2.5 + u_time*0.05).rgb;

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
