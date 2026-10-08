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
/**
 * Escape time algorithm for the Sierpinski triangle.
 * 
 * This is based on an algorithm found in Fractals
 * Everywhere 2nd Edition by Michael F. Barnsley
 * in section VII.1. I translated the code
 * to GLSL and 
 */


#define MAX_ITERS 20.0
#define RADIUS 1.0
#define CENTER (vec3(u_resolution, 1.0).xy / 2.0)

vec2 inverse_sierpinski(vec2 point) {
    //Inverse transform 1: NW
    if (point.y > 0.0)
        return 2.0 * point + vec2(0.5, -0.5);
    else if (point.x > 0.0)
        return 2.0 * point + vec2(-0.5, 0.5);
    else
        return 2.0 * point + vec2(0.5, 0.5);
}

float escape_time(vec2 point) {
    
    vec2 z = point;
    for (float i = 0.0; i < MAX_ITERS; i++) {
        z = inverse_sierpinski(z);
        
        
        if (length(z) > RADIUS)
            return i;
    }
    return 0.0;
}

float normcos(float x) {
    return 0.5 + 0.5 * cos(x);
}

float normsin(float x) {
    return 0.5 + 0.5 * sin(x);
}

vec4 palette(float n) {
    /*
    vec4 result;
    //result.r = normcos(n - 3.0 * u_time);
    //result.g = normsin(n - 2.0 * u_time);
    //result.b = normcos(2.0 * n - u_time);
    return result;
	*/
    return vec4(0.0, 0.75 * normcos(n - u_time), 0.75 * normsin(n - u_time), 1.0);
}


void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = (fragCoord - CENTER)/vec3(u_resolution, 1.0).y;
    uv *= 1.5;
    
    float n = escape_time(uv);
    
    fragColor = palette(n);
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
