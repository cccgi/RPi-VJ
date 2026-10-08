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
// Radial Fractal
// By Noztol

mat2 rot(float a) {
    float s = sin(a), c = cos(a);
    return mat2(c, -s, s, c);
}

vec3 getDynamicColor(float radius, float time) {
    float cycle = sin(time * 0.8) * 0.5 + 0.5;
    
    vec3 magenta = vec3(1.0, 0.0, 0.6);
    vec3 yellow  = vec3(1.0, 0.7, 0.0);
    vec3 red     = vec3(1.0, 0.05, 0.0);
    vec3 green   = vec3(0.0, 1.0, 0.1);
    vec3 blue    = vec3(0.0, 0.4, 1.0); // A tight blue core keeps it grounded
    
    float centerWidth = mix(0.1, 0.35, cycle);
    vec3 bloomColor = mix(magenta, yellow, radius * 3.0); 
    vec3 centerColor = mix(blue, bloomColor, cycle);
    
    // 2. Middle Phase (Red Expansion)
    // Red is the bridge. When center is small, red pushes very close to the middle.
    float redBlend = smoothstep(centerWidth - 0.1, centerWidth + 0.1, radius);
    vec3 midStage = mix(centerColor, red, redBlend);
    
    // 3. Outer Phase (Green Returns vs Red Takes Over)
    float greenStart = mix(0.85, 0.45, cycle); 
    float greenBlend = smoothstep(greenStart, greenStart + 0.3, radius);
    
    return mix(midStage, green, greenBlend);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = (fragCoord - 0.5 * vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    uv *= 2.5;
    
    float origDist = length(uv);
    vec3 finalColor = vec3(0.0);
    vec2 p = uv;
    
    float time = u_time * 0.15; 
    float rays = 12.0;
    float angleStep = 6.2831853 / rays;
    
    // Fractal Iteration Loop
    for (int i = 0; i < 4; i++) {
        // Polar folding
        float a = atan(p.y, p.x);
        float r = length(p);
        a = mod(a + angleStep/2.0, angleStep) - angleStep/2.0;
        p = r * vec2(cos(a), sin(a));
        
        // Kaleidoscopic spatial folding
        p = abs(p);
        
        p.x -= 0.6;
        p *= 1.35;
        p *= rot(0.2 + time); 
        
        float lineDist = length(p.y); 
        float glow = 0.015 / (lineDist + 0.005);
        
        float intensity = glow * (1.0 - float(i) / 10.0);
        
        finalColor += getDynamicColor(origDist, u_time) * intensity;
    }
    
    finalColor *= smoothstep(2.2, 0.6, origDist);
    fragColor = vec4(1.0 - exp(-finalColor), 1.0);
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
