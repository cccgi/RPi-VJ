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
    "Cheap Turbulence" by @XorDev

    Simulating proper fluid dynamics can be complicated and requires a back buffer or multi-pass setup.

    Sometimes, you just want to emulate some smoke or something simple, and you don't want to go through all that trouble.

    This method is very simple! Start with pixel coordinates and scale them down as desired, then with a for loop,
    you should do a sine wave offset. In my case I'm doing "p.x+=sin(p.y)".
    To animate it, you can add a time offset to the sine wave, and it also helps to shift each iteration with the
    iterator "i" to break up visible patterns.

    Next, you want to rotate the coordinates and scale down. It could be as simple as p=p.yx/0.8 or a rotation matrix like mat2(.8,-.6,.6,.8).

    Now the resulting p coordinates will appear turbulent, and you can use these coordinates in color function.
    My color equation looks like this:

    fragColor=sin(p.xyxy*.3+vec4(0,1,2,3))*.5+.5
    
    Smooth, continious equations look best
*/
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Scaled pixel coordinates
    vec2 p=fragCoord.xy/vec3(u_resolution, 1.0).y*6.;
    
    //8 wave passes
    for(float i=0.0; i<8.0;i++)
    {
        //Add a simple sine wave with an offset and animation
        p.x += sin(p.y+i+u_time*.3);
        //Rotate and scale down
        p *= mat2(6,-8,8,6)/8.;
    }
    //Pick a color using the turbulent coordinates
    fragColor = sin(p.xyxy*.3+vec4(0,1,2,3))*.5+.5;
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
