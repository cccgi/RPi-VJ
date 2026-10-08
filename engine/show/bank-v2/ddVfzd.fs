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
    "GM Shaders: Tonemaps" by @XorDev

    A quick tonemap demo for my tutorial:
    https://mini.gmshaders.com/p/tonemaps
*/
#define TONEMAP -1

#define TM_NONE   0
#define TM_ACES   1
#define TM_HABLE  2
#define TM_UNREAL 3
#define TM_TANH   4

// Narkowicz 2015, "ACES Filmic Tone Mapping Curve"
// https://www.shadertoy.com/view/llXyWr
vec3 Tonemap_ACES(vec3 x)
{
    const float a = 2.51;
    const float b = 0.03;
    const float c = 2.43;
    const float d = 0.59;
    const float e = 0.14;
    return (x * (a * x + b)) / (x * (c * x + d) + e);
}

// Hable 2010, "Filmic Tonemapping Operators"
// http://filmicworlds.com/blog/filmic-tonemapping-operators/
vec3 Tonemap_Uncharted2(vec3 x)
{
    x *= 16.0;
    const float A = 0.15;
    const float B = 0.50;
    const float C = 0.10;
    const float D = 0.20;
    const float E = 0.02;
    const float F = 0.30;
    
    return ((x*(A*x+C*B)+D*E)/(x*(A*x+B)+D*F))-E/F;
}

// Unreal 3, Documentation: "Color Grading"
// https://www.shadertoy.com/view/llXyWr
vec3 Tonemap_Unreal(vec3 x)
{
    // Adapted to be close to Tonemap_ACES, with similar range
    // Gamma 2.2 correction is baked in, don't use with sRGB conversion!
    return x / (x + 0.155) * 1.019;
}

// My tanh tonemap
vec3 Tonemap_tanh(vec3 x)
{
    x = clamp(x, -40.0, 40.0);
    return (exp(x)-exp(-x)) / (exp(x)+exp(-x));
}

//Demo
void mainImage( out vec4 fragColor, vec2 fragCoord)
{
    //Resolution, position and offset vector
    vec2 r = vec3(u_resolution, 1.0).xy;
    vec2 p = (fragCoord*2.0-r)/r.y;
    vec2 v = vec2(.8*cos(u_time),.3);
    
    vec4 col = vec4(0,0,0,1);
    //Center ring
    col.rgb += vec3(0.3,0.1,1.0) / max(abs(length(p  )-.4), 1e-5);
    //Green ring
    col.rgb += vec3(0.1,0.4,0.1) / max(abs(length(p+v)-.1), 1e-5);
    //Orange ring
    col.rgb += vec3(0.5,0.2,0.1) / max(abs(length(p-v)-.1), 1e-5);
    
    //Select tonemap
    int t = TONEMAP<0? int(u_time)%5 : TONEMAP;
    //Apply tonemap
    if (t == TM_NONE)
        col.rgb /= 1e1;
    else if (t == TM_ACES)
        col.rgb = Tonemap_ACES(col.rgb / 4e1);
    else if (t == TM_HABLE)
        col.rgb = Tonemap_Uncharted2(col.rgb / 4e1);
    else if (t == TM_UNREAL)
        col.rgb = Tonemap_Unreal(col.rgb / 4e1);
    else
        col.rgb = Tonemap_tanh(col.rgb / 4e1);
    
    fragColor = col;
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
