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
uniform sampler2D uPrevFrame;
const float pi = 3.14156;
const float phi = 1.61803;
const float e = 2.71828;


//change to adjust
const float speed = 10.;
const float modAmplitude = 0.35;
const float divideBy = 3.;
const bool centerMode = true;
const bool normalizeMode = true;
const bool warpedMode = true;
const bool clampWarp = false;
const float SlewRate = 0.85;
const int iterations = 4;

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    float warp = 1.;
    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = fragCoord/vec3(u_resolution, 1.0).xy;
    
    if(centerMode) uv -= 0.5;
    if(normalizeMode) uv.x *= vec3(u_resolution, 1.0).x/vec3(u_resolution, 1.0).y;
    vec2 uv2 = uv;
    vec2 res = vec3(u_resolution, 1.0).xy;
    vec3 col = 0.5 + 0.5*cos(u_time+uv.xyx+vec3(0,2,4));
    


    col = 0.5 + 0.5 * cos( u_time / speed + uv2.xyx + vec3(0,2,4));
    vec2 offVec = vec2( sin(uv2.x * 2. * pi) * modAmplitude,  
                        cos(uv2.y * 2. * pi) * modAmplitude );

    for(int i=0; i < iterations; i++){
        offVec = vec2(cos(sin(uv2.x * phi * uv2.y * e)   + u_time * (1. + float(i) / 12.) * 0.5)*modAmplitude,  
                      sin(cos(uv2.x * phi * uv2.y * e)   + u_time * (1. + float(i) / 12.) * 0.5)*modAmplitude );
        if(warpedMode) warp = tan(length(uv2)+ u_time/(speed *13.));
        if((clampWarp) && (warp > 10.)) warp = 10. + sin(warp / u_time*(phi * 10. * speed));
        uv2 = fract(uv2 * divideBy* warp);
        if(uv2.x > (1. - offVec.x ) ||uv2.x < (0.0 + offVec.x )
        || uv2.y > (1. - offVec.y ) ||uv2.y < (0.0 + offVec.y )  )break;

        col = 0.5 + 0.5*cos(u_time/speed + vec3(i )+ uv2.xyx + vec3(0,2,4));
    
    }
    // Output to screen
    vec3 colLast = texture(uPrevFrame, fragCoord).rgb;
    vec3 delta = col - colLast;
    col = colLast + clamp(delta, -SlewRate, SlewRate);
    //col *= 1.1;
    fragColor = vec4(col,1.0);
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
