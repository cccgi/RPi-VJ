/*{
    "DESCRIPTION": "3D Raymarched Audio-Reactive Shattered Glitch Core (Optimized)",
    "CATEGORIES": ["3D", "Glitch", "Raymarching", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "glitch_intensity",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.1,
            "MAX": 3.0
        },
        {
            "NAME": "core_speed",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.2,
            "MAX": 3.0
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

mat2 rot(float a) {
    float c = cos(a), s = sin(a);
    return mat2(c, -s, s, c);
}

float sdBox(vec3 p, vec3 b) {
    vec3 q = abs(p) - b;
    return length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0);
}

float map(vec3 p) {
    float t = u_time * core_speed;
    p.xy *= rot(t * 0.3);
    p.yz *= rot(t * 0.4 + u_bass * 0.4);
    
    // Audio glitch displacement slices
    float glitchSlice = step(0.7, fract(p.y * 3.0 + t * 2.0)) * u_onset * glitch_intensity * 0.4;
    p.x += glitchSlice * sin(p.z * 10.0 + t * 8.0);
    
    // Central core box + sphere hybrid
    float box = sdBox(p, vec3(0.85 + u_bass * 0.3));
    float sphere = length(p) - (1.1 + u_mid * 0.3);
    float core = mix(box, sphere, 0.4);
    
    // Outer shattered shell cage
    vec3 pShell = abs(p) - vec3(0.75 + u_onset * 0.3);
    float shell = sdBox(pShell, vec3(0.35 + u_treble * 0.2));
    
    return min(core, shell);
}

// 3-sample fast normal
vec3 calcNormal(vec3 p, float d) {
    const vec2 e = vec2(0.006, 0.0);
    return normalize(vec3(
        map(p + e.xyy) - d,
        map(p + e.yxy) - d,
        map(p + e.yyx) - d
    ));
}

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / min(u_resolution.x, u_resolution.y);
    
    vec3 ro = vec3(0.0, 0.0, -4.2 + u_bass * 0.4);
    vec3 rd = normalize(vec3(uv, 1.2));
    
    float dist = 0.0;
    float glow = 0.0;
    vec3 col = vec3(0.0);
    
    // 28 iterations (2x cut from standard 60-step march)
    for (int i = 0; i < 28; i++) {
        vec3 p = ro + rd * dist;
        float d = map(p);
        
        glow += exp(-d * 3.5) * (0.02 + u_onset * 0.05);
        
        if (d < 0.003) {
            vec3 n = calcNormal(p, d);
            
            float fresnelBase = 1.0 - max(dot(n, -rd), 0.0);
            float fresnel = fresnelBase * fresnelBase;
            
            // RGB chromatic edge aberration
            vec3 glitchCol = vec3(
                sin(p.x * 4.0 + u_time * 3.0) * 0.5 + 0.5,
                sin(p.y * 4.0 + u_time * 2.0 + 2.0) * 0.5 + 0.5,
                sin(p.z * 4.0 + u_time * 4.0 + 4.0) * 0.5 + 0.5
            );
            
            col = mix(vec3(0.03, 0.01, 0.04), glitchCol, fresnel * (1.2 + u_treble));
            col += vec3(1.0) * (fresnel * fresnel) * (0.9 + u_mid);
            break;
        }
        
        if (dist > 14.0) break;
        dist += max(d * 0.95, 0.03);
    }
    
    // Glow and kick transient
    vec3 auraColor = mix(vec3(1.0, 0.05, 0.3), vec3(0.0, 0.8, 1.0), u_bass);
    col += auraColor * glow * (1.4 + u_bass * 1.8);
    col += vec3(u_onset * 0.3);
    
    gl_FragColor = vec4(col, length(col) > 0.02 ? 1.0 : 0.0);
}
