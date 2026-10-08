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
Author: Felipe Tovar-Henao [www.felipe-tovar-henao.com]
Title: Sacred Geometries
*/

#define PI 3.14159265359
#define TWO_PI 6.28318530718
#define MAX_ITER 3

vec3 gradient(in float t,in vec3 a,in vec3 b,in vec3 c,in vec3 d){
	return a+b*cos(TWO_PI*(c*t+d));
}

mat2 rot2(in float a){
	float c=cos(a);
	float s=sin(a);
	return mat2(c,-s,s,c);
}

float scale(in float x,in float a,in float b,in float c,in float d){
	return(x-a)/(b-a)*(d-c)+c;
}

vec2 scale(in vec2 p,in float a,in float b,in float c,in float d){
	return vec2(scale(p.x,a,b,c,d),scale(p.y,a,b,c,d));
}

vec3 scale(in vec3 p,in float a,in float b,in float c,in float d){
	return vec3(scale(p.xy,a,b,c,d),scale(p.z,a,b,c,d));
}

vec4 scale(in vec4 p,in float a,in float b,in float c,in float d){
	return vec4(scale(p.xy,a,b,c,d),scale(p.zw,a,b,c,d));
}

float fold(in float x){
	return abs(mod(x+1.,2.)-1.);
}

vec2 fold(in vec2 p){
	return vec2(fold(p.x),fold(p.y));
}

vec3 fold(in vec3 p){
	return vec3(fold(p.xy),fold(p.z));
}

vec4 fold(in vec4 p){
	return vec4(fold(p.xy),fold(p.zw));
}

float cosine(in float x,in float s){
	float y=cos(fract(x)*PI);
	return floor(x)+.5-(.5*pow(abs(y),1./s)*sign(y));
}

vec2 cosine(in vec2 p,in float s){
	return vec2(cosine(p.x,s),cosine(p.y,s));
}

vec3 cosine(in vec3 p,in float s){
	return vec3(cosine(p.xy,s),cosine(p.z,s));
}

vec4 cosine(in vec4 p,in float s){
	return vec4(cosine(p.xy,s),cosine(p.zw,s));
}

vec2 adjustViewport(in vec2 uv,in vec2 r){
	return(uv*2.-r)/(r.x<r.y?r.x:r.y);
}

void mainImage(out vec4 fragColor,in vec2 fragCoord){
	// reference time
	float t=u_time*.125;
	
	const vec3 c1=vec3(.5);
	const vec3 c2=vec3(.5);
	const vec3 c3=vec3(1.);
	vec3 c4a=vec3(.7647,.8784,.9451);
	vec3 c4b=vec3(.4667,.549,.6);
	vec3 c4=mix(c4a,c4b,cosine(fold(t*2.5),1.));
	
	vec2 uv=adjustViewport(fragCoord.xy,vec3(u_resolution, 1.0).xy)*.75;
	
	// angle of rotation for each fractal layer;
	float theta=TWO_PI/scale(sin(t*.1),-1.,1.,1.,12.);
	
	// mirror-repetition of space
	uv=fold(uv);
	
	// get mirrored distance from origin
	float mag=length(uv);
	
	// copy global uv coordinates
	vec2 loc=uv;
	
	// initialize alpha
	float a=0.;
	
	// fractal zoom
	float base=2.+sin(t)*.5;
	
	// frequency of rings
	float fq=7.5+sin(mag-t*.5)*3.;
	
	// stroke edge
	float edge=cosine(fold(t*.1),2.5)*.008+.01;
	
	// stroke color
	vec3 col=vec3(0.);
	
	float blur=exp(-length(uv*mag));
	// generate fractal layers
	for(float i=0.;i<float(MAX_ITER);i+=1.){
		// rotate current layer
		loc*=rot2(pow(base,i)*theta);
		
		// fractalize
		loc=fract(loc*base)-.5;
		
		// fold layer
		loc=fold(loc);
		
		// get layer's distance from origin
		float d=length(loc)*blur;
		
		// distance to stroke
		d=sin(d*fq+t)/fq;
		d=abs(d);
		d=edge/d;
		d=cosine(d,1.5);
		d=pow(d,.5);
		
		col+=gradient(mag+i*.25-t*.5,c1,c2,c3,c4)*d;
	}
	
	fragColor=vec4(col,1.);
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
