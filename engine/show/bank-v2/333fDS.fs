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
const float PI = 6.28318;

vec3 palette(float t, vec3 a, vec3 b, vec3 c, vec3 d)
{
    return a + b * cos(PI * (c * t + d) );
}

vec3 palette_rainbow_sparse(float t)
{
    vec3 a = vec3(0.5);
    vec3 b = vec3(0.5);
    vec3 c = vec3(1.0);
    vec3 d = vec3(0.0, 0.333, 0.666);
    return palette(t, a, b, c, d);
}

vec3 palette_rainbow_dense(float t)
{
    vec3 a = vec3(0.5);
    vec3 b = vec3(0.5);
    vec3 c = vec3(PI * 0.5);
    vec3 d = vec3(0.0, 0.333, 0.666);
    return palette(t, a, b, c, d);
}

vec3 palette1(float t)
{
    vec3 a = vec3(0.5);
    vec3 b = vec3(0.5);
    vec3 c = vec3(1.0);
    vec3 d = vec3(0.263, 0.416, 0.557);
    return palette(t, a, b, c, d);
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
    vec2 uv = (fragCoord * 2.0 - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    vec2 uv0 = uv; // save original uv before repetition
    
    // In order for our SDF to work, (0, 0) must represent the screen centre
    // However, our uv's range changes from [-1, 1] to [0, 1] when we call fract!
    uv = fract(uv * 4.0); // Range from [-1, 1] to [0, 1]
    uv = uv * 2.0 - 1.0;  // Range from [0, 1] to [-1, 1] <-- uv centred again!
    
    float n = sin(u_time) * 0.4 + 0.6; // [0.2, 1.0]
    
    // Evaluate the distance to whatever shape you'd like to repeat!
    //float d = sdCircle(uv, n);
    float d = sdHexagram(uv, n);
    float dShape = 1.0 - step(0.0, d);
    
    // Add "ripples" to your shape by taking the absolute value of the sin of the distance
    //vec3 col = palette1(d) * dShape;
    //vec3 col = palette1(abs(sin(d * 8.0))) * dShape;
    
    float d0 = 1.0 - smoothstep(0.0, 2.0, length(uv0));
    vec3 col_fg = palette1(abs(sin(d * 8.0))) * dShape * d0;
    vec3 col_bg = palette1(length(uv0) + u_time * 0.25) * d0;
    vec3 col = col_fg + col_bg;
    fragColor = vec4(col, 1.0);
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
