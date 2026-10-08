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
#ifdef GL_ES
precision mediump float;
#endif

uniform float time;
uniform vec2 mouse;
uniform vec2 resolution;

const float minR = -2.0;
const float maxR = 1.0;
const float minI = -1.0;
const float maxI = 1.0;
const int maxIterations = 40;

struct complexNumber
{
	float r;
	float i;
};
	
complexNumber c_add(complexNumber a, complexNumber b)
{
	complexNumber ret;
	ret.r = a.r + b.r;
	ret.i = a.i + b.i;
	return ret;
}

complexNumber c_subtract(complexNumber a, complexNumber b)
{
	complexNumber ret;
	ret.r = a.r - b.r;
	ret.i = a.i - b.i;
	return ret;
}

complexNumber c_divide(complexNumber a, float d)
{
	complexNumber ret;
	ret.r = ret.r / d;
	ret.i = ret.i / d;
	return ret;
}

complexNumber c_pow(complexNumber a, const int p)
{
	complexNumber ret = a;
	int iCount = 1;
	for(int i = 1; i < 5; i++)
	{
		ret.r = ret.r*ret.r - ret.i*ret.i;
		ret.i = 2.0*a.r*a.i;
		if(++iCount == p)
			break;
	}
	
	return ret;
}



float c_abs(complexNumber a)
{
	float absVal = sqrt(a.r*a.r + a.i*a.i);
	return absVal;
}

complexNumber c_exp(complexNumber a)
{
	complexNumber ret =  c_add(complexNumber(1.0,0.0), a); // not really exp, but it looks nice
	//ret = c_add(ret, c_divide(c_pow(ret,2), 2.0));
	return ret;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
	float time = u_time;
	vec2 pos = ( fragCoord.xy / vec3(u_resolution, 1.0).xy );
	vec3 col = vec3(0.0,0.0,0.0);

	complexNumber c = complexNumber(0.0,0.0);	
	c.r = minR + (cos(pos.x*16.0)/2.0+1.0)*(maxR-minR);
	c.i = minI + (cos(pos.y*16.0)/2.0+1.0)*(maxI-minI);
	
	complexNumber z = c;
	int iterations = -1;
	for(int i = 0; i < maxIterations; i++)
	{
		if(c_abs(z) > 50.0)
		{
			iterations = i;
			break;
		}
		z = c_add(c_exp(c_pow(z,3)), complexNumber(-2.0*sin(time+(pos.x - sin(pos.y)))-2.0, 0.0));
		
	}
	
	if(iterations >= 0)
	{
		float i = float(iterations)/float(maxIterations);
		col.r = smoothstep(0.0,0.5+(0.5*sin(time)/2.0), i);
		col.g = smoothstep(0.0,1.0,i);
		col.b = smoothstep(0.4,1.0, i);
	}
	else
	{
		col.g = 0.3*cos(vec3(u_resolution, 1.0).x*length(vec2(0.5,0.5) - pos)) * cos(vec3(u_resolution, 1.0).x*length(vec2(0.8,0.2) - pos));
		col.b = cos(sin(time)*10.0+10.0*length(vec2(0.2,0.6) - pos));
	}
	
	fragColor.rgb = col;
	fragColor.a = 1.0;
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
