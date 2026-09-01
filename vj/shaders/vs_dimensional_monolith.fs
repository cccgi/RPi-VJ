/*{
    "DESCRIPTION": "3D Raymarched Brutalist Monoliths with Volumetric Lighting & Floor Reflection (Inspired by VS Dimensional)",
    "CATEGORIES": ["3D", "Raymarching", "Brutalist", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "camera_speed",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.1,
            "MAX": 3.0
        },
        {
            "NAME": "glow_color",
            "TYPE": "color",
            "DEFAULT": [0.0, 0.85, 1.0, 1.0]
        }
    ]
}*/

uniform float u_time;
uniform vec2  u_resolution;
uniform float u_bass;
uniform float u_mid;
uniform float u_treble;
uniform float u_onset;
uniform float u_beat_phase;

// Smooth minimum for organic geometric blending
float smin(float a, float b, float k) {
    float h = clamp(0.5 + 0.5 * (b - a) / k, 0.0, 1.0);
    return mix(b, a, h) - k * h * (1.0 - h);
}

// Rotation matrix
mat2 rot(float a) {
    float c = cos(a), s = sin(a);
    return mat2(c, -s, s, c);
}

// 3D Scene Signed Distance Field
float map(vec3 p) {
    vec3 q = p;
    
    // Domain repetition for infinite architectural pillars
    q.xz = mod(q.xz + 4.0, 8.0) - 4.0;
    
    // Audio-reactive height displacement
    float height = 4.0 + sin(p.x * 0.5 + u_time) * cos(p.z * 0.5 + u_time) * (2.0 + u_bass * 2.5);
    
    // Box SDF
    vec3 d = abs(q) - vec3(0.8 + u_mid * 0.3, height, 0.8 + u_mid * 0.3);
    float box = min(max(d.x, max(d.y, d.z)), 0.0) + length(max(d, 0.0));
    
    // Reflective floor SDF
    float floorDist = p.y + 2.5;
    
    // Central floating glowing core
    vec3 corePos = p;
    corePos.y -= 1.0 + sin(u_time * 2.0) * 0.5;
    corePos.xz = mod(corePos.xz + 8.0, 16.0) - 8.0;
    corePos.xz *= rot(u_time + u_bass);
    corePos.xy *= rot(u_time * 0.7);
    vec3 cd = abs(corePos) - vec3(0.6 + u_onset * 0.4);
    float core = min(max(cd.x, max(cd.y, cd.z)), 0.0) + length(max(cd, 0.0));
    
    return min(smin(box, floorDist, 0.8), core);
}

// Calculate surface normal
vec3 calcNormal(vec3 p) {
    vec2 e = vec2(0.002, 0.0);
    return normalize(vec3(
        map(p + e.xyy) - map(p - e.xyy),
        map(p + e.yxy) - map(p - e.yxy),
        map(p + e.yyx) - map(p - e.yyx)
    ));
}

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / u_resolution.y;
    
    // Audio-reactive chromatic aberration
    vec2 uvR = uv * (1.0 + u_onset * 0.03);
    vec2 uvB = uv * (1.0 - u_onset * 0.03);
    
    // 3D Camera Setup
    float t = u_time * camera_speed * 0.5;
    vec3 ro = vec3(sin(t * 0.5) * 4.0, 1.5 + sin(t * 0.3) * 0.8, t * 6.0);
    vec3 target = ro + vec3(sin(t * 0.5 + 0.2), -0.1, cos(t * 0.5 + 0.2));
    
    vec3 ww = normalize(target - ro);
    vec3 uu = normalize(cross(ww, vec3(0.0, 1.0, 0.0)));
    vec3 vv = normalize(cross(uu, ww));
    vec3 rd = normalize(uv.x * uu + uv.y * vv + (1.2 + u_bass * 0.2) * ww);
    
    // Volumetric Raymarch Loop
    float dist = 0.0;
    float glow = 0.0;
    vec3 col = vec3(0.0);
    
    for (int i = 0; i < 70; i++) {
        vec3 p = ro + rd * dist;
        float d = map(p);
        
        // Accumulate volumetric neon glow around pillars
        glow += exp(-d * 3.0) * (0.015 + u_onset * 0.03);
        
        if (d < 0.001) {
            vec3 n = calcNormal(p);
            vec3 lightDir = normalize(vec3(sin(u_time), 2.0, cos(u_time)));
            
            // Diffuse & Specular Lighting
            float diff = max(dot(n, lightDir), 0.0);
            float spec = pow(max(dot(reflect(-lightDir, n), -rd), 0.0), 16.0);
            
            // Neon Edge Fresnel
            float fresnel = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
            
            vec3 matCol = mix(vec3(0.05, 0.06, 0.1), glow_color.rgb, fresnel * (0.8 + u_mid));
            col = matCol * (diff + 0.2) + vec3(1.0) * spec * (0.6 + u_treble);
            break;
        }
        
        if (dist > 40.0) break;
        dist += max(d * 0.7, 0.02);
    }
    
    // Volumetric Depth Fog & Audio Glow Integration
    vec3 fogColor = mix(vec3(0.01, 0.02, 0.05), glow_color.rgb, 0.15);
    col = mix(col, fogColor, 1.0 - exp(-0.04 * dist));
    col += glow_color.rgb * glow * (1.0 + u_bass * 2.0);
    col += vec3(u_onset * 0.3); // Strobe flash on downbeat
    
    gl_FragColor = vec4(col, 1.0);
}
