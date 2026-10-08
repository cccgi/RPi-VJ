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
// Perlin Texture Blending
// Based on "Just Some Fog" by Josh1billion
// Original: https://www.shadertoy.com/view/XtfSzs
float random(float p)
{
    return fract(sin(p)*10000.0);
}

float noise(vec2 p) {
    return random(p.x + p.y*10000.0);
}

float stepNoise(vec2 p) {
    return noise(floor(p));
}

vec2 sw(vec2 p) { return vec2( floor(p.x), floor(p.y) ); }
vec2 se(vec2 p) { return vec2( ceil(p.x), floor(p.y) ); }
vec2 nw(vec2 p) { return vec2( floor(p.x), ceil(p.y) ); }
vec2 ne(vec2 p) { return vec2( ceil(p.x), ceil(p.y) ); }

float smoothNoise(vec2 p) {
  vec2 inter = smoothstep(0., 1., fract(p));
  float s = mix(noise(sw(p)), noise(se(p)), inter.x);
  float n = mix(noise(nw(p)), noise(ne(p)), inter.x);
  return mix(s, n, inter.y);
  return noise(nw(p));
}

float fractalNoise(vec2 p) {
  float total = 0.0;
  total += smoothNoise(p);
  total += smoothNoise(p*2.) / 2.;
  total += smoothNoise(p*4.) / 4.;
  total += smoothNoise(p*8.) / 8.;
  total += smoothNoise(p*16.) / 16.;
  total /= 1. + 1./2. + 1./4. + 1./8. + 1./16.;
  return total;
}

float normalizedSin(float x) {
    return abs(sin(x));
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
	vec2 uv = fragCoord.xy / vec3(u_resolution, 1.0).xy;
    
    vec2 p = uv * 6.0;
    float brightness = fractalNoise(p + u_time);
    
    fragColor = mix(texture(iChannel0, uv),texture(iChannel1,uv),brightness);
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
