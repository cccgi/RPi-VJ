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

vec2 Rotate(vec2 p, float rads)
{
    mat2 R = mat2(
        cos(rads), sin(rads),
       -sin(rads), cos(rads));
    return R*p;
}

// Returns clockwise vector beginning from up:(0,1). 0=Up, PI/2=Right, PI=Down, 3PI/4=Left
vec2 VectorFromAngle(float rads)
{
    return vec2(sin(rads), cos(rads));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    float aspect = vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y;
    vec2 mouse = iMouse.xy/vec3(u_resolution, 1.0).xy;
    vec2 uv = (fragCoord - 0.5*vec3(u_resolution, 1.0).xy)/vec3(u_resolution, 1.0).y;
	vec3 col = vec3(0);
    
    
    float scale = 1.3 + 0.1*cos(u_time);  // track how much we've scaled uv space - used later to correct line thickness
	
    uv *= scale;			             // zoom out
    uv = Rotate(uv, u_time*0.1);
    uv.y += .5*tan(5./6.*3.1415);        // move shape up to middle
    
    // Fold across y
    uv.x = abs(uv.x);
    
    // Fold diagonally
    {
        vec2 v = VectorFromAngle(5./6.*3.1415);
        float d = dot(uv-vec2(0.5,0), v);
        uv -= v*max(0.,d)*2.;
        //col += smoothstep(0.002,0.001,abs(d));
    	//col.rg = sin(uv * 45.);
    }
    
    // Koch fractal loop
    {
        vec2 v = VectorFromAngle((2./3.) * 3.1415);
        
        uv.x += 1.5/3.; 					// center shape by compensating for scaling in loop
        for (int i = 0; i < 5; i++)
        {
            scale *= 3.;
            uv *= 3.;
            uv.x -=1.5;

            uv.x = abs(uv.x);				// mirror about y axis
            uv.x -= .5; 					// move line out from mid
            uv -= v*min(0.,dot(uv, v))*2.;  // bend line
        }
    }
    
    float dist = length(uv - vec2(clamp(uv.x, -1., 1.), 0));
	uv /= scale;
    
    
    col += smoothstep(1./vec3(u_resolution, 1.0).y, 0., dist/scale); // draw line
    col += texture(iChannel0, uv*2. - u_time*0.1).rgb;    // draw texture
    //col.rg += fract(uv*2. - u_time*0.1);                  // draw uvs
    
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
