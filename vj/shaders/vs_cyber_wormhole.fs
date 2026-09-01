/*{
    "DESCRIPTION": "3D Relativistic Cyber Wormhole with Audio-Reactive Gravitational Warping (Inspired by VS Cyber Spells)",
    "CATEGORIES": ["3D", "Wormhole", "Cyberpunk", "Audio-Reactive"],
    "INPUTS": [
        {
            "NAME": "warp_intensity",
            "TYPE": "float",
            "DEFAULT": 1.2,
            "MIN": 0.2,
            "MAX": 3.0
        },
        {
            "NAME": "ring_density",
            "TYPE": "float",
            "DEFAULT": 12.0,
            "MIN": 4.0,
            "MAX": 24.0
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

void main() {
    vec2 uv = (gl_FragCoord.xy - 0.5 * u_resolution.xy) / u_resolution.y;
    
    // Audio-reactive chromatic aberration offset
    float chromOffset = (u_onset * 0.04 + u_bass * 0.02);
    
    vec3 col = vec3(0.0);
    
    for (int ch = 0; ch < 3; ch++) {
        vec2 chUv = uv;
        if (ch == 0) chUv *= (1.0 + chromOffset);
        if (ch == 2) chUv *= (1.0 - chromOffset);
        
        float r = length(chUv);
        float a = atan(chUv.y, chUv.x);
        
        // Relativistic Spacetime Curvature
        float depth = 1.0 / (r + 0.04);
        float warpSpeed = u_time * (1.5 + u_bass * 3.0 * warp_intensity);
        
        // Cybernetic Ring Coordinates
        vec2 wuv = vec2(a / 3.14159265 * 4.0 + sin(depth * 0.2 + u_time), depth * 0.5 + warpSpeed);
        wuv.x += sin(wuv.y * 0.5 + u_mid) * 0.3;
        
        // Grid & Glyph Pattern
        float rings = sin(wuv.y * ring_density) * cos(wuv.x * 6.0);
        float glow = smoothstep(0.1, 0.9, rings + u_bass * 0.8);
        
        // Edge lighting & depth falloff
        float channelVal = glow * (1.0 / (r * 2.5 + 0.3));
        
        if (ch == 0) col.r = channelVal * (0.8 + u_onset * 0.6);
        if (ch == 1) col.g = channelVal * (0.4 + u_mid * 0.6);
        if (ch == 2) col.b = channelVal * (1.2 + u_treble * 0.8);
    }
    
    // Core Singularity Energy Glow
    float centerR = length(uv);
    float singularity = exp(-centerR * 6.0) * (1.5 + u_onset * 3.0);
    col += vec3(0.0, 0.9, 1.0) * singularity;
    col += vec3(u_onset * 0.35); // Beat strobe
    
    gl_FragColor = vec4(col, 1.0);
}
