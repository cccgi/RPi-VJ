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
    "GM Shaders: Shading" by @XorDev
    A simple method for bumping in 2D
    
    Written for a mini tutorial on shading in games:
    https://xordev.substack.com/p/gm-shaders-mini-shading?sd=pf

*/
void mainImage( out vec4 fragColor, in vec2 fragCoord)
{
    //Compute light direction vector
    vec2 light_dir = vec2(cos(u_time),sin(u_time));
    //Set light pitch angle
    float light_pitch = 0.5;
    
    //Point light version:
    /*
    light_dir = iMouse.xy-fragCoord;
    float light_len = length(light_dir);
    light_dir /= light_len;
    light_pitch = atan(vec3(u_resolution, 1.0).y*0.2,light_len);
    */
    
    //Compute texel coordinates:
    //https://xordev.substack.com/p/gm-shaders-mini-texels-and-pixels-1308242
    vec2 texel = 1.0 / vec3(u_resolution, 1.0).yy;
    
    //Convert pixel to texture coordinates
    vec2 uv0 = fragCoord * texel;
    //Compute uvs one texel toward the light
    vec2 uv1 = uv0 + light_dir * texel;
    
    //Get the pixel height
    float height0 = texture(iChannel0,uv0).r;
    //Get the offset pixel height
    float height1 = texture(iChannel0,uv1).r;
    
    //Depth of heightmap
    float depth = 0.1;
    //Compute surface normal along light_dir axis
    float normal = atan(texel.y, (height0-height1) * depth);
    
    //Compute light using Lambert's cosine law
    float lam = max(cos(normal - light_pitch), 0.0);
    //Output the results
    fragColor = vec4(lam, lam, lam, 1.0);
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
