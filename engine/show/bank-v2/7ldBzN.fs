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
/*
    "Hurricane Twist" by @XorDev

    A little hurricane shader, written for my mini tutorial series:
    https://www.getrevue.co/profile/xordev

    This could be a fun shader to golf!
*/

#define PI 3.1415926536

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Resolution for scaling/centering
    vec2 res = vec3(u_resolution, 1.0).xy;
    //Centered pixel coordinates
    vec2 coord = fragCoord - res * 0.5;
    
    //Scaled texture coordinates (-1 to +1 and aspect correct)
    vec2 u = coord / res.y;
    //Radius
    float r = length(u);
    //Zoom
    float z = log(r) - u_time*0.2;
    //Polar angle
    float a = atan(u.y, u.x) / PI;
    //Mipmap fix
    fwidth(a)>1.0 ? a = 0.0 : a;
    
    //Get new, polar-coordinates for texture
    vec2 uv = vec2(z, a-z*0.5-u_time*0.05);
    //Sample texture
    vec4 tex = texture(iChannel0, uv);
    //Give it a blue color
    fragColor = pow(1.-tex*r, vec4(3,2,1,1));
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
