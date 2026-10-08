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
vec3 palette(float t, vec3 a, vec3 b, vec3 c, vec3 d)
{
    return a + b*cos( 6.283185*(c*t+d) );
}

vec3 customColor(float t)
{
    vec3 a = vec3(0.500, 0.500, 0.500);
    vec3 b = vec3(0.500, 0.500, 0.500);
    vec3 c = vec3(3.138, 3.138, 3.138);
    vec3 d = vec3(1.000, 0.333, 0.667);
    return palette(t, a, b, c, d);
}

mat2 rotate(float rad) {
    return mat2(
        cos(rad), -sin(rad),
        sin(rad), cos(rad)
    );
}

float sdCircle(vec2 p, float r)
{
    return length(p) - r;
}

float sdHexagram(vec2 p, float r)
{
    const vec4 k = vec4(-0.5,0.8660254038,0.5773502692,1.7320508076);
    p = abs(p);
    p -= 2.0*min(dot(k.xy,p),0.0)*k.xy;
    p -= 2.0*min(dot(k.yx,p),0.0)*k.yx;
    p -= vec2(clamp(p.x,r*k.z,r*k.w),r);
    return length(p)*sign(p.y);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy;
    uv = uv * 2.0 - 1.0;
    uv.x *= vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y;
    vec2 uv0 = uv;
    
    // Apply rotation, zoom, and repetition.
    uv *= rotate(radians(u_time * 10.0));
    uv *= (sin(u_time * 0.5) * 0.49 + 0.51) * 8.0;
    uv = fract(uv);
    
    // Re-centre uv since it's domain is now [0, 1]
    uv -= 0.5;
    
    // Spiral geometry (Apply opOnion via a periodic function)
    float spiralFrequency = 8.0;
    float spiralThickness = 8.0;
    //float d = sdCircle(uv, 0.0);
    float d = sdHexagram(uv, 1.0);
    d = sin(d * spiralFrequency + u_time) / spiralThickness;
    d = abs(d);
    d = 0.02 / d;
    
    // Colour based on distance from screen centre
    vec3 col = customColor(length(uv0));
    vec3 rgb = col * d;
    fragColor = vec4(rgb, 1.0);
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
