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
    "Add vs Screen" by @XorDev
    
    Comparing Additive blending with screen blending.
    Additive blending is just when you add color "A" to color "B": A+B
    This is fine when the sum of the colors is less than 1, but when you
    adding bright colors, you may notice nasty over exposure and color changes.
    For example: vec3(1,0,0) + vec3(0,1,0) looks the same as vec3(1,1,0) + vec3(0,1,0)
    
    The simple fix here is to use screen blending: 
    https://en.wikipedia.org/wiki/Blend_modes#Screen
    
    The concept is to invert both colors, multiply them and then invert the results:
    1 - (1-A) * (1-B)
    This can be simplified to:
    A + B - A * B
    It's also sometimes written as A + B*(1-A), which is the same result.
    This blending is commutative, so swapping A and B makes no difference!
    
    
    This means the total of each color channel smoothly approaches full-brightness, 
    and won't start clipping as soon as it add up to 1.
    For example if A and B are both 0.5, you get an output of 0.5+0.5-0.25 or 0.75
    
    Note: If you're dealing with floating-point colors, it's better to use tonemapping:
    shadertoy.com/view/ddVfzd
    This technique is primarily for blending standard 8-bit, unorm textures.
*/

//Blend mode (0==Additive, 1==Screen)
#define SCREEN int(u_time)&1

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Resolution for scaling
    vec2 res = vec3(u_resolution, 1.0).xy;

    //Distance to point "a" and "b"
    float la = length(fragCoord-res*vec2(.65,.5)) / res.y;
    float lb = length(fragCoord-res*vec2(.40,.5)) / res.y;

    
    //Attenuate light
    vec3 A = exp(-abs(la-.15) * vec3(4,10,14));
    vec3 B = exp(-abs(lb-.25) * vec3(10,14,4));

    //Blend additively or with screen blending
    fragColor = vec4(A+B - A*B * float(SCREEN), 1.0);
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
