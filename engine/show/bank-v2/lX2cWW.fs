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
#define R vec3(u_resolution, 1.0)
#define t u_time/1.2
#define rot(a) mat2(cos(a),-sin(a),sin(a),cos(a))

void mainImage( out vec4 o, vec2 i )
{
    vec2 uv = (i - .5 * R.xy)/R.y;
    uv *=2.8*rot(22.+(t)/22.)/cos(sin(t/12.));
    float r = 1.,d=2.;
    vec2 z = uv, p=uv;
    for(int i = 0;i < 28;i++){
       p=vec2(sin(uv.x+t),sin(uv.y+t))*p;
       z = vec2(z.x * z.x - z.y * z.y, 2. * z.x * z.y)-.284-vec2((sin(t)/22.,sin(t)/22.))+p*(sin(t+32.)*cos(t+42.));
       r = min(r,dot(z,z)-dot(p,p));
    }
    r += fract(r*r/d)/r+0.35;
    o = vec4(sqrt(d*d/2.)/r*r*r);
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
