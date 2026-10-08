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
// Golf of "Quadicube" by panna_pudi
// https://shadertoy.com/view/NsByWV

//<512 Chars playlist: shadertoy.com/playlist/N3SyzR

//This version is 608 characters (57%) shorter!
//FabriceNeyret2 saved 12 chars
//coyote saved 7 chars

//Square SDF
#define S length(max(s, 0.)) + min(0., max(s.x, s.y))

void mainImage( out vec4 O, vec2 I)
{
    vec2 m = vec3(u_resolution, 1.0).xy, //Shortened resolution
         p = (I+I - m) / m.y, //Signed uv coordinates (-1 to +1)
         u,q,s;
    
    float i = 1.,z = .2,b = i; //Zoom and border
    
    //Iterate through quadtree
    for (m = (2. * iMouse.xy - m) / m.y; i++ < 15. && clamp(q,m-1./z,m) == q || abs(S)*z < 1.4; s = abs(u)-.35)
    
        z += z, //zoom
        u = floor(p * z) / z, //cell coordinates
        q = abs( (p - u) * z - .5 ), // distance to center
        b = min( b, abs( max(q.x, q.y) - .5) / z ), //border
        q = u, //Back to cell coordinates for mouse
        u.x += .1, //Offset
        O = cos(u_time * 1.26 + vec4(0,33,11,0)), //Rotating matrix
        u += .16*O.wz, //Revolve
        u *= mat2(O); //Rotate
        
    s = abs(p) - vec2(1.252, .944); //Border square
    O = min(b/fwidth(p).xxxx*.7+.1,1.) + ceil(b=S); //Aprroximate AA and white background
    O *= ceil(b*b-1e-4); //Black border
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
