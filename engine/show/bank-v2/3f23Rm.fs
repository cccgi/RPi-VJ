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
    "Constructing SDFs" by @XorDev
    
    Written for an upcoming tutorial on SDFs:
    https://mini.gmshaders.com
    
    We'll start by constructing a triangle.
    The concept here is simple:
    -Create an outline using line segments
    -Calculate sign from edges
    
    This basic process works with a vast range of primitives
*/
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = (fragCoord*2.0-vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;

    //Time loop
    float time = abs(8.0 - mod(u_time+8.0, 16.0));
        
    //Side lengths for demo
    float edge_x = (0.6+0.4*cos(u_time)) * smoothstep(1.0,2.0,time);
    float edge_y = (0.6+0.4*sin(u_time)) * smoothstep(3.0,4.0,time);
    //Hypotenuse edge
    vec2 edge_h = vec2(edge_x,-edge_y) * smoothstep(5.0,6.0,time);
    
    //Centered position coordinates
    vec2 p = uv + 0.5*vec2(edge_x, edge_y);
    //Distance to x-edge
    float d = length(p - clamp(p, vec2(0.0), vec2(edge_x,0)));
    //Distance to y-edge
    d = min(d, length(p - clamp(p, vec2(0.0), vec2(0,edge_y))));
    
    //Hypotenuse edge distance
    //Start at top edge
    p -= vec2(0, edge_y);
    //Hypotenuse gradient
    float h = clamp(dot(p, edge_h) / dot(edge_h, edge_h), 0.0, 1.0);
    //Distance to the hypotenuse edge
    d = min(d, length(p - edge_h*h));

    //Compute distance field sign
    //X and Y edges signs
    float ex = -p.x;
    float ey = -edge_y-p.y;
    //Hypotenuse edge sign
    float eh = edge_y*p.x + edge_x*p.y;
    //Use the sign of the greatest edge
    float sig = sign(max(ex, max(ey, eh)));
    //Animate time after 6 seconds
    sig = mix(1.0, sig, smoothstep(6.0, 7.0, time));
    
    //Isolines
    vec3 col = 0.95+0.05*cos(d*1e2+vec3(0,1,2));
    //Glow and sign color
    col /= 1.0+20.0*d*exp(sig*vec3(3,0,-1));

    //Output to screen
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
