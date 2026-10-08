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
    "GM Shaders: Gamma" by @XorDev
    
    Anything to do with light should be done in linear color.
    On the left: approx. gamma light blending
    On the right: linear light blending

    Written for my tutorial on Gamma:
    https://mini.gmshaders.com/p/gamma
    
    sRGB tends to overexpose easily.
    Not to be confused with tonemapping:
    https://www.shadertoy.com/view/ddVfzd
*/

//Decode gamma to linear color
vec3 gamma_decode(vec3 srgb, float GAMMA)
{
    return pow(srgb, vec3(GAMMA));
}

//Encode linear color to gamma
vec3 gamma_encode(vec3 lrgb, float GAMMA)
{
    return pow(lrgb, vec3(1.0/GAMMA));
}

//Here's the more exact formulas specifically for sRGB
//The difference between Gamma 2.2 and sRGB is subtle
//But it is more noticeable with darker shades

//Decode sRGB to linear
vec3 SRGB_decode(vec3 srgb)
{
    return mix(
        srgb / 12.92,
        pow((srgb + 0.055) / 1.055, vec3(2.4)),
        step(0.04045, srgb)
    );
}
//Encode linear to sRGB
vec3 SRGB_encode(vec3 lrgb)
{
    return mix(
        12.92 * lrgb,
        1.055 * pow(lrgb, vec3(1.0 / 2.4)) - 0.055,
        step(0.0031308, lrgb)
    );
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Screen coordinates from -1 to +1 (aspect ratio corrected)
    vec2 suv = (fragCoord*2.0 - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    //2.2 gamma on the right
    float GAMMA = suv.x>0.0 ? 2.2 : 1.0;
    
    //Light source positions
    vec2 pos = 0.4 * sin(u_time+vec2(0,11)) * cos(u_time*.618);
    
    //Red light
    vec3 light1 = 1.0 / (1.0 + length(suv+pos) * vec3(1,5,9));
    //Blue Light
    vec3 light2 = 1.0 / (1.0 + length(suv-pos) * vec3(9,5,1));
    
    //Linear color values
    vec3 lin1 = gamma_decode(light1, GAMMA);
    vec3 lin2 = gamma_decode(light2, GAMMA);
    
    //Add light sources together
    vec3 col = lin1+lin2;
    
    //Convert back to sRGB
    fragColor = vec4(gamma_encode(col,GAMMA), 1.0);
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
