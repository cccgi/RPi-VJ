/*{
    "DESCRIPTION": "3D Fractured Glitch Torus Knot with Chromatic Displacement (Inspired by VS Shattered Illusions)",
    "CATEGORIES": ["3D", "Glitch", "Raymarching", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "glitch_rate",
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

// Torus Knot SDF
float sdTorusKnot(vec3 p) {
    // Audio-reactive twisting
    p.xy *= rot(u_time * 0.5);
    p.yz *= rot(u_time * 0.7 + u_bass * 0.4);
    
    // Glitch slice displacement
    if (mod(p.y * 8.0 + u_time * 10.0, 2.0) < 0.3) {
        p.x += (sin(u_time * 30.0) * 0.15 + u_onset * 0.2) * glitch_rate;
    }
    
    vec2 q = vec2(length(p.xz) - (1.6 + u_bass * 0.3), p.y);
    float a = atan(p.z, p.x);
    q *= rot(a * 1.5 + u_time);
    q.x = abs(q.x) - 0.5;
    return length(q) - (0.2 + u_treble * 0.1);
}

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / min(u_resolution.x, u_resolution.y);
    
    // Pixel-sort glitch scanlines on kick drum
    if (u_onset > 0.4 && mod(gl_FragCoord.y, 16.0) < 4.0) {
        uv.x += sin(uv.y * 40.0 + u_time * 20.0) * 0.08 * u_onset;
    }
    
    vec3 ro = vec3(0.0, 0.0, -4.0);
    vec3 rd = normalize(vec3(uv, 1.2));
    
    float dist = 0.0;
    float glow = 0.0;
    vec3 col = vec3(0.0);
    
    for (int i = 0; i < 55; i++) {
        vec3 p = ro + rd * dist;
        float d = sdTorusKnot(p);
        
        glow += exp(-d * 5.0) * (0.02 + u_onset * 0.03);
        
        if (d < 0.001) {
            vec3 n = normalize(p);
            float fresnel = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
            
            // Gritty metallic surface with electric magenta edge
            vec3 edgeCol = mix(vec3(1.0, 0.05, 0.3), vec3(0.0, 1.0, 0.9), sin(p.z * 4.0 + u_time * 2.0) * 0.5 + 0.5);
            col = vec3(0.05, 0.05, 0.08) + edgeCol * fresnel * (1.5 + u_mid * 1.5);
            col += vec3(1.0) * pow(fresnel, 8.0) * (1.0 + u_treble);
            break;
        }
        
        if (dist > 12.0) break;
        dist += max(d * 0.8, 0.015);
    }
    
    col += vec3(1.0, 0.1, 0.5) * glow * (1.2 + u_bass * 2.0);
    col += vec3(u_onset * 0.3);
    
    gl_FragColor = vec4(col, length(col) > 0.02 ? 1.0 : 0.0);
}
