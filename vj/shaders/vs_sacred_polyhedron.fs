/*{
    "DESCRIPTION": "3D Raymarched Audio-Reactive Sacred Polyhedron Shard Matrix (Inspired by VS Battles)",
    "CATEGORIES": ["3D", "Sacred Geometry", "Raymarching", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "rotation_speed",
            "TYPE": "float",
            "DEFAULT": 1.2,
            "MIN": 0.2,
            "MAX": 4.0
        },
        {
            "NAME": "shard_dispersion",
            "TYPE": "float",
            "DEFAULT": 1.5,
            "MIN": 0.5,
            "MAX": 3.5
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

// Polyhedron SDF with facet cutting
float sdPolyhedron(vec3 p, float r) {
    // Icosahedron facet cutting planes
    const float phi = 1.61803398875;
    vec3 n1 = normalize(vec3(phi, 1.0, 0.0));
    vec3 n2 = normalize(vec3(0.0, phi, 1.0));
    vec3 n3 = normalize(vec3(1.0, 0.0, phi));
    
    p = abs(p);
    float d = dot(p, n1) - r;
    d = max(d, dot(p, n2) - r);
    d = max(d, dot(p, n3) - r);
    return d;
}

float map(vec3 p) {
    // Audio-reactive rotation & breathing
    float t = u_time * rotation_speed;
    p.xy *= rot(t * 0.4);
    p.yz *= rot(t * 0.6 + u_bass * 0.5);
    p.xz *= rot(t * 0.5);
    
    // Kick drum explosive dispersion
    float burst = u_onset * shard_dispersion * 0.8;
    
    // Core Sacred Polyhedron
    float core = sdPolyhedron(p, 1.2 + u_bass * 0.4);
    
    // Orbiting Geometric Shards
    vec3 pShard1 = p + vec3(sin(t * 2.0), cos(t * 2.0), 0.0) * (1.8 + burst);
    pShard1.xy *= rot(t * 3.0);
    float shard1 = sdPolyhedron(pShard1, 0.4 + u_mid * 0.2);
    
    vec3 pShard2 = p - vec3(cos(t * 1.5), 0.0, sin(t * 1.5)) * (2.2 + burst);
    pShard2.yz *= rot(t * 2.5);
    float shard2 = sdPolyhedron(pShard2, 0.35 + u_treble * 0.2);
    
    return min(core, min(shard1, shard2));
}

vec3 calcNormal(vec3 p) {
    vec2 e = vec2(0.002, 0.0);
    return normalize(vec3(
        map(p + e.xyy) - map(p - e.xyy),
        map(p + e.yxy) - map(p - e.yxy),
        map(p + e.yyx) - map(p - e.yyx)
    ));
}

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / min(u_resolution.x, u_resolution.y);
    
    vec3 ro = vec3(0.0, 0.0, -4.5 + u_bass * 0.5);
    vec3 rd = normalize(vec3(uv, 1.3));
    
    float dist = 0.0;
    float glow = 0.0;
    vec3 col = vec3(0.0);
    
    for (int i = 0; i < 60; i++) {
        vec3 p = ro + rd * dist;
        float d = map(p);
        
        // Volumetric energy aura
        glow += exp(-d * 4.0) * (0.02 + u_onset * 0.04);
        
        if (d < 0.001) {
            vec3 n = calcNormal(p);
            
            // Fresnel Wireframe Neon Edge Glow
            float fresnel = pow(1.0 - max(dot(n, -rd), 0.0), 2.5);
            
            vec3 edgeCol = mix(vec3(1.0, 0.1, 0.4), vec3(0.0, 0.9, 1.0), sin(p.y * 3.0 + u_time) * 0.5 + 0.5);
            vec3 baseCol = vec3(0.02, 0.03, 0.06);
            
            col = mix(baseCol, edgeCol, fresnel * (1.2 + u_mid));
            col += vec3(1.0) * pow(fresnel, 6.0) * (0.8 + u_treble);
            break;
        }
        
        if (dist > 15.0) break;
        dist += max(d * 0.8, 0.015);
    }
    
    // Combine geometry with volumetric aura and kick transient flash
    vec3 auraColor = mix(vec3(0.1, 0.0, 0.3), vec3(0.0, 0.8, 1.0), u_bass);
    col += auraColor * glow * (1.5 + u_bass * 2.0);
    col += vec3(u_onset * 0.35);
    
    gl_FragColor = vec4(col, length(col) > 0.02 ? 1.0 : 0.0);
}
