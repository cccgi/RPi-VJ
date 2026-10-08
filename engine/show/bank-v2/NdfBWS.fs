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
// Copyright Luka Gorgadze, 2022 - https://lukegorgadze.github.io/CV/
// I am the sole copyright owner of this Work.
// You cannot host, display, distribute or share this Work in any form,
// including physical and digital. You cannot use this Work in any
// commercial or non-commercial product, website or project. You cannot
// sell this Work and you cannot mint an NFTs of it.
// I share this Work for educational purposes, and you can link to it,
// through an URL, proper attribution and unmodified screenshot, as part
// of your educational material.
const float Max_Iter=500.;
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{

     vec2 uv= fragCoord/vec3(u_resolution, 1.0).xy;
    uv-=.5;
    uv.x*=vec3(u_resolution, 1.0).x/vec3(u_resolution, 1.0).y;
    uv*=2.8;
    vec3 col=vec3(0.,0.,0.);
    
    //JULIA GOES BRR
    vec2 c=vec2(cos(u_time*1.),sin(u_time*1.));
    vec2 z=uv;
    float smoothcolor=0.;
    for(float i=0.;i<28.0;i++)
    {
        //z=vec2(z.x*z.x-z.y*z.y,2.*z.x*z.y)+c/1.2;
        z=vec2(pow(z.x,3.)-3.*z.x*pow(z.y,2.),3.*pow(z.x,2.)*z.y-pow(z.y,3.))+c/1.2;// cube
        z=vec2(pow(z.x,4.)-6.*pow(z.x,2.)*pow(z.y,2.)+pow(z.y,4.),4.*pow(z.x,3.)*z.y-4.*z.x*pow(z.y,3.))+c;
        z=vec2(pow(z.x,5.)-10.*pow(z.x,3.)*pow(z.y,2.)+5.*z.x*pow(z.y,4.),5.*pow(z.x,4.)*z.y-10.*pow(z.x,2.)*pow(z.y,3.)+pow(z.y,5.))+c/.99;
        smoothcolor+= fract(exp(-length(z) * 1.5)) * 0.15;
        //smoothcolor+=exp(-length(z)*1.5)*.12;
        if(dot(z,z)>20.)
        break;
    }
    
    //END OF JULIA
    
    col = vec3(0.3804, 0.0353, 0.0353) + smoothcolor*vec3(.9 * clamp(cos(5.+u_time * 1.),-1.,-0.8) ,.3 * clamp(cos(.1+ u_time * 5.),-1.,-0.2),.9922 * clamp(sin(.2+ u_time * 4.),-1.,0.5))*15.;
    float r=.4+.4*(sin(smoothcolor*.1+u_time*.63));
    float g=r*r*.7;
    float b=r*g*10.;
    
    fragColor=vec4(1.-(col+vec3(1.0, 1.0, 1.0)),1.);
    
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
