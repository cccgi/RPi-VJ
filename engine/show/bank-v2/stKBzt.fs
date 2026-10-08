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
// A trick to use any variable as an array index.
// Array index must be a constant, and the loop index is a constant when unrolled 
// so we can use it to compare it with any variable and return the array value that matches, 
// or an interpolation of the two closest matches.

// mouse click to see palette

vec3 palette[7]; // the color palette is stored here

// just pick a color based on c value
vec3 getcolor(float c) 
{
	c=mod(c,7.); // cycle palette
	int p=0;
	vec3 color=vec3(0.);
	for(int i=0;i<7;i++) {
		if (float(i)-c<=.0) { // check loop index against color value
			color=palette[i]; // store color picked	
		}
	}
	return color;
}

// get a gradient of the palette based on c value, with a "smooth" parameter (0...1)
vec3 getsmcolor(float c, float s) 
{
    s*=.5;
    c=mod(c-.5,7.);
    vec3 color1=vec3(0.0),color2=vec3(0.0);
    for(int i=0;i<7;i++) {
        if (float(i)-c<=.0) {
            color1 = palette[i];
            color2 = palette[(i+1>6)?0:i+1];
        }
    }
    // smooth mix the two colors
    return mix(color1,color2,smoothstep(.5-s,.5+s,fract(c)));
}
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
	// define the colors 
	palette[6]=vec3(255,000,000)/255.;
	palette[5]=vec3(255,127,000)/255.;
	palette[4]=vec3(255,255,000)/255.;
	palette[3]=vec3(150,050,050)/255.;
	palette[2]=vec3(000,050,50)/255.;
	palette[1]=vec3(075,000,130)/255.;
	palette[0]=vec3(143,000,255)/255.;
	
	vec3 color=vec3(0.);
	vec2 uv = gl_FragCoord.xy / vec3(u_resolution, 1.0).xy;

	if (uv.x>.9) { 
		color=getsmcolor(uv.y*7.+u_time*.5,.25+.75*abs(sin(u_time))); //gradient function
	} else if (uv.x<.1) {
		color=getcolor(uv.y*7.-u_time*.5); // basic function
	} 

	vec2 p=(uv-.5);
	p.x*=vec3(u_resolution, 1.0).x/vec3(u_resolution, 1.0).y;
	

	// fractal
    float a=u_time*.50;	
    float b=u_time*1.;	
	float ot=1000.;
	mat2 rot=mat2(cos(a),sin(a),-sin(a),cos(a));
	p+=sin(b)*.005;
    float l=length(p);
    for(int i=0;i<15;i++) {
		p*=rot;
        p=abs(p)*1.2-1.;
        ot=min(ot,abs(dot(p,p)-sin(b+l*20.)*.015-.15)); //orbit trap
	}
	ot=max(0.,.1-ot)/.1; //orbit trap 
	if (length(max(vec2(0.),abs(uv-.5)-vec2(.485,.47)))>0.0) color*=0.; // border 	
	if (iMouse.w<1. || length(max(vec2(0.),abs(uv-.5)-vec2(.37,.46)))<0.01) {
		color=getsmcolor(ot*4.+l*10.-u_time*7.,1.)*(1.-.4*step(.5,1.-dot(p,p))); //get color gradient for orbit trap value	
	}
    color=mix(vec3(length(color))*.5,color,.6); // saturation adjustment
    color*=1.-pow(l*1.1,5.); color+=pow(max(0.,.2-l)/.2,3.)*1.2; // center glow
    fragColor = vec4(color,1.0)*min(1.,u_time*.3);
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
