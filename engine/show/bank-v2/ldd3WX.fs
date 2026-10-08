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
#define PI 3.14159
const int iter = 50;
const vec2 scale = vec2(2., 1.333);
const vec2 offset = vec2(0, 0);

vec2 lissajous(){
    float a = 0.5;
    float b = 0.6;
    float speed = 0.2;
    return vec2(cos(u_time*speed), cos(speed*(u_time*a + b)));
}

vec4 palette(float index){
  return vec4(
      0.5+0.5*sin(index*PI/0.5),
      0.5+0.5*sin((index*u_time)*PI/0.5),
      0.5+0.5*sin((index* 14.2)*PI/0.5),
      1.
  );
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = 2. * fragCoord.xy / vec3(u_resolution, 1.0).xy - 1.;
    vec2 z = vec2(0);
    vec2 c;
    vec2 sc = scale;
    if(iMouse.z>0.){
        c = 2. * iMouse.xy / vec3(u_resolution, 1.0).xy - 1.;
        sc *= length(c);
    }
    else{
        c = lissajous();
    }
	int j = 0; //dummy for accessing i outside of loop
    
    z = (uv + offset) * sc;

    for(int i=0; i<25; i++){
        vec2 xy = vec2(
            z.x * z.x - z.y * z.y,
            z.y * z.x + z.x * z.y);
        if(dot(xy,xy) > float(iter)) break;
        z = xy + c;
        j = i; //need to share i outside the for loop
    }
    
	float val = float(j) / float(iter);
    fragColor = palette(val);
    fragColor.xyz *= 1. - length(z*uv*c);
    
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
