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
const int iterations = 15;

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
	vec2 uv = fragCoord.xy / vec3(u_resolution, 1.0).xy;
    uv = uv * 2.0 - 1.0;
    uv.x *= vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y;
    
     uv.x += sin(uv.y + u_time * .2) / 10.0;
    
    // Zoom
    uv *= 1.05 + cos(u_time * .25);
    
    // Pan
    uv +=  .25 * vec2(sin(u_time * .02), .25 * cos(u_time * .01));
    
    vec2 sp = vec2(.5, .5);
    
    float average = 0.;
	float len = length(uv);
	float prevl;
    
    for (int ndx = 0; ndx < iterations; ndx++) 
	{	
		//kaliset base form
		uv = abs(uv) / dot(uv,uv) - sp; 

		prevl = len;
		len = length(uv);
	
		average += abs(len - prevl);
	}
    
    //get the average length based upon the amount of iterations elapsed. multiply it to adjust "definition"
	average /= float(iterations) * 10.;
	
	//color fluctuation
	average += u_time * 0.025;
	
	vec3 myColor = vec3(0.2,0.2,1.0);
	vec3 finalColor;	
	
	//set the colors! 
	finalColor.r = (fract(float(average) / myColor.r));
	finalColor.g = (fract(float(average) / myColor.g));
	finalColor.b = (fract(float(average) / myColor.b));
	
	fragColor = vec4(finalColor,1.0);
       
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
