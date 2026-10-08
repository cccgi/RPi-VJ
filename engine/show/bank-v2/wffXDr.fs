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
    "Turbulent Flame" by @XorDev
    
    For my tutorial on Turbulence:
    https://mini.gmshaders.com/p/turbulence
    
    Simulating proper fluid dynamics can be complicated, limited, and requires a multi-pass setup.

    Sometimes you just want some smoke, fire, or fluid, and you don't want to go through all that trouble.

    This method is very simple! Start with pixel coordinates and scale them down as desired,
    then loop through adding waves, rotating the wave direction and increasing the frequency.
    To animate it, you can add a time offset to the sine wave.
    It also helps to shift each iteration with the iterator "i" to break up visible patterns.

    The resulting coordinates will appear turbulent, and you can use these coordinates in a coloring function.
    
    Smooth, continious equations look best!
    
    To complete the flame look, we need to scroll the waves and expand the coordinate space upwards 
*/

//Fire ring radius
#define RADIUS 0.4
//Falloff gradient
#define GRADIENT 0.3
//Scroll speed
#define SCROLL 1.6
//Flicker intensity
#define FLICKER 0.12
//Flicker animation speed
#define FLICKER_SPEED 12.0

//Number of turbulence waves
#define TURB_NUM 10.0
//Turbulence wave amplitude
#define TURB_AMP 0.4
//Turbulence wave speed
#define TURB_SPEED 6.0
//Turbulence frequency (inverse of scale)
#define TURB_FREQ 7.0
//Turbulence frequency multiplier
#define TURB_EXP 1.3

//Apply turbulence to coordinates
vec2 turbulence(vec2 p)
{
    //Turbulence starting scale
    float freq = TURB_FREQ;
    
    //Turbulence rotation matrix
    mat2 rot = mat2(0.6, -0.8, 0.8, 0.6);
    
    //Loop through turbulence octaves
    for(float i=0.0; i<TURB_NUM; i++)
    {
        //Scroll along the rotated y coordinate
        float phase = freq * (p * rot).y + TURB_SPEED*u_time + i;
        //Add a perpendicular sine wave offset
        p += TURB_AMP * rot[0] * sin(phase) / freq;
        
        //Rotate for the next octave
        rot *= mat2(0.6, -0.8, 0.8, 0.6);
        //Scale down for the next octave
        freq *= TURB_EXP;
    }
    
    return p;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Screen coordinates, centered and aspect corrected
    vec2 p = (fragCoord.xy*2.0-vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    vec2 screen = p;
    
    //Expand vertically
    float xstretch = 2.0 - 1.5*smoothstep(-2.0,2.0,p.y);
    //Decelerate horizontally
    float ystretch = 1.0 - 0.5 / (1.0+p.x*p.x);
    //Combine
    vec2 stretch = vec2(xstretch, ystretch);
    //Stretch coordinates
    p *= stretch;
    
    //Scroll upward
    float scroll = SCROLL*u_time;
    p.y -= scroll;
    
    p = turbulence(p);
    
    //Reverse the scrolling offset
    p.y += scroll;
    
    //Distance to fireball
    float dist = length(min(p,p/vec2(1,stretch.y))) - RADIUS;
    //Attenuate outward and fade vertically
    float light = 1.0/pow(dist*dist+GRADIENT*max(p.y+.5,0.0),3.0);
    //Coordinates relative to the source
    vec2 source = p + 2.0*vec2(0,RADIUS) * stretch;
    //RGB falloff gradient
    vec3 grad = 0.1 / (1.0 + 8.0*length(source) / vec3(9, 2, 1));
    
    //Flicker animation time
    float ft = FLICKER_SPEED * u_time;
    //Flicker brightness
    float flicker = 1.0+FLICKER*cos(ft+sin(ft*1.618-p.y));
    //Ambient lighting
    vec3 amb = 16.0*flicker/(1.0+dot(screen,screen))*grad;
    
    //Scrolling texture uvs
    vec2 uv = (p - SCROLL*vec2(0,u_time)) / 1e2 * TURB_FREQ;
    //Sample texture for fire
    vec3 tex = texture(iChannel0,uv).rgb;
    
    //Combine ambient light and fire
    vec3 col = amb + light*grad*tex;
    //Exponential tonemap
    //https://mini.gmshaders.com/p/tonemaps
    col = 1.0 - exp(-col);
    fragColor = vec4(col,1);
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
