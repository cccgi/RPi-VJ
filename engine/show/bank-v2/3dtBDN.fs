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
#define hypot(x,y) sqrt(x*x+y*y)
#define max(a,b,c,d) max(max(a,b),max(c,d))
//Recreated @aemkey's "Tixy Land" in ShaderToy using shader syntax.
//Play around with the original here: https://tixy.land

#define tixy(t,i,x,y) cos((x*x-x*4.-y*8.+y*y)*.1+8.-t)

// .01/cos(i*(i+y)+t/3.)
// tan(x/max(x-8.,y-8.,7.-x,7.-y)-t*4.)

/*
	While some codes (like this one) will work on tixy.land too,
	there are plenty of syntax and function name differences.
	You can't use floats and ints interchangably for example.
	Still thought it was fun to try though!
*/





//You can fiddle with dot count here:
#define count 16.

//Render with the result:
void mainImage( out vec4 color, in vec2 coord )
{
    float s = 1.2/vec3(u_resolution, 1.0).y;
    vec2 u = (coord-.5*vec3(u_resolution, 1.0).xy)*s;
    u.y = -u.y;
    
    float square = step(abs(u.x),.5)*step(abs(u.y),.5);
    vec2 cell = floor((u+.5)*count);
    float index = cell.x+cell.y*count;
    float tixel = float(tixy(u_time,index,cell.x,cell.y));
    
    float dist = length(fract(u*count)-.5)*count/.5;
    float radius = count*min(abs(tixel),1.);
    
    vec3 col = tixel<0. ? vec3(255,34,68)/255. : vec3(1);
    col *= clamp((radius-dist)/s/count/count/2.,0.,1.)*square;

    color = vec4(col,1);
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
