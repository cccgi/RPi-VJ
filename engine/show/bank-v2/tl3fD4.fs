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
// Created by Arthur Jacquin - 2021
// https://www.linkedin.com/in/arthur-jacquin-631921153/
// License Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License.
//Inspired by https://www.youtube.com/watch?v=il_Qg9AqQkE&t=369s

#define PI 3.14159265359

vec2 GetDirection(float angle)
{
    return vec2(sin(angle), cos(angle));
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 st = (fragCoord.xy - .5 * vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y; //UVs centré

    //Pour les symetries
    st *= 1.25;
    st.x = abs(st.x);

    float angle = 5.0 / 6.0 * PI;
    st.y += tan(angle) * 0.5; //Centrage en hauteur
    vec2 n = GetDirection(angle); //Direction de l'axe de symetrie
    float axeSym = dot(st - vec2(.5, 0), n); //Axe de symetrie
    st -= n * max(0., axeSym) * 2.; //On applique la symétrie

    angle = 2.0 / 3.0 * PI;
    n = GetDirection(angle); //Direction de l'axe de symetrie
    
    const int it = 4; //Nombre d'iterations
    float scale = 1.; //scale appliqué pour décompression plus tard
    
    st.x += .5; //Centrage
    for(int i = 0; i < it; i++)
    {
        scale *= 3.;
        st *= 3.;
        st.x -= 1.5;

        st.x = abs(st.x); //Symetrie en X
        st.x -= 0.5; //Aggrandissement de la ligne
        
        float axe = dot(st, n); //Axe de symetrie
        st -= n * min(0., axe) * 2.; //On applique la symétrie

    }

    //line shape 1 pixel d'épaisseur
    vec3 col = 0.5 + 0.5*cos(u_time - st.yyy + vec3(6,4,2));
    float d = length(st - vec2(clamp(st.x, -1., 1.), 0));
    col.rg += st / scale;
    
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
