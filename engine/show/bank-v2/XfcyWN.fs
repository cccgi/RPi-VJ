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
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Normalizza le coordinate a un range [-1,1]
    vec2 uv = (fragCoord * 2.0 - vec3(u_resolution, 1.0).xy) / min(vec3(u_resolution, 1.0).x, vec3(u_resolution, 1.0).y);
    
    // Variabile temporale
    float t = u_time * 0.5;

    // Calcola la distanza radiale e l'angolo
    float r = length(uv);
    float angle = atan(uv.y, uv.x);

    // Crea un pattern frattale dinamico
    float f = 0.0;
    vec2 p = uv * 2.0;
    float amp = 1.0;
    for(int i = 0; i < 5; i++) {
        f += sin(p.x + t) * sin(p.y + t) * amp;
        p = p * 2.0 + vec2(t * 0.1);
        amp *= 0.5;
    }

    // Genera colori con tonalità vivaci
    vec3 color;
    color.r = 0.5 + 0.5 * sin(r * 10.0 + angle * 5.0 + t + f);
    color.g = 0.5 + 0.5 * sin(r * 12.0 + angle * 6.0 + t + f + 2.094);
    color.b = 0.5 + 0.5 * sin(r * 14.0 + angle * 7.0 + t + f + 4.188);

    // Aggiungi un effetto di pulsazione
    float pulse = 0.5 + 0.5 * sin(u_time * 2.0 + r * 8.0);
    color *= pulse;

    // Aumenta saturazione e contrasto
    color = pow(color, vec3(0.8));

    // Output finale
    fragColor = vec4(color, 1.0);
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
