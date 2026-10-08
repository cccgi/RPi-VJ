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
//Tweet: https://twitter.com/XorDev/status/1519343739419959297
//Twigl: https://t.co/FELzNSfU40
//Based on "Molecules 2": https://www.shadertoy.com/view/7llBzS

//<512 Chars playlist: shadertoy.com/playlist/N3SyzR
//-1 Thanks to FabriceNeyret2

void mainImage(out vec4 O, vec2 I)
{
    vec3 r = vec3(u_resolution, 1.0),
         c = vec3(0,2,1),
         T = u_time+c,
         P = (I+I-r.xy)/r.y * mat3x2(-7,4, 0,-8, 7,4) / 6. + .2;
    int  A = int(T.z)/2%3;
    O = min( (.08 - length(max(abs(fract(
                          P += ( T - sin(T*6.283)/6. ).x
                              * ceil( mod(T,3.) - 2. )
                              * cos( 3.14* ceil( P[int(T.y)%3] ) )
                          ) -.5) -.4, 0.))
             ) *.2*r.y
            , 1.)
        * ( .5 + .5* sin(dot( mod( ceil(P+vec3(A<1,A<2,A-1)), 2.) , c+7.) - c )).xzyz;
}
//Original [338]
/*
void mainImage(out vec4 O, vec2 I)
{
    vec3 r=vec3(u_resolution, 1.0),
    c = vec3(0,2,1),
    T=u_time+c,
    P=(I+I-r.xy)/r.y*mat3x2(-7,4, 0,-8, 7,4) / 6.+.2;
    int A=int(T.z)/2%3;
    O=min((.08-length(max(abs(fract(
    P+=(T-sin(T*6.283)/6.).x*floor(mod(T,3.)-1.)*cos(ceil(P[int(T.y)%3])*3.14)
    )-.5)-.4,0.)))*.2*r.y,1.)
    *(sin(dot(mod(ceil(P+vec3(A<1,A<2,A-1)),2.),c+7.)-c)*.5+.5).xzyz;
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
