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
#define MAX_ZOOM 15.

vec2 N(float angle) {
	return vec2(sin(angle), cos(angle));
}

float sigmoid(float x) {
	return 1./(1.+.6*exp(-x));
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = (fragCoord-.5*vec3(u_resolution, 1.0).xy)/vec3(u_resolution, 1.0).y;
    vec2 mouse = iMouse.xy/vec3(u_resolution, 1.0).xy;
 
    float mult = 1. + MAX_ZOOM*sigmoid(sin(u_time*.08)*5.);
    uv *= mult;
    
    vec3 col = vec3(0);
       
    vec2 n;
    float d;
    
    for (int i = 0; i < int(mult); i++) {
        uv = abs(uv);
        vec2 delta = vec2(.25,0.); // Distance between repetitions - increase this to make then repeat farther away from each other
        uv -= delta;
        n = N((1./3.)*3.1415);
        d = dot(uv-vec2(.5,0.), n);
        uv -= n*max(0., d)*2.;
        uv += delta;
    }
    
    uv.x = abs(uv.x);
    uv.y += tan((5./6.)*3.1415)*.5;
    
    n = N((5./6.)*3.1415);
    d = dot(uv-vec2(.5,0.), n);
    uv -= n*max(0., d)*2.;
    
    n = N((2./3.) * 3.1415);
    uv.x += .5;
    float scale = 1.;
    for (int i = 0; i < 4; i++) {
        uv *= 3.;
        scale *= 3.;
        uv.x -= 1.5;

        // The shape is symmetrical around the x-axis, so we only have
        // to worry about half of the x coordinates
        uv.x = abs(uv.x);
        // Make the line wider (both sides will measure .5 more)
        uv.x -= .5;
        // Bend the line around a normal
        uv -= n*min(0., dot(uv, n))*2.;
    }
    
    d = length(uv - vec2(clamp(uv.x, -1., 1.), 0));
    col += smoothstep(1./vec3(u_resolution, 1.0).y, .0, d/(scale*mult));
    uv /= scale;
    col += texture(iChannel0, uv*2.-u_time*.02).rgb;

    fragColor = vec4(col,1.0);
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
