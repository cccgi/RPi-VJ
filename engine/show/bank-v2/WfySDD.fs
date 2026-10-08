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
    "Squircle 2" by @XorDev
    
    A series of lightweight shaders made for easy learning and high-performance applications:
    https://www.shadertoy.com/playlist/ccffz7
    
    MIT license
    Credit is appreciated, but not required!
    
    Here's the overview of the modified version:
    1) Center the coordinates and scale to fit vertically
    2) Rotate smoothly
       https://x.com/XorDev/status/1654553771001749504
    3) Calculate the coloring with an RGB sine wave
       iq's Palettes: https://www.shadertoy.com/view/ll2GD3
    4) Use the distance field waves for brightness
    5) Tonemap with tanh
       Intro to tonemapping: https://mini.gmshaders.com/p/tonemaps
*/

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Centered and scaled to fit vertically
    vec2 p = (fragCoord * 2. - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    //Shortened time variable
    float t = u_time;
    //Rotate coordinates using sine tricks
    
    //33 is approximately 90 degrees and 11 is approximately 270
    vec2 c = p * mat2(cos(u_time / 4. - sin(u_time) / 4. + vec4(0, 33, 11, 0)));
    //Distance field waves for rotated squircle
    float l = cos(sqrt(length(c * c)) / 0.1 - t);
    //Generate color waves with
    vec4 w = sin(3. * length(c) + 2. * c.y + t + vec4(6, 1, 2, 0));
    //Color calculation with sine oscillations, scaled and tonemapped
    fragColor = tanh((0.6 + 0.5 * w) / max(l / 0.2, -l));
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
