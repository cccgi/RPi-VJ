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
// base -1+i  allows to represent the whole complex plane as a binary number set.
// TwinDragon of order n = all positions that can be represented with n bits.
// cf https://en.wikipedia.org/wiki/Complex-base_system#Base_%E2%88%921_%C2%B1_i
               

// --- golfed version.   -9 ch by coyote

void mainImage( out vec4 O, vec2 I )
{
     I -= .5 + .5*vec3(u_resolution, 1.0).xy;  
     
    for ( O *= 0. ; I != -I ; O += .05 )              // --- for all digits, as long as bit remains
        I = floor( ( vec2( I.y, -I) -I ) / 2. );      // euclidian div by base -1+i  = next bit
}





/** // --- 270 chars version

// complexes op from https://shadertoyunofficial.wordpress.com/2019/01/02/programming-tricks-in-shadertoy-glsl/
#define cmul(A,B) ( mat2( A, -(A).y, (A).x ) * (B) )  // by deMoivre formula
#define cinv(Z)   ( vec2( (Z).x, -(Z).y ) / dot(Z,Z) ) 
#define cdiv(A,B)   cmul( A, cinv(B) )


void mainImage( out vec4 O, vec2 I )
{
     I -= .5 + .5*vec3(u_resolution, 1.0).xy;  
     
    float i = 0.;
    for ( ; i < 16.; i++ ) {                  // --- for all digits   
        I = floor( cdiv( I, vec2(-1,1) ) );   // euclidian div by base -1+i  = next bit
        if ( I == vec2(0) ) break;          
    }
    O = vec4( i/16. );                        // show order as grey level
}

/**/
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
