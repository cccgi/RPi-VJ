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
#define ITERATIONS 12

vec3 lighten(vec3 c, float d) {
	return vec3(min(1.0, c.r + d), min(1.0, c.g + d), min(1.0, c.b + d));
}

vec3 pastel(vec3 c) {
	float avg = (c.r + c.g + c.b) / 3.0;
	return lighten(c, avg);
}

vec2 rotate2D(vec2 p, float angle) {
	return vec2(
        p.x * cos(angle) + p.y * sin(angle), 
        -p.x * sin(angle) + p.y * cos(angle)
    );
}

float julia(vec2 c,vec2 c1, float max){
	float xx = c.x * c.x;
	float yy = c.y * c.y;
	float xy = c.x * c.y;
	float s = xx + yy;
    
	for (int i = ITERATIONS; i > 0; i--) {
		c = vec2(xx - yy, xy + xy) + c1;
		xx = c.x * c.x;
		yy = c.y * c.y;
		xy = c.x * c.y;
		s = xx + yy;
        
		if (s >= max || i == 1) {
            return float(ITERATIONS - i) / float(ITERATIONS);
        }
	}
	return 0.0;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
	vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy;	
	vec2 juv  = rotate2D(uv - vec2(0.5), u_time * 0.74) + vec2(cos(u_time * 2.0) * 0.1, sin(u_time) * 0.1);
    
	float j = julia(juv, juv, 16.0);
	float r = j * sin(u_time + uv.y * 2000.0 * sin(u_time)) * cos((juv.x + juv.y) * u_time * 2.0)
                * sin(uv.y * sin(u_time)* 3.0);
	float g = j * cos(u_time + uv.y * 3000.0 * cos(u_time))
                * sin(juv.x * u_time * 6.0 + cos(juv.x + juv.y) * u_time * 333.0);
    
	float b = (r + g + sin(u_time * 6.0) *uv.y) / 2.0;
    
	fragColor = vec4(pastel(vec3(r, g, b)), 1.0);
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
