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
//Tweet: https://twitter.com/XorDev/status/1517616659590266886
//Twigl: https://bit.ly/3OHnB6i

//Inspired by: https://www.shadertoy.com/view/sdBBWK

//Golfed with FabriceNeyret2's help
void mainImage(out vec4 O, vec2 I)
{
    vec3  r = vec3(u_resolution, 1.0),
          t = u_time +r-r,
          P =   ( I+I - r.xy ) / r.y
              * mat3x2(707,-408, 0,816, -707,-408) / 3e2
              +.5;

    O = ( length( fract( P +   ( t - sin(t*6.283)/6. ) 
                             * ceil( mod( vec3(2,0,1) - t , 3. ) - 2. )
                             *  cos( ceil( P[int(t)%3] ) *3.14 ) 
                       ) -.5
                )
           -.5 ) *.1 * r.yyyy;
}

//Original (266 chars)
/*
void mainImage(out vec4 O, vec2 I)
{
    //Tau approximation (pi * 2).
    float T = 6.283,
    //Shortened time variable.
    t = u_time;
    //Resolution for scaling.
    vec3 r = vec3(u_resolution, 1.0),
    //Ray origin position (rotated to for isometric view)
    P = (vec3(I+I,1)-r)*mat3(707,-408,577, 0,816,577, -707,-408,577)/3e2/r.y;
    //Get length to nearest sphere center (scaled to pixel).
    O = r.yyyy*.1*(length(fract(P+.5
    //Smoothly move alternating through axes.
    +(t-sin(t*T)/T)*(mod(vec3(2,0,1)-ceil(t),3.)-1.)
    //Alternate direction per row.
    *cos(round(P[int(t)%3])*T*.5))-.5)-.5);
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
