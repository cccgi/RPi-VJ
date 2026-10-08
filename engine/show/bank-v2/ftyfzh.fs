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
    "2D Rotation Demo" by @XorDev"
    
    A simple 2D rotation demo for mini tutorial series: getrevue.co/profile/xordev

    This code is only for illustrating, so it's not my cleanest or most clear code.
*/

//Simple 2D rotation matric
mat2 rotate2D(float angle)
{
    return mat2(cos(angle), -sin(angle), sin(angle), cos(angle));
}
//Line distance function
float line(vec2 p, vec2 a, vec2 b)
{
    return length(p-a + (a-b) * clamp(dot(p-a,a-b) / dot(b-a,a-b),0.,1.));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    //Resolution for scaling
    vec2 res = vec3(u_resolution, 1.0).xy;
    //Centered
    vec2 coord = (fragCoord - 0.5*res)/res.y;
    //Rotation angle
    float angle = u_time + iMouse.x/res.x*6.2831;
    
    //Center point (red)
    vec2 A = vec2(0,0);
    //Horizontal rotated point
    vec2 B = A+vec2(cos(angle),-sin(angle))*.4;
    //Vertical rotated point
    vec2 C = A+vec2(sin(angle),cos(angle))*.4;
    //Rotated coordinate space
    vec2 D = mod(coord * rotate2D(angle),.2)-.1;
    
    //Time for animation loop
    float t = mod(u_time,20.);
    //Fade-in guide lines
    float f_g = smoothstep(2.0, 4.0, t);
    //Fade-in "C" point
    float f_c = smoothstep(6.0, 8.0, t);
    //Fade-in "D" points
    float f_d = smoothstep(12.0, 16.0, t);
    //Fade-out
    f_c *= 1.0-f_d;
    f_g *= 1.0-f_d;
    f_d *= smoothstep(20.0, 18.0, t);
    
    //A-B line
    float l = line(coord, A, B)-.02;
    //A-C line (with fade-in)
    l = min(l, line(coord, A, C)-.04*f_c+.02);
    //Rotation ring
    float g = abs(length(coord - A)-.4)-.01;
    //A-B axis lines
    g = min(g, line(coord, A, vec2(B.x,A.y))-.02*f_g+.01);
    g = min(g, line(coord, vec2(B.x,A.y), B)-.02*f_g+.01);
    //A-C axis lines
    g = min(g, line(coord, A, vec2(A.x,C.y))-.02*f_c+.01);
    g = min(g, line(coord, vec2(A.x,C.y), C)-.02*f_c+.01);    
    
    //Lines alpha
    float a_l = clamp(2.-l*res.y, 0., 2.)*.5;
    //Guides alpha
    float a_g = clamp(2.-g*res.y, 0., 2.)*.5;
    //"A" alpha
    float a_a = clamp(2.-(length(coord - A)-.03)*res.y, 0., 2.)*.5;
    //"B" alpha
    float a_b = clamp(2.-(length(coord - B)-.03)*res.y, 0., 2.)*.5;
    //"C" alpha
    float a_c = clamp(2.-(length(coord - C)-.03)*res.y, 0., 2.)*.5 * f_c;
    //"D" alpha
    float a_d = clamp(2.-(length(D)-.03)*res.y, 0., 2.)*.5 * f_d;
    
    //Background color
    vec4 col = vec4(0.1,0.1,0.3,1);
    //Guides color
    col = mix(col, vec4(.2,.2,.4,1), a_g);
    //Lines color
    col = mix(col, vec4(.8,.8,.8,1), a_l);
    //"A" color
    col = mix(col, vec4(.9,.1,.1,1), a_a);
    //"B" color
    col = mix(col, vec4(0,.5,1,1), a_b);
    //"C" color
    col = mix(col, vec4(.4,.8,0,1), a_c);
    //"D" color
    col = mix(col, vec4(1,1,0,1), a_d);

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
