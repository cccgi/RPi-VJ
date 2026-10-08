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
// Fractal Bang
// By Noztol

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = (fragCoord - 0.5 * vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;

    float cycleDuration = 10.0;
    float t = fract(u_time / cycleDuration); // Loops from 0.0 to 1.0
    float env = pow(sin(t * 3.14159265), 0.3);
    float flash = exp(-80.0 * t) + exp(-80.0 * (1.0 - t));
    vec2 p = uv * (3.5 - 2.0 * env);

    // Polar Folding
    float r = length(p);
    float a = atan(p.y, p.x);
    
    // 8-fold symmetry 
    float petals = 8.0;
    float sector = 6.2831853 / petals;
    
    // Fold space into wedges
    a = mod(a + u_time * 0.15, sector) - sector / 2.0;
    p = r * vec2(cos(a), sin(a));

    vec3 col = vec3(0.0);
    
    for (float i = 0.0; i < 7.0; i++) {
        // Kaleidoscopic mirroring
        p = abs(p);

        // Spherical space inversion
        float r2 = dot(p, p);
        float scale = 1.3 / clamp(r2, 0.1, 1.0);
        
        // Pushing the fractal outward and inward
        p = p * scale - vec2(0.6, 0.9) * env; 

        // Dynamic color palette
        vec3 palette = 0.5 + 0.5 * cos(u_time * 0.4 + i * 0.8 + vec3(0.0, 1.5, 3.0));
        
        // Makethe lines cast light
        float crispLines = 0.04 / (abs(p.y) + 0.005) + 0.04 / (abs(p.x) + 0.005);
        float crispPoints = 0.06 / (length(p) + 0.01);
        
        // Massively boosted the final accumulator (from 0.12 to 0.35)
        col += palette * (crispLines + crispPoints) * (0.35 * env);
    }

    col *= env;

    // colapse to singularity
    float dist = length(uv);
    vec3 core = vec3(1.0, 0.85, 0.6) * (flash * 0.04 / (dist + 0.001));
    
    // Shockwave ring
    float ring = smoothstep(0.015, 0.0, abs(dist - env * 1.5));
    col += vec3(0.2, 0.5, 1.0) * ring * flash * 2.0;

    col += core;

    // Deep space vignette
    col *= smoothstep(2.2, 0.4, dist);

    // Lifted the gamma curve to boost midtones
    fragColor = vec4(pow(col, vec3(0.8)), 1.0);
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
