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
const float PHI = 1.618;
const float EPS = 1e-5;
const int HOX_FOLDS = 1;
const int MAX_ITER = 128;

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    
    vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy;
    vec2 domain = vec2(
        mix(-1.8, 1.2, uv.x),
        mix(-1.5, 1.5, uv.y)
    );

    vec2 center = vec2(-0.3, 0.0);
    domain = (domain - center) * vec2(vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y, 1.0) + center;

    vec2 z = 1514137.0 * exp(-log(1514137.0) * fract(u_time / 60.0)) * domain;

    vec2 c = vec2(-0.745, 0.1);
    
    float theta = PHI - 1.618;
    mat2 rot = mat2(
        cos(theta), -sin(theta),
        sin(theta),  cos(theta)
    );

    float ffp = float(MAX_ITER);
    bool escaped = false;

    for (int i = 0; i < MAX_ITER; i++) {
        vec2 z_prev = z;

        vec2 z_inv = z / (dot(z, z) + EPS);

        vec2 z_folded = abs(z_inv);

        vec2 iz = z_folded / (dot(z_folded, z_folded) + EPS);
        vec2 phase = sin(iz);

        mat2 FFP = mat2(
            z.x,     phase.x,
           -phase.y, z.y
        );
        vec2 z_next = (FFP * z_folded) + phase * 0.15;

        if (i % HOX_FOLDS == 0) {
            vec2 z_sq = vec2(
                z_next.x * z_next.x - z_next.y * z_next.y,
                2.0 * z_next.x * z_next.y
            );
            z_next = (rot * z_sq) + c;
        } else {
            z_next = z_next + c;
        }

        z = z_next;

        if (dot(z, z) >= 4.0) {
            ffp = float(i);
            escaped = true;
            break;
        }
    }
    // thanks to: Inigo Quilez: for palette formula 
    // i'm sure i've ripped so, so much else. that said: palette stood out. so simple! Wish I w/could always attribute!
    float t = ffp / float(MAX_ITER); 
    vec3 color = 0.5 + 0.5 * cos(42.0 * (t * 4.0 + vec3(0.0, 0.33, 0.67)));

    if (!escaped) {
        color = vec3(0.0);
    }

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
