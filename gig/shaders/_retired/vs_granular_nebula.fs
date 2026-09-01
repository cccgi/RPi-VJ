/*{
    "DESCRIPTION": "3D Volumetric Raymarched Granular Fluid Nebula (Inspired by VS Granular)",
    "CATEGORIES": ["3D", "Volumetric", "Granular", "Fluid", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "fluid_speed",
            "TYPE": "float",
            "DEFAULT": 1.0,
            "MIN": 0.2,
            "MAX": 3.0
        },
        {
            "NAME": "granularity",
            "TYPE": "float",
            "DEFAULT": 2.0,
            "MIN": 0.5,
            "MAX": 5.0
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

// 3D Noise function for volumetric smoke/fluid
float hash(vec3 p) {
    p = fract(p * 0.3183099 + 0.1);
    p *= 17.0;
    return fract(p.x * p.y * p.z * (p.x + p.y + p.z));
}

float noise3D(vec3 x) {
    vec3 p = floor(x);
    vec3 w = fract(x);
    vec3 u = w * w * (3.0 - 2.0 * w);
    
    float n = p.x + p.y * 157.0 + 113.0 * p.z;
    return mix(
        mix(mix(hash(p + vec3(0,0,0)), hash(p + vec3(1,0,0)), u.x),
            mix(hash(p + vec3(0,1,0)), hash(p + vec3(1,1,0)), u.x), u.y),
        mix(mix(hash(p + vec3(0,0,1)), hash(p + vec3(1,0,1)), u.x),
            mix(hash(p + vec3(0,1,1)), hash(p + vec3(1,1,1)), u.x), u.y), u.z
    );
}

// Fractional Brownian Motion for turbulent grain clouds
float fbm(vec3 p) {
    float f = 0.0;
    float amp = 0.5;
    for (int i = 0; i < 4; i++) {
        f += amp * noise3D(p);
        p = p * 2.02 + vec3(0.0, u_time * 0.1, 0.0);
        amp *= 0.5;
    }
    return f;
}

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / u_resolution.y;
    
    vec3 ro = vec3(0.0, 0.0, -3.0);
    vec3 rd = normalize(vec3(uv, 1.2));
    
    float t = u_time * fluid_speed * 0.4;
    vec3 col = vec3(0.0);
    float density = 0.0;
    
    // Volumetric Smoke Raymarching
    for (int i = 0; i < 45; i++) {
        float depth = float(i) * 0.1;
        vec3 p = ro + rd * (2.0 + depth);
        
        // Domain warping with audio bass
        p += vec3(sin(p.z * 1.5 + t), cos(p.x * 1.5 + t), sin(p.y * 1.5)) * (0.3 + u_bass * 0.4);
        
        float d = fbm(p * granularity);
        d = smoothstep(0.35, 0.85, d + u_bass * 0.3);
        
        if (d > 0.01) {
            vec3 cloudCol = mix(vec3(0.1, 0.8, 1.0), vec3(1.0, 0.1, 0.6), sin(depth * 3.0 + t) * 0.5 + 0.5);
            cloudCol += vec3(1.0, 0.8, 0.2) * pow(d, 3.0) * (0.8 + u_onset);
            
            float stepDensity = d * 0.08 * (1.0 - depth * 0.2);
            col += cloudCol * stepDensity * (1.0 - density);
            density += stepDensity;
            
            if (density > 0.95) break;
        }
    }
    
    col += vec3(u_onset * 0.25);
    gl_FragColor = vec4(col, min(density * 1.5, 1.0));
}
