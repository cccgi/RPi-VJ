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
#define radius .05 
#define BGB false

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = (fragCoord-.5*vec3(u_resolution, 1.0).xy)/vec3(u_resolution, 1.0).xy; //Center on screen (-.5 to .5)
	uv.x = uv.x * (vec3(u_resolution, 1.0).x/vec3(u_resolution, 1.0).y); //Adjust x cord for aspect ratio

	
    vec3 ro = vec3(0);
    vec3 rd = normalize(vec3(uv.x, uv.y, 1));
    
    vec3 s = vec3(0.05, 0.05, abs(sin(u_time)/2.));
    
    float t = dot(s-ro, rd);
    
    vec3 p = ro + rd*t;
    
    float y = length(p-s);
    
    float x = sqrt(radius*radius - y*y);
    
    float t1 = x + t;
    
    float t2 = x - t;
    
    float dist = length(ro +rd*t1);
    
    float blink_bg = BGB ? abs(sin(u_time)) : 1.;
    
    vec3 ballcolor = bool(int(u_time)%10) ? vec3(1, .2, 0) 
        : vec3(float((int(fragCoord.x)*2)^(int(fragCoord.y)*2))/((vec3(u_resolution, 1.0).x*vec3(u_resolution, 1.0).y)/300.),.2,0);
    
  	vec3 col = y > radius ? vec3(0,0,float(int(fragCoord.x)^int(fragCoord.y))/((vec3(u_resolution, 1.0).x*vec3(u_resolution, 1.0).y)/300./blink_bg) ) : //Generate background
    	mix(vec3(x*20. - dist), ballcolor, .4);//"Raytraced" ball
    
    // Output to screen
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
