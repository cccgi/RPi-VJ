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
float sdPentagon( in vec2 p, in float r )
{
    const vec3 k = vec3(0.809016994,0.587785252,0.726542528);
    p.x = abs(p.x);
    p -= 2.0*min(dot(vec2(-k.x,k.y),p),0.0)*vec2(-k.x,k.y);
    p -= 2.0*min(dot(vec2( k.x,k.y),p),0.0)*vec2( k.x,k.y);
    p -= vec2(clamp(p.x,-r*k.z,r*k.z),r);    
    return length(p)*sign(p.y);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    float angle = sin(u_time) * 0.2;
    vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy * 2.0 - 1.;
    
    vec4 col = vec4(1.);
    
    for(float i = 0.0; i < 28.0; i++)
    {
        uv = abs(uv);
        uv -= 0.5;
        //uv *= 1.1;
    
        uv *= mat2(
            cos(angle), -sin(angle),
            sin(angle), cos(angle));
   
    
    }
    
    

    //float len = length(normalCoord);
    //float d = smoothstep(sdPentagon(uv, 0.2), 0., 0.4);
    float d = sdPentagon(uv, 0.5);
    col = d > 0. ? vec4(1.000,0.639,0.969,0.) : vec4(0.302,1.000,0.569,0.);
    col*= 1. - exp(-2.0 * d);
    col += exp(2.0 * d);
    
    //fragColor = vec4(length(normalCoord + vec2(0.8, 0.3)),
    //                length(normalCoord + vec2(-0.4, -0.6)),
    //                length(normalCoord + vec2(-0.1, 0.9)),1.0);
    
    fragColor = col;
    
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
