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
// Enhanced noise and visual effects shader with improved structure and performance

// Constants for better maintainability
const float TWO_PI = 6.28318530718;
const float GOLDEN_RATIO = 1.618033988749;
const vec2 HASH_SEED = vec2(127.1, 311.7);
const vec3 HUE_OFFSETS = vec3(0.0, 0.333, 0.666);

// Improved hash function with better distribution
float hash21(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// High-quality noise with quintic interpolation
float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    
    // Sample grid corners
    float a = hash21(i);
    float b = hash21(i + vec2(1.0, 0.0));
    float c = hash21(i + vec2(0.0, 1.0));
    float d = hash21(i + vec2(1.0, 1.0));
    
    // Quintic interpolation for smoother results
    vec2 u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// Fractal noise with octaves for more detail
float fbm(vec2 p, int octaves) {
    float value = 0.0;
    float amplitude = 0.5;
    float frequency = 1.0;
    
    for (int i = 0; i < octaves; i++) {
        value += amplitude * noise(p * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    
    return value;
}

// Improved quantization with adjustable smoothness
float quantize(float x, float levels, float smoothness) {
    float scaled = x * levels;
    float floored = floor(scaled);
    float frac = fract(scaled);
    float smooth_frac = pow(frac, smoothness);
    return (floored + smooth_frac) / levels;
}

// Enhanced HSV to RGB conversion
vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

// Radial distortion field
vec2 calculateRadialWarp(vec2 uv, vec2 center, float time) {
    vec2 dir = uv - center;
    float dist = length(dir);
    
    // Dynamic radius based on time
    float radius = 0.25 + 0.1 * sin(time * 0.3);
    float falloff = smoothstep(radius * 1.5, 0.0, dist);
    
    // Add some rotation to the warp
    float rotation = time * 0.5 + dist * 2.0;
    vec2 rotated_dir = vec2(
        dir.x * cos(rotation) - dir.y * sin(rotation),
        dir.x * sin(rotation) + dir.y * cos(rotation)
    );
    
    return normalize(rotated_dir) * falloff * 3.0;
}

// Enhanced interference pattern
float calculateInterference(vec2 uv, float time) {
    float wave1 = sin(uv.y * 150.0 + time * 2.5);
    float wave2 = sin(uv.x * 80.0 + time * 1.8);
    float wave3 = sin((uv.x + uv.y) * 60.0 + time * 3.2);
    
    return (wave1 + wave2 * 0.7 + wave3 * 0.5) * 0.3 + 0.5;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    // Setup UV coordinates
    vec2 uv = (fragCoord.xy - 0.5 * vec3(u_resolution, 1.0).xy) / min(vec3(u_resolution, 1.0).x, vec3(u_resolution, 1.0).y);
    vec2 mouseUV = (iMouse.xy - 0.5 * vec3(u_resolution, 1.0).xy) / min(vec3(u_resolution, 1.0).x, vec3(u_resolution, 1.0).y);
    
    float time = u_time;
    
    // Enhanced displacement parameters
    float displacementAmount = 0.015;
    float displacementFreq = 8.0;
    
    // Calculate radial warp with time-based animation
    vec2 radialWarp = calculateRadialWarp(uv, mouseUV, time);
    
    // Multi-octave noise for richer texture
    vec2 noiseInput = uv * displacementFreq + radialWarp;
    float noiseX = fbm(noiseInput + vec2(time * 0.7, 0.0), 2);
    float noiseY = fbm(noiseInput + vec2(0.0, time * 0.7), 2);
    
    // Quantize with variable smoothness
    float quantLevels = 6.0 + 2.0 * sin(time * 0.4);
    float smoothness = 1.5 + 0.5 * sin(time * 0.6);
    
    vec2 quantNoise = vec2(
        quantize(noiseX, quantLevels, smoothness),
        quantize(noiseY, quantLevels, smoothness)
    );
    
    // Apply displacement
    uv += (quantNoise - 0.5) * displacementAmount;
    
    // Enhanced interference pattern
    float interference = calculateInterference(uv, time);
    
    // Dynamic color calculation
    float angle = atan(uv.y, uv.x);
    float hue = fract(angle / TWO_PI + interference * 0.3 + time * 0.08);
    
    // Enhanced saturation and brightness
    float saturation = 0.8 + 0.2 * sin(length(uv) * 5.0 + time);
    float brightness = 0.7 + 0.3 * interference;
    
    vec3 color = hsv2rgb(vec3(hue, saturation, brightness));
    
    // Multi-layered brushed metal effect
    float brushed1 = sin((uv.x + uv.y) * 120.0 + time * 8.0) * 0.08;
    float brushed2 = sin((uv.x - uv.y) * 200.0 + time * 12.0) * 0.05;
    color += brushed1 + brushed2;
    
    // Add subtle vignette
    float vignette = 1.0 - smoothstep(0.7, 1.2, length(uv));
    color *= vignette;
    
    // Output with slight gamma correction
    fragColor = vec4(pow(color, vec3(0.95)), 1.0);
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
