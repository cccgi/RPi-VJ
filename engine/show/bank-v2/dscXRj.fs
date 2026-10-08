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
float INVSPEED = 100.0;

vec3 hsl2rgb( in vec3 c )
{
    vec3 rgb = clamp( abs(mod(c.x*6.0+vec3(0.0,4.0,2.0),6.0)-3.0)-1.0, 0.0, 1.0 );

    return c.z + c.y * (rgb-0.5)*(1.0-abs(2.0*c.z-1.0));
}

vec3 getColor(float t) {
    vec3 color;
    if (t < 0.5) {
        color = mix(vec3(0, 0, 0), hsl2rgb(vec3((u_time*0.07/INVSPEED),1.0,0.5)), t * 2.0);
    } else {
        color = mix(hsl2rgb(vec3((u_time*0.07/INVSPEED)+0.125,1.0,0.5)), vec3(1.0, 1.0, 1.0), (t - 0.5) * 2.0);
    }
    return color;
}

vec2 mandelbrot(vec2 z, vec2 c)
{
    return vec2(z.x*z.x - z.y*z.y + c.x, 2.0*z.x*z.y + c.y);
}

vec2 mandelbrotcubed(vec2 z, vec2 c)
{
    return vec2(z.x*z.x*z.x - 3.0*z.x*z.y*z.y - c.x, 3.0*z.y*z.x*z.x - z.y*z.y*z.y + c.y);
}

vec2 burningship(vec2 z, vec2 c)
{
    return vec2(z.x*z.x - z.y*z.y + c.x, 2.0*abs(z.x*z.y) - c.y);
}

vec2 rotateVector(vec2 v, float t)
{
    return vec2(v.x*cos(t)-v.y*sin(t),v.x*sin(t)+v.y*cos(t));
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 camPos = vec2(0,0);
    float zoom = 1.0;


    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = fragCoord/vec3(u_resolution, 1.0).xy;
    float aspect = vec3(u_resolution, 1.0).x/vec3(u_resolution, 1.0).y;
    uv -= vec2(0.5,0.5);
    uv /= vec2(0.25*zoom,0.25*zoom*aspect);
    uv += camPos;
    uv = rotateVector(uv,0.0);
    

    // Time varying pixel color
    //vec3 col = 0.5 + 0.5*cos(u_time+uv.xyx+vec3(0,2,4));
    
    int iterations = 200;
    int currentIterations = 0;
    vec3 col = vec3(0.0,0.0,0.0);
    //float speed = 4.0*pow(cos(u_time/4.0),2.0)+4.0;
    //float inverseSpeed = 100.0;
    
    vec2 c = 0.7885*vec2(0.8*cos(u_time/INVSPEED)-0.17,sin(u_time/INVSPEED));
    //vec2 c = 0.2885*vec2(cos(u_time/4.0)+0.1*cos(u_time/0.40),sin(u_time/4.0)+0.1*sin(u_time/0.40));
    vec2 z = uv.xy;
    //vec2 z = vec2(0.0,0.0);
    //vec2 c = uv.xy;
    
    for (int i = 1; i < iterations; i++)
    {
        z = mandelbrot(z,c);
        if (length(z) > 2.0)
        {
            col = vec3(1.0,1.0,1.0)/float(i);
            currentIterations = i;
            break;
        }
    }
    
    if (currentIterations == 0 && length(c) <= 2.0)
        col = vec3(0.0,0.0,0.0);
    else
        col = getColor(pow(float(currentIterations)/float(iterations),0.5));
        //col = hsl2rgb(vec3(0.025*float(currentIterations),1.0,0.5));

    // Output to screen
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
