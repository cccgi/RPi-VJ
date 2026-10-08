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
#define NUM_ITER 15
#define PI 3.14159265359

float f(vec2 pixel, int i, float mod)
{
	float time = u_time + 0.3 + mod;
	float v = cos(u_time + pixel.y + cos(2.0 * u_time + PI * pixel.x));
	return v;
}

float g(vec2 pixel, int i, float mod)
{
	float time2 = u_time +  2.0 * 0.3 + mod;
	float v = cos(time2 + pixel.x + cos( 3.0 * (time2 + 0.3) + PI * pixel.y));
	return v;
}

float integrateX(vec2 pixel)
{
	float r = 0.0;
	float dt = 0.3;
	
	const int iter = NUM_ITER;
	
	for(int i = 0; i < iter; ++i)
	{
		r = r + dt * f(pixel, i, 0.3);
	}
	
	return r;
}

float integrateY(vec2 pixel)
{
	float r = 0.0;
	float dt = 0.3;
	
	const int iter = NUM_ITER;
	
	for(int i = 0; i < iter; ++i)
	{
		r = r + dt * g(pixel, i, 0.3);
	}
	
	return r;
}

vec2 density(vec2 pixel)
{
	float x = integrateX(pixel);
	float y = integrateY(pixel);
	return vec2(x, y);
}


vec2 random2f( vec2 seed )
{
	float rnd1 = mod(fract(sin(dot(seed, vec2(14.9898,78.233))) * 43758.5453), 1.0);
	float rnd2 = mod(fract(sin(dot(seed+vec2(rnd1), vec2(14.9898,78.233))) * 43758.5453), 1.0);
	return vec2(rnd1, rnd2);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
	vec2 rand = random2f(vec2(25.0, 10.0));
	
	vec2 uv = (fragCoord.xy + rand) / vec3(u_resolution, 1.0).xy;
	
	vec2 d = density(uv);
	
	vec4 color = vec4( (1.0 + sin(d.x * d.y)) * 0.5,
					   (1.0 + cos(d.x + d.y)) * 0.5,
					   (1.0 + sin(d.x + d.y)) * 0.5, 1.0);
	
	fragColor = color;
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
