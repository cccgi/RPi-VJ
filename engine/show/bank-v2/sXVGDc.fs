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
// angle: radian value
mat2 rotate2D( float angle )
{
    float s = sin(angle), c = cos(angle);
    return mat2(c, -s, s, c);
}

// Modify This Macro ( 0~4 )
#define METHOD 3


#define PI 3.1415926535897932384626

void main1(out vec4 fragColor, in vec2 fragCoord ) {
   
    // Basic Variables
    vec2 uv = (2. * fragCoord - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    float time = u_time;

    /** Change VALUE of these variables **/

    // Instead of Define Macro I'll use variable for each main function
    const bool SHAPE_SQUARE = true;

    /** Main Logic **/
    float angle = (atan(uv.x, uv.y) + PI) / (2. * PI);
    
    float dist = length(uv);
    if (SHAPE_SQUARE) {
        vec2 p1 = uv, p2 = p1 * p1, p4 = p2 * p2, p8 = p4 *p4;
        dist = pow(p8.x + p8.y, 1. / 8.);
    }

    vec2 polarUV = vec2(angle, .1 / dist + time * .1);

    fragColor = texture(iChannel0, polarUV) * dist;
}

void main2(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (2. * fragCoord.xy - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    uv *= rotate2D(-u_time) * clamp(sin(u_time), .3, 1.);
    vec4 texColor = texture(iChannel1, uv);

    fragColor = texColor;
}


void main3(out vec4 fragColor, in vec2 fragCoord) {
   
    // Basic Variables
    vec2 uv = (2. * fragCoord.xy - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    float time = u_time * .25;
    
    /** Change VALUE of these variables **/

    // Instead of Define Macro I'll use variable for each main function
    const float SIZE = 4.;
    
    /** Main Logic **/
    float a = atan(uv.y, uv.x) + time;
    float r = length(uv);
    float s = .5 + .5 * sin(a * SIZE);
    float t = .05 + .65 * pow(s, .35) ;
    t += .15 * pow(.5 + .5 * cos(a * SIZE * 2.), .5);
    float h = r / t;

    vec4 color = vec4(.86, .33, .13, 1.);
    if (h < 1.) {
        color.rgb = mix(color.rgb, (.5 + .5 * h) * vec3(.3, 1., .1), max(t, .1));
    }

    fragColor = color;
}

void main4(out vec4 fragColor, in vec2 fragCoord) {
   
    // Basic Variables
    vec2 uv = (2. * fragCoord.xy - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    float time = u_time * .25;

    /** Change VALUE of these variables **/
    
    // Background Color
    vec3 col = mix(vec3(.2, .2, .5), vec3(.46, .53, .93), (uv.y * .5 + .5));
    vec3 sunCol = vec3(.8, .9, .05);
    const float sunR = .5;

    // Instead of Define Macro I'll use variable for each main function
    
    
    /** Main Logic **/
    float a = atan(uv.y, uv.x) + time;
    float r = length(uv);
    float s = (cos(a * 16.) * .5 + .5) * .2;
    float b = r * .3;
    float t = smoothstep(sunR + b, sunR - b, r - s);


    // Sun Color
    col = mix(col, sunCol, t);

    fragColor = vec4(col, 1.);
}


void mainImage( out vec4 fragColor, in vec2 fragCoord )
{

#if METHOD == 0
    // Training 1
    main1(fragColor, fragCoord);

#elif METHOD == 1
    // Training 2
    main2(fragColor, fragCoord);
    
#elif METHOD == 2
    // Training 3
    main3(fragColor, fragCoord);
    
#elif METHOD == 3
    // Training 4
    main4(fragColor, fragCoord);

#else
    // Default
    vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy; 
    vec3 col = vec3(.5 + .5 * cos(u_time + uv.xyx +  vec3(2., 4., 6.)));
    fragColor = vec4(col, 1.);

#endif
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
