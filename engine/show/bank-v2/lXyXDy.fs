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
// code ref: https://www.shadertoy.com/view/Mss3Wf
#define PI 3.1415926
const int maxIterations=15;//a nice value for fullscreen is 8

float circleSize=1.0/(3.0*pow(2.0,float(maxIterations)));
//generic rotation formula
vec2 rot(vec2 uv,float a){
	return vec2(uv.x*cos(a)+uv.y*sin(a),uv.y*cos(a)-uv.x*sin(a));
}

void mainImage( out vec4 fragColor, in vec2 fragCoord ){
	//normalize stuff
	vec2 uv=vec3(u_resolution, 1.0).xy;uv=-.5*(uv-2.0*fragCoord.xy)/uv.x;
    float angle = (0.8+ 0.2* u_time*9./4.0/2.0+0.5)*9./8.0;
	//global rotation and zoom
	uv=rot(uv,angle);
	uv*=sin(u_time*PI  / 4.)*0.5+2.0;
	
	//mirror, rotate and scale 6 times...
	float s=1.0;
    
	for(int i=0;i<maxIterations;i++){
		uv=abs(uv)-s;
		uv=rot(uv,angle);
        int x = int(floor(uv.x));
        int y = int(floor(uv.y));
        uv = uv * (0.5+1.5 *float((abs((y-x)) & (y +x))));
		s/=2.;
	}
	
	//draw a circle
    vec3 colorA = vec3(0.506,0.845,0.712);
    vec3 colorB = vec3(0.900,0.728,0.5);
	vec3 col = length(uv)> circleSize / 0.00001?vec3(1.,0.62,0.87) : vec3(1.0);
    col = mix(col, colorA, sin(u_time/5.) * 0.1);
    col = mix(col, colorB, sin(u_time/5.) * 0.3);
	fragColor = vec4(col,1.);
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
