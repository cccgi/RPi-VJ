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
Title: Electric fields
*/


#define PI 3.14159265359
#define TWO_PI 6.28318530718
#define MAX_ITER 5.

vec2 adjustViewport(in vec2 uv,in vec2 r){
	return(uv*2.-r)/(r.x<r.y?r.x:r.y);
}

float random1D(in vec2 uv,in int seed){
    return fract(sin(dot(uv.xy,vec2(12.9898,78.233))+float(seed))*43758.5453123);
}

float valueNoise(in vec2 uv,in int seed){
    vec2 x=floor(uv);
    vec2 m=fract(uv);
    
    float bl=random1D(x,seed);
    float br=random1D(x+vec2(1.,0.),seed);
    float tl=random1D(x+vec2(0.,1.),seed);
    float tr=random1D(x+vec2(1.,1.),seed);
    
    vec2 cf=smoothstep(vec2(0.),vec2(1.),m);
    
    float tm=mix(tl,tr,cf.x);
    float bm=mix(bl,br,cf.x);
    
    return mix(bm,tm,cf.y);
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

mat2 rot2(in float a){
    float c=cos(a);
    float s=sin(a);
    return mat2(c,-s,s,c);
}

vec3 gradient(in float t,in vec3 a,in vec3 b,in vec3 c,in vec3 d){
	return a+b*cos(TWO_PI*(c*t+d));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord){
	vec2 uv=adjustViewport(fragCoord.xy,vec3(u_resolution, 1.0).xy);
	float t=u_time*.05;
	
	float mag=length(uv);
	float x=0.;
	vec2 coord=uv;
	float theta=TWO_PI/2.;
	float noise=valueNoise(uv,1);
	for(float i=0.;i<MAX_ITER;i++){
		coord=smoothstep(0.,1.,fold(coord));
		coord*=rot2(i*theta+mag-t*.15);
		coord=fract(coord*2.)-.5;
		float s=pow(2.,i+noise);
		x+=length(coord)/s;
	}
	x=fold(x+t);
	x=sin(x*8.+noise*.001*t+mag)/8.;
	x=abs(x);
	float stroke=sin(t*3.-noise)*.02+.05;
	x=stroke/x;
	
	const vec3 c1=vec3(.4);
	const vec3 c2=vec3(.125);
	const vec3 c3=vec3(1.);
	vec3 c4a=vec3(.3,.4,.5);
	vec3 c4b=vec3(.6);
	vec3 c4=mix(c4a,c4a,cosine(fold(t*.8),1.));
	vec3 col=gradient(mag*1.55+t+noise,c1,c2,c3,c4)*x;
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
