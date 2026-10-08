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
Experiment with different values. Make sure WARPRATIO is a power of DRASTARATIO.
example values:
  2 |  4 |  5 | x
256 | 64 | 25 | x
*/
// Ratio of drasta image
#define DRASTARATIO 3.
// Ratio of space distortion.
#define WARPRATIO 81.
#define PI 3.14159265359

//simple grid
vec3 grid (vec2 uv) {
    vec2 d = abs(fract(uv*DRASTARATIO) -.5);
    return vec3(max(step(.48,d.x),step(.48,d.y)));
}

//polar coordinates grid
vec3 polar(vec2 uv) {
    float a = (atan(uv.y,uv.x)/PI+1.)/2.;
    vec3 col = vec3(1.);
    col *= step(.0025, abs(mod(a,.1)-.05));
    col *= max(step(1.,length(uv)), step(length(uv),.95));
    return 1.-col;
}

/*
vec3 polar(vec2 uv) {
    float a = (atan(uv.y,uv.x)/PI+1.)/2.;
    float na = abs(mod(a,.1)-.05);
    vec3 col = vec3(1.);
    col *= step(.0025, na);
    col *= step(.025, .5-abs(fract(log(length(uv))/log()) - .5));
    return 1.-col;
}
*/



void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Shapings for ratio
    //float ratio = 1.+(WARPRATIO -1.)*(pow(clamp(mix(-.5,1.5,abs( mod(u_time*.3,2.)-1. )),0., 1. ),3.));
    //float ratio = WARPRATIO;
    float ratio = 1.+(WARPRATIO -1.)*smoothstep(.2,.8,2.*abs(fract(u_time*.05)-.5));
    
    vec2 uv = 2.*(fragCoord - vec3(u_resolution, 1.0).xy*.5)/vec3(u_resolution, 1.0).y;
    float angle = (atan(uv.y,uv.x)/PI+1.)/2.;
    float l = length(uv);
    
    // compute angle distortion
    float b = exp(-4.*PI*PI/log(ratio)); // In order to get this function I had to compute a limit of the slope of a logarithmic spiral and this spat out 
    float na = fract(log(l)/log(b)) + angle;
    // compute radial distortion along with angle
    uv = l*pow(ratio,angle) * vec2(cos(PI*2.*na),sin(PI*2.*na));
    
    // Convert coords for a self containing image
    uv /= pow(DRASTARATIO, ceil(log(max(abs(uv.x),abs(uv.y)))/log(DRASTARATIO)));
    
    //color screen with drasta image
    vec3 col = grid(uv);
    //vec3 col = polar(uv);
    
    // Output to screen
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
