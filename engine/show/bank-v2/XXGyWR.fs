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

// book of shaders iirc
float random (in vec2 _st) {
    return fract(sin(dot(_st.xy,
                         vec2(12.9898,78.233)))*
        43758.5453123);
}


// 2D Noise based on Morgan McGuire @morgan3d
// https://www.shadertoy.com/view/4dS3Wd
float noise (in vec2 st) {
    vec2 i = floor(st);
    vec2 f = fract(st);

    // Four corners in 2D of a tile
    float a = random(i);
    float b = random(i + vec2(1.0, 0.0));
    float c = random(i + vec2(0.0, 1.0));
    float d = random(i + vec2(1.0, 1.0));

    // Smooth Interpolation

    // Cubic Hermine Curve.  Same as SmoothStep()
    vec2 u = f*f*(3.0-2.0*f);
    // u = smoothstep(0.,1.,f);

    // Mix 4 coorners percentages
    return mix(a, b, u.x) +
            (c - a)* u.y * (1.0 - u.x) +
            (d - b) * u.x * u.y;
}

// plasma + noise
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    float t = u_time;
    vec2 st = fragCoord.xy/vec3(u_resolution, 1.0).xy - .5;
    st *= 35.0;

    float n = noise(st);
    float k = cos(t *.2) * (cos(t*.001));
    float l = sin(t*.8) * (cos(t*.7));
    mat2 m = mat2(k,-l,l,k); // Rotate
    st *= m;

    vec3 color = vec3(sin(t)*.5+.5)+.5;

    float x1 = st.x*sin(t*.37837);
    float x2 = st.x*sin(t*.94873);
    float x3 = st.x*sin(t*.73834);
    float x4 = st.x*sin(t*.98484);

    float y1 = st.y*.37837;
    float y2 = st.y*.94873;
    float y3 = st.y*.73834;
    float y4 = st.y*.98484;

    float xx1 = (.37837)*n*n;
    float xx2 = (.94873)*n*n;
    float xx3 = (.73834)*n*n;
    float xx4 = (.98484)*n*n;

    float yy1 = (.37837)*n*n;
    float yy2 = (.94873)*n*n;
    float yy3 = (.73834)*n*n;
    float yy4 = (.98484)*n*n;

    float d1 = sin(1.0)*distance(vec2(x1, y1), vec2(xx1, yy1));
    float d2 = sin(d1)*distance(vec2(x2, y2), vec2(xx2, yy2));
    float d3 = sin(d2)*distance(vec2(x3, y3), vec2(xx3, yy3));
    float d4 = sin(d3)*distance(vec2(x4, y4), vec2(xx4, yy4));
    float c = sin(d1) * sin(d2) * sin(d3) * sin(d4) * noise(st);

    color *= vec3(c*c, c/2.0, c)*8.0;

    fragColor = vec4((color),1.0);
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
