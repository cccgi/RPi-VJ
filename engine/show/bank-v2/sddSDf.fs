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
#define PI 3.14159265358979323

// i derived them so idk if theyre actually correct
vec2 cmplxmult(vec2 a, vec2 b) { // complex multiplication
    return vec2(a.x*b.x - a.y*b.y, a.y*b.x + a.x*b.y);
}
vec2 cmplxcjg(vec2 x) { // conjugation
    return vec2(x.x, - x.y);
}
vec2 cmplxdiv(vec2 a, vec2 b) { // complex division a/b
    return cmplxmult(a, cmplxcjg(b)) / (b.x*b.x + b.y*b.y);
}
vec2 cmplxsqr(vec2 x) {
    return vec2(x.x*x.x-x.y*x.y, 2.0*x.x*x.y);
}
//vec2 cmplxexp()
// z_n+1 = z_n^2 + c
vec2 C = vec2(0.0, 1.0);
// colours
vec3 solcol[3] = vec3[3](vec3(1.0, .0, .0), vec3(.0, 1.0, .0), vec3(.0, .0, 1.0));

vec2 next_step(vec2 x) { // step = -f(x)/f'(x)
    return cmplxsqr(x) + C;
}

const float s = 5.0; // tiles/screenW

void mainImage( out vec4 fragColor, in vec2 fc )
{
    vec2 t = vec3(u_resolution, 1.0).xx/s; // tile dimension
    vec2 uv = fc/vec3(u_resolution, 1.0).xy;
    vec2 c = (fc.xy-(vec3(u_resolution, 1.0).xy/2.0) )/t.xy;
    vec2 fmou = (iMouse.xy-vec3(u_resolution, 1.0).xy/2.0)/t.xy;
    float borderw = 1.0/vec3(u_resolution, 1.0).x*s;
    
    C = fmou - vec2(0.25, 0.0);
    
    vec2 rem = vec2(abs(round(c.x) - c.x), abs(round(c.y) - c.y));
    vec3 col = vec3(0.0); /*(0.25 + 0.5*step(0.5, 1.0-min(round(abs(c.x) + 0.4), round(abs(c.y) + 0.4)))
        ) * vec3(0.0, 1.0, 1.0) * (1.0 - min(
        step(0.5, 1.0-(0.5 + borderw - rem.x) ),
        step(0.5, 1.0-(0.5 + borderw - rem.y) )
    ));*/
    
    vec2 x = vec2(c.x-0.5, c.y-0.0);
    vec2 nstep;
    //float ii = 0.0; // sum
    int i = 0;
    #define max_iter 100
    for (; i <= max_iter; i++) {
        nstep = next_step(x);
        x += nstep;
        //x += next_step(x);

        //ii += -(length(nstep));
        
        if (length(x) >= 4.0) {
            //ii = log(ii);
            //ii -= (length(x) - 4.0);
            break;
        } 
    }
    
    float d = 9999999999999999.0;
    
    #define fff 0.05
    if (i < max_iter) {
        float ii = float(i);
        col += (1.0-pow(0.8, 0.5*(ii)))
            *vec3(sin(ii*fff), sin(ii*fff+PI/3.0), sin(ii*fff+PI/3.0*2.0));
    }
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
