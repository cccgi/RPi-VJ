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
    "Chromatic Aberration" by @XorDev
    
    Here's a super simple "Chromatic Aberration" effect that you can easily
    add as a post-process effect to other shaders.
    
    Used here: https://www.shadertoy.com/view/mdfGRs
*/
void mainImage(out vec4 fragColor, vec2 fragCoord)
{
    //Compute uv coordinates from fragcoordinates
    vec2 u = fragCoord/vec3(u_resolution, 1.0).xy;
    //Color output starts at 0.
	vec4 c = vec4(0);
	
    //Iterate 20 times from 0 to 1
	for(float i = 0.; i<1.; i+=.05)
        //Add a texture sample approaching the center (0.5, 0.5)
        //This center could moved to change how the direction of aberation
        //The mix amount determines the intensity of the aberration smearing
        //Note: .bgra is for blue tint here and isn't important to the example
		c += texture(iChannel0, mix(u, vec2(.5), i*.2*cos(u_time))).bgra*
        //This makes each sample have a different color from red to green to blue
        //The total should be multiplied by the 2/number of samples, (e.g. 0.1)
        vec4(i,1.-abs(i+i-1.),1.-i,1)*.1;
	
    //Output the resulting color
    fragColor = c;
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
