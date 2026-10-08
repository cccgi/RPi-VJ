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

#define palette(x) cosPalette(x,vec3(0.5, 0.5, 0.5),vec3(0.5, 0.5, 0.5),vec3(2.0, 1.0, 0.0),vec3(0.50, 0.20, 0.25))
#define PI_l 3.14159265358979323846264338327950288420
#define SQRT2 1.41421356237
// set to false to try aces_tonemap 
#define USE_REINHARD true

vec3 cosPalette(  float t,  vec3 a,  vec3 b,  vec3 c, vec3 d ){
    return a + b*cos( PI_l * 2. *(c*t+d) );
}

// Taken from https://www.shadertoy.com/view/XsGfWV
// by the shader god `iq'
vec3 aces_tonemap(vec3 color){	
	mat3 m1 = mat3(
        0.59719, 0.07600, 0.02840,
        0.35458, 0.90834, 0.13383,
        0.04823, 0.01566, 0.83777
	);
	mat3 m2 = mat3(
        1.60475, -0.10208, -0.00327,
        -0.53108,  1.10813, -0.07276,
        -0.07367, -0.00605,  1.07602
	);
	vec3 v = m1 * color;    
	vec3 a = v * (v + 0.0245786) - 0.000090537;
	vec3 b = v * (0.983729 * v + 0.4329510) + 0.238081;
	return pow(clamp(m2 * (a / b), 0.0, 1.0), vec3(1.0 / 2.2));	
}


void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec3 base = vec3(0.0);
    
    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = (fragCoord/vec3(u_resolution, 1.0).xy);
    uv = (uv - 0.5) * 2.0;
    uv.x *= vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y;
    vec2 uv0 = uv;
  
    for(int i = 0; i<5; ++i){
        
        // produce new fractal
        uv = (fract(uv * SQRT2) - 0.5);

        // get magnitude
        float mag = length(uv) * exp(-length(uv0));
        mag = abs(sin(mag * 8.0 + u_time)/ 8.);
        mag = 0.01/mag;
        mag = pow(mag, 1.2);
        
        // palette sampling
        vec3 col = vec3(palette(length(uv0) + float(i) *.1+ u_time*.1f));
        
        base += (mag * col);
    }
    
    // Reinhard tone map
    if(USE_REINHARD)
    {
        float luma = 0.2126 * base.x +0.7152 * base.y +0.0722 * base.z;
        base = (base / (luma + 1.));

    } else // uses ACES
    {
        base = aces_tonemap(base);
    }

    // Output to screen
    fragColor = vec4(base, 1.0f);
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
