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
#define PI 3.1415
vec3 intersect(in vec2 p, in vec2 d, in vec2 wp1, in vec2 wp2) {
    vec2 nd = normalize(d);
    float m1 = nd.y/nd.x;
    float b1 = p.y - p.x*m1;
    
    float m2 = (wp2.y - wp1.y)/(wp2.x-wp1.x);
    float b2 = wp1.y - wp1.x*m2;
    
    float x = (b2-b1)/(m1-m2);
    float y = m1*x+b1;
    
    float minx = min(wp1.x, wp2.x);
    float maxx = max(wp1.x, wp2.x);
    
    vec2 relintpoint = vec2(x, y) - p;

    vec2 v = wp2 - wp1;
    vec2 w = vec2(x,y) - wp1;
    
    float t = dot(w,v)/dot(v,v);
    
    return vec3(x,y,t);
}


void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = fragCoord/vec3(u_resolution, 1.0).xy;
    uv -= vec2(0.5,0.5);
    uv.x *= vec3(u_resolution, 1.0).x/vec3(u_resolution, 1.0).y;
    
    vec2 campos = vec2(0,0);
    vec2 raydir = vec2(sin(uv.x),cos(uv.x));
    
    vec3 where = intersect(campos,raydir,vec2(sin(u_time)*1.5,cos(u_time)*1.5+4.0),vec2(sin(u_time+PI)*1.5,cos(u_time+PI)*1.5+4.0));
    vec4 col;
    if (where.z >= 0.0 && where.z <= 1.0 && abs(uv.y)<1.0/distance(campos,where.xy)) {
        float wallheight = 1.0 / distance(campos, where.xy);
        wallheight = (uv.y / wallheight) * 0.5 + 0.5;
        
        col = texture(iChannel0,vec2(where.z,wallheight));
    } else {col = vec4(0.0);}

    // Output to screen
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
