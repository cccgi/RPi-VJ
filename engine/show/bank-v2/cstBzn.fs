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
//Fabrice's version: https://shadertoy.com/view/DdyyWt
//Shane's original: https://shadertoy.com/view/cdycRt

//-11 Thanks to FabriceNeyret2
//-3 Thanks to coyote
void mainImage( out vec4 O, vec2 I )
{
    I -= O.xy = vec3(u_resolution, 1.0).xy * .5;
    I /= O.y;
    float W = 24.,
          a = atan(-I.x, I.y+=.5),
          v = length(I) * W,
          e = min(ceil(v), W);
        
    O = abs( cos( v/W/.4 + vec4(1,2,3,0) ) ) 
      * min((.4 - length(
                    vec2(  e*(clamp(a, 1.57*sin(u_time/.5+e/W*4.7), 1.57) 
                              - a )
                         , v-e+.5)
                        )
            )*O.y/35.
            ,1.);
}
//Original [269]
/*
void mainImage(out vec4 O, vec2 I)
{
    vec2 r = vec3(u_resolution, 1.0).xy,
    p = (I+I-r) / r.y;
    
    float N = 24.,
        a = atan(p.x,p.y+=.5)+1.57,
        l = length(p)*N,
        R = min(ceil(l),N);
        
    O = sqrt(cos(l/N*5.+vec4(2,4,6,0))+1.) * 
    min((.4-length(
        vec2(clamp(a*=R, 0., R*1.57*(1.-sin(u_time/.5+R/N*4.7)))-a, l-R+.5))
    )*r.y/1e2,.7);
}
*/
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
