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
    "Efficient Chaos" by @XorDev
    
    Techniques I use for fast pseudo-randomness in pixel shaders
    
    Read the full tutorial here:
    https://mini.gmshaders.com/p/chaos
*/

//Number of layers
#define LAYERS 5.0
//Overall scale
#define SCALE 0.1
//Overall brightness
#define BRIGHTNESS 0.04
//Fisheye curvature (0.0 = none)
#define CURVATURE 0.1

//Toggle layer rotation
#define LAYER_ROTAT
//Toggle layer shift (shifting factor)
#define LAYER_SHIFT 1.618
//Toggle layer spacing (spacing factor)
#define LAYER_SPACE 0.1
//Toggle layer wave distortions (amplitude)
#define LAYER_WAVES 0.2
//Toggle Layer blocking (threshold)
#define LAYER_BLOCK 0.5
//Toggle color shifting
#define LAYER_COLOR
//Toggle layer parallax (velocity)
#define LAYER_PARALLAX vec2(2,1)

//White noise function (1D output)
float w1(vec2 p)
{
    return fract(cos(dot(p,vec2(0.1618,0.2399)))*4e4);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    //Signed screen uv coordinates
    vec2 suv = (2.0*fragCoord - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    //Screen curvature
    #ifdef CURVATURE
        suv /= 1.0+CURVATURE*dot(suv,suv);
    #endif
    
    //Output color for example
    vec3 col = vec3(0.0);
    
    //Starfield Coordinates
    vec2 c = suv / SCALE;
    //Spacing factor
    float spa = 1.0;
    
    //Read about the Golden Angle here:
    //https://mini.gmshaders.com/i/139108917/golden-angle
    mat2 gold = mat2(0.22252093, -0.97492791, 0.97492791,  0.22252093);
    //Save orientation
    mat2 orient = mat2(1);
    
    //Loop through layers
    for(float i = 0.5 / LAYERS; i<1.0; i+=1.0 / LAYERS)
    {
        //Rotate layer
        #ifdef LAYER_ROTAT
            orient *= gold;
        #endif
        
        //Get rotated coordinates
        vec2 p = c * orient;
        
        //Apply layer shifting
        #ifdef LAYER_SHIFT
            p += LAYER_SHIFT*i;
        #endif
        
        //Adjust layer spacing
        #ifdef LAYER_SPACE
            spa += LAYER_SPACE;
            p /= spa;
        #endif
        
        //Apply layer parallax
        #ifdef LAYER_PARALLAX
            p += u_time * (i+1.0) * LAYER_PARALLAX * orient;
        #endif
        
        //Add layer waves
        #ifdef LAYER_WAVES
            p += LAYER_WAVES*sin(p.yx);
        #endif
        
        //Cut out random blocks
        float n = w1(floor(p/2.0)+i);
        #ifdef LAYER_BLOCK
            if (n>LAYER_BLOCK) continue;
        #endif
        
        //Color phase
        float phase = i*6.2831;
        #ifdef LAYER_COLOR
            phase += p.x + 0.61*p.y;
        #endif
        
        //Distance to cell center
        float len = length(mod(p, 2.0) - 1.0);
        //Pick RGB hue from color phase [0.1, 2.1]
        vec3 hue = cos(phase+vec3(6,1,2))+1.1;
        //Light attenuation with circle cutoff
        float att = max(1.0-len,0.0) / len;
        col += hue * att;
    }
    //Tanh tonemapping
    //https://www.shadertoy.com/view/ms3BD7
    col = tanh(BRIGHTNESS*col);
    //Output color
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
