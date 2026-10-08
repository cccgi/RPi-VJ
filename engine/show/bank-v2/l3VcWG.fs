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
    "Stripy Anti-aliasing" by @XorDev
    
    Often shaders introduce hard edges (aliasing) unintentionally.
    Aliasing is fine if you're going for pixelated style, but aliasing
    rarely occurs in nature and it often makes shaders look fake.
    
    This is an extension of my GM Shaders: Anti-aliasing example:
    https://www.shadertoy.com/view/l3scWM
    
    More on this in the full tutorial:
    https://mini.gmshaders.com/p/antialiasing
*/

#define GAMMA 2.2

//For when the derivatives must be manually calculated
float antialias_l2_dxy(float d, vec2 dxy)
{
    //Get gradient width
    float width = length(dxy);
    //Calculate reciprocal scale (avoid division by 0!)
    float scale = width>0.0? 1.0/width : 1e7;
    //Normalize the gradient d with it's scale
    return clamp(0.5 + 0.7 * scale * d, 0.0, 1.0);
}
//Sine-base hue function
vec3 color(float index)
{
    return cos(index+vec3(0,2,4))*0.5+0.5;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Normalize screen coordinates (aspect ratio corrected)
    vec2 uv = (fragCoord-0.5*vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;

    //Generate a little pattern
    float value = cos(uv.x*9.)*cos(uv.y*9.)*3.+uv.x*7.+uv.y*9.+u_time;
    
    //Get the gradient from the pattern
    float stripe_grad = value;
    //Break into stripe steps
    float stripe_step = round(stripe_grad);
    //Find the gradient to the nearest edge
    float stripe_frac = stripe_grad - stripe_step;
    
    //Calculate the derivatives on the continous gradient
    vec2 dxy = vec2(dFdx(stripe_grad), dFdy(stripe_grad));
    
    //Blend between the two nearest stripes
    vec3 c1 = color(stripe_step+0.0);
    vec3 c2 = color(stripe_step+1.0);
    //Anti-aliasing like normal
    vec3 col = mix(c1, c2, antialias_l2_dxy(stripe_frac,dxy));

    // Output to screen
    fragColor = vec4(pow(col, 1.0/vec3(GAMMA)),1.0);
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
