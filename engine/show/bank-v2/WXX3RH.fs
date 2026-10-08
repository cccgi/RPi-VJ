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
    "Fire" by @XorDev
    
    An attempt to create a fire effect in 280 chars.
    See my twigl example here:
    https://x.com/XorDev/status/1903168199069216954
    
    One-Pass Fire:
    https://www.shadertoy.com/view/tf2SWc
    
    Turbulent Flame:
    https://www.shadertoy.com/view/wffXDr
    
    
    FabriceNeyret2: -11
*/

//Noise Texture
#define T texture(iChannel0, p

void mainImage(out vec4 O, vec2 I)
{
    //Resolution for scaling and centering
    vec2  r = vec3(u_resolution, 1.0).xy,
    //Centered, aspect-correct coordinates
          p = ( I+I - r ) / r.y,
    //Vector for wave rotation
          R;
    //Expand coordinates vertically
    p *= 1. - .5 / vec2( 1./p.y  , 1. + p*p );
    
    //Time for scrolling and turbulence
    float t = u_time, 
    //Starting wave frequency (and magic rotation number)
          F = R.x = 11.;
    //Scroll upward and loop through turbulence waves
    //https://mini.gmshaders.com/p/turbulence
    for( p.y -= t ; F < 50.; F *= 1.2 )
        //Add waves and rotate
        p += .4* sin( F* dot( p, sin(++R) ) + 6.*t ) * cos(R)/F;
    
    //Sample noise for distance field
    F = T/8.).r;
    //Get approximate distance to fire source
    t = length(p+vec2(0,t+.5)) / .8 - F;
    //Add more noise for additional texturing
    F += T/4.).g;
    
    //Compute brightness with a higher falloff outside the fire
    //Tanh for tonemapping
    //https://mini.gmshaders.com/p/tonemaps
    O = tanh( F / ( .2 - .1*F 
                   + t*t * (t < 0. ? 5. : 50. )
                  ) / vec4(1,3,9,1)
            );
}

//Original [337]
/*
#define T texture(iChannel0, p

void mainImage(out vec4 O, vec2 I)
{
    //Resolution for scaling and centering
    vec2 r = vec3(u_resolution, 1.0).xy,
    //Centered, aspect-correct coordinates
    p = (I+I - r) / r.y,
    //Vector for wave rotation
    R;
    //Expand coordinates vertically
    p *= 1.-.5/vec2(1./p.y,1.+p*p);
    //Time for scrolling and turbulence
    float t = u_time,
    //Starting wave frequency (and magic rotation number)
    F = 11.,
    //Approximated distance to fire
    d;
    //Scroll upward
    p.y -= t;
    //Loop through turbulence waves
    //https://mini.gmshaders.com/p/turbulence
    for(R.x = F; F<50.; F*=1.2)
        //Add waves and rotate
        p += .4*sin(F*dot(p,sin(++R))+6.*t)*cos(R)/F;
    
    //Sample noise for distance field
    F = T/8.).r;
    //Get distance to fire source
    d = length(p+vec2(0,t+.5))/.8-F;
    //Add more noise for texturing
    F += T/4.).g;
    
    //Compute brightness with a higher falloff outside the fire
    //Tanh for tonemapping
    //https://mini.gmshaders.com/p/tonemaps
    O = tanh(F / (.2-.1*F + max(d/.1,-d)*abs(d)/.2) / vec4(1,3,9,1));
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
