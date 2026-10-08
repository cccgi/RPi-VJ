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
const int i_max = 120;
vec2 c = vec2( -0.01625, -0.6875);
// vec2 c = vec2( -0.625, -0.9875);
float threshold = 10.0;


// vec3 palette( in float t, in vec3 a, in vec3 b, in vec3 c, in vec3 d )
// {
//     return a + b*cos( 6.28318*(c*t+d) );
// }

vec2 complex_square( vec2 v ) {
	return vec2(
		v.x * v.x - v.y * v.y,
		v.x * v.y * 2.0
	);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    
	vec2 z = fragCoord.xy - vec3(u_resolution, 1.0).xy * 0.5;
	z *= 2.5 / min( vec3(u_resolution, 1.0).x, vec3(u_resolution, 1.0).y );
	
	
	float scale = 0.1;

	int count = 0;
	
    
    float k = 2.0 + mod(floor(.125), 10.0 );
    vec3 col = vec3(0.0);
    c.x += sin(u_time * .25) * 1.3125;
	for ( int i = 0 ; i < i_max; i++ ) {
        z = c + complex_square( z );
		count = i;
        if ( dot(z,z) > threshold ) {  break; }
	}
    vec3 tmp = vec3(0.0);
   
    if( float(count)<20.5 )
    {
        // smooth iteration count
        float sit = float(count) - log2(log2(dot(z,z))/(log2(threshold)))/log2(k);
        tmp = 0.5 + 0.5*cos( 3.0 + sit*0.075*k + vec3(0.4,0.1,1.));
    } else {
        col += vec3(1.0, 1.0, 1.0);
    }
	
    col += tmp;
    // float shade = 1.0- float( count ) * scale;
    // vec3 col = palette(shade, vec3(0.5,0.5,0.5),vec3(0.5,0.5,0.5),vec3(1.0,1.0,1.0),vec3(0.0,0.33,0.67));
    
    fragColor = vec4(col, 1.0 );
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
