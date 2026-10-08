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
// Radiant Furnace — Shadertoy First Edition
// Adapted from the Fractals "sceneGridHymn" family.
// Built as a hotter, denser sibling to Harmonic Grid.

#define PI  3.141592653589793
#define TAU 6.283185307179586

mat2 rot(float a) {
    float s = sin(a);
    float c = cos(a);
    return mat2(c, -s, s, c);
}

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);

    float a = hash21(i);
    float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0));
    float d = hash21(i + vec2(1.0, 1.0));

    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    for (int i = 0; i < 2; i++) {
        v += a * noise(p);
        p = p * 2.03 + vec2(7.13, 3.71);
        a *= 0.5;
    }
    return v;
}

vec2 aspectUv(vec2 fragCoord, float zoom) {
    vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy;
    vec2 p = (uv - 0.5) * vec2(vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y, 1.0);
    return p * zoom;
}

vec3 furnacePalette(float t) {
    t = clamp(t, 0.0, 1.0);

    vec3 deep   = vec3(0.03, 0.01, 0.005);
    vec3 ember  = vec3(0.32, 0.06, 0.01);
    vec3 fire   = vec3(0.90, 0.32, 0.03);
    vec3 gold   = vec3(1.00, 0.78, 0.24);
    vec3 whiteH = vec3(1.00, 0.96, 0.82);

    vec3 col = mix(deep, ember, smoothstep(0.00, 0.22, t));
    col = mix(col, fire, smoothstep(0.18, 0.55, t));
    col = mix(col, gold, smoothstep(0.48, 0.82, t));
    col = mix(col, whiteH, smoothstep(0.82, 1.00, t));
    return col;
}

vec3 sceneRadiantFurnace(vec2 fragCoord, float time) {
    // Radiant Furnace family index = 36 in your registry.
    // sceneGridHymn uses local = mod(family, 5.0), so local = 1.0 here.
    float local   = 1.0;
    float variant = 6.0;

    vec2 p = aspectUv(fragCoord, 1.22 + local * 0.10 + variant * 0.04);
    p *= rot(time * (0.02 + local * 0.008));

    float gridScale = 2.6 + local * 0.45 + variant * 0.12;
    vec2 gp = abs(fract(p * gridScale) - 0.5);

    float lineCore = min(gp.x, gp.y);
    float lines = exp(-16.0 * lineCore * (1.0 + local * 0.12));

    float waves  = sin((p.x + p.y) * (6.0 + local * 1.6) + time * (1.0 + variant * 0.10));
    float pulses = sin(length(p) * (10.0 + local * 2.4) - time * (1.7 + local * 0.10));
    float mist   = fbm(p * (2.0 + local * 0.35));

    float radialHeat = exp(-1.8 * length(p));
    float emberNoise = fbm(p * 3.2 + vec2(time * 0.06, -time * 0.03));
    float sparks = pow(max(noise(p * 20.0 + time * 0.35) - 0.87, 0.0) * 6.0, 2.0);

    float v = 0.0;
    v += lines * 0.42;
    v += waves * 0.14;
    v += pulses * 0.18;
    v += mist * 0.20;
    v += radialHeat * 0.18;
    v += emberNoise * 0.14;
    v += sparks * 0.08;

    v = clamp(v, 0.0, 1.0);

    float r = length(p);
    float furnaceGlow = smoothstep(1.6, 0.0, r);
    float chamber = 0.65 + 0.35 * furnaceGlow;

    vec3 col = furnacePalette(pow(v, 0.88));
    col *= chamber;

    // Hot line filaments
    col += vec3(1.00, 0.55, 0.12) * pow(lines, 3.0) * 0.12;

    // Interior glow
    col += vec3(1.00, 0.35, 0.05) * radialHeat * 0.10;

    // Spark highlights
    col += vec3(1.00, 0.85, 0.45) * sparks * 0.10;

    // Slight bloom-like shaping
    col *= 0.85 + 0.25 * furnaceGlow;

    return col;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float time = u_time;

    vec3 col = sceneRadiantFurnace(fragCoord, time);

    // Gentle display polish
    col = pow(max(col, 0.0), vec3(0.94));

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
