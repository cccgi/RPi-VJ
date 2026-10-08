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
    "Breaking Repetition" by @XorDev
    
    Here's a simple technique for pseudo-randomly distributing objects.
    This method preserves proper spacing for distance fields and works in 3D!
    
    The concept is simple:
    -Render a few tiled layers of objects
    -Rotate and scale the coordinates for each layer
    -Divide the distance by the octave scale
    
    Rotation only helps a ton, but when combined with scaling it's patternless*
*/

//Optional octave index for coloring
float IND = 0.0;
//Distance field function
float dist(vec2 p, float num, float scl)
{
    //Arbitrary rotation matrix
    mat2 m = mat2(.8,-.6,.6,.8);
    //Scaling factor
    float s = 1.0;
    //Store minimum distance
    float d = 1e9;
    
    //Loop through passes
    for(float i = 0.0; i<num; i++)
    {
        //Repeating circles rotating octaves and scaling
        float _d = length(mod((p*=m)*s+i*.3,2.)-1.)/s;
        //Store closest distance (and optionally, index for coloring)
        if (d>_d)
        {
            d = _d;
            IND = i;
        }
        //Scale by factor amount
        s *= scl;
    }
    //Return distance field
    return d;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Centered and scale pixel coordinates
    vec2 coord = (fragCoord - vec3(u_resolution, 1.0).xy*0.5)/3e1;
    //Time loop
    float T = abs(mod(u_time+6.,12.)-6.);
    //Number of octaves
    float N = clamp(T,1.,4.);
    //Octave scaling
    float S = clamp(1.4-T*.1,.9,1.);
    //Distance gradients and coloring
    fragColor = (1.-dist(coord,N,S)) * (.5-.5*cos(IND+vec4(0,2,4,0)));
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
