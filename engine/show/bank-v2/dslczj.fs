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
//This is my first try at Shader Coding. 
//Took inspiratin from this youtube video 🙏 https://www.youtube.com/watch?v=f4s1h2YETNY&t=740s



vec3 palette( float t ){
    //vector coordinates taken from http://dev.thi.ng/gradients/
    //[[0.988 0.328 0.718] [0.659 0.438 0.328] [0.388 0.388 0.296] [2.538 2.478 0.168]]

    vec3 a = vec3(0.5, 0.5, 0.5);
    vec3 b = vec3(0.5, 0.5, 0.5);
    vec3 c = vec3(1.0, 1.0, 1.0);
    vec3 d = vec3(0.263, 0.416, 0.558);
    
    return a +b*cos( 6.28318*(c*t*d));
}


void mainImage (out vec4 fragColor, in vec2 fragCoord ) {
    //vec3(u_resolution, 1.0).xy <=> vec2(iResoluton.x, vec3(u_resolution, 1.0).y)
    //uv = uv - 0.5;
    //uv = uv*2;  
    vec2 uv = (fragCoord * 2.0 - vec3(u_resolution, 1.0).xy) /vec3(u_resolution, 1.0).y;
    //variable for local gradient repetitions
    vec2 uv0 = uv;
    vec3 finalColor = vec3(0.0);
    
    for (float i = 0.0; i < 2.0; i++){
        uv = fract(uv * 2.0) - 0.5;

    //Changing aspect ratios
    //uv.x *= vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y;

        float f = length(uv);

        vec3 col = palette(length(uv0) + u_time);

        //Moving Pattern
        f = sin(f*-2.0 + u_time*0.5)/5.0;
        f = abs(f);

        f = 0.02 / f;

        finalColor += col * f;

        //f = smoothstep(0.0, 0.06, f);
    }
    
    fragColor = vec4 (finalColor, 1.0);
    
    
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
