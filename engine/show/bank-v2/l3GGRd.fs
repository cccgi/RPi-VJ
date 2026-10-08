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
#define MANDELBROT_SET (z = vec2(z.x*z.x - z.y*z.y, 2.0*z.x*z.y) + c)
#define BURNING_SHIP (z = vec2(z.x*z.x - z.y*z.y, abs(2.0*z.x*z.y)) + (c*vec2(1., -1.)))
#define TRICORN (z = vec2(z.x*z.x - z.y*z.y, -2.0*z.x*z.y) + c)

#define FRACTAL BURNING_SHIP

#define ABS_OUT (abs(z.x*z.y) > 16.0)
#define DIVERGENCE (dot(z, z) > 4.0)
#define CONVERGENCE (length(z - zn_s_1) < 0.001)
#define DIVERGENCE_CONVERGENCE (DIVERGENCE || CONVERGENCE)
#define ALL_OUT (CONVERGENCE || ABS_OUT)

#define OUT ALL_OUT

// Convert HEX color, to vec3 color
vec3 hexToColor(int hex) {
    float r = float((hex >> 16) & 0xFF) / 255.0;
    float g = float((hex >> 8) & 0xFF) / 255.0;
    float b = float(hex & 0xFF) / 255.0;
    return vec3(r, g, b);
}


// Function to generate a color from a distance
vec3 palette(float dist, vec3 interior, vec3[8] exterior) {
    if (dist == 1.0) {
        return interior;
    }

    float index = mod(dist * 7.0, 7.0);
    int i = int(floor(index));
    float t = fract(index);

    vec3 color = mix(exterior[i], exterior[i+1], t);
    return color;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    // Normalized coordinates
    vec2 uv = fragCoord / vec3(u_resolution, 1.0).y;
    
    // Mapping the coordinates
    vec2 c = uv - vec2(1.5, 0.5);
    
    // Parameters
    vec2 position = vec2(-0.5, 0.0); // Center position
    float scale = 0.2; // Zoom
    
    if (iMouse.z > 0.0) {
        scale *= 30.;
        vec2 m = iMouse.xy / vec3(u_resolution, 1.0).xy;
        m = 1.-m;
        m += vec2(-0.55, -0.5);
        position = m * (scale*4.5);
    }
    
    // Applying position and scale parameters
    c = (c - position) / scale;
    
    vec2 z = vec2(0.0);
    vec2 zn_s_1 = vec2(0.0);
    float iter = 0.0;
    float maxIter = 64.0;
    
    // Fractal algorithm
    for(int i = 0; i < int(maxIter); i++) {
        zn_s_1 = z;
        FRACTAL;
        if(OUT) {
            break;
        }
        iter++;
    }
    
    // Palette
    float iterations = float(iter) / maxIter;
    vec3 interior;
    vec3 exterior[8];
    interior = vec3(0.0);
    exterior[0] = hexToColor(0xf0b6ad);
    exterior[1] = hexToColor(0xdc8864);
    exterior[2] = hexToColor(0xba4848);
    exterior[3] = hexToColor(0xc75a1b);
    exterior[4] = hexToColor(0xf7c435);
    exterior[5] = hexToColor(0x818b2e);
    exterior[6] = hexToColor(0x0b5227);
    exterior[7] = hexToColor(0x85a993);
    
    vec3 col = palette(iterations, interior, exterior);
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
