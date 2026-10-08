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
float sdCircle(vec2 p, float r)
{
    return length(p) - r;
}

float sdBox(vec2 p, vec2 b)
{
    vec2 d = abs(p) - b;
    return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0);
}

float sdEquilateralTriangle( in vec2 p, in float r )
{
    const float k = sqrt(3.0);
    p.x = abs(p.x) - r;
    p.y = p.y + r/k;
    if( p.x+k*p.y>0.0 ) p = vec2(p.x-k*p.y,-k*p.x-p.y)/2.0;
    p.x -= clamp( p.x, -2.0*r, 0.0 );
    return -length(p)*sign(p.y);
}

float opOnion(float d, float r)
{

    return abs(d + r * 0.5) - r * 0.5;
}

float opUnion(float d1, float d2)
{
    return min(d1, d2);
}

float opSubtraction(float d1, float d2)
{
    return max(-d1, d2);
}

float opIntersection(float d1, float d2)
{
    return max(d1, d2);
}

float opXor(float d1, float d2)
{
    return max(min(d1, d2), -max(d1, d2));
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv0 = fragCoord / vec3(u_resolution, 1.0).xy;
    vec2 uv = uv0 * 2.0 - 1.0;
    uv.x *= (vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y);
    
    vec2 p0 = vec2(-0.8, 0.5);  // top-left
    vec2 p1 = vec2(0.8, 0.5);   // top-right
    vec2 p2 = vec2(-0.8, -0.5); // bottom-left
    vec2 p3 = vec2(0.8, -0.5);  // bottom-right
    
    float radius = 0.5;           // radius of 0.5
    vec2 size = vec2(0.75, 0.25); // width of 0.5 and height of 0.5
    
    float c0 = sdCircle(uv - p0, radius);
    float b0 = sdBox(uv - p0, size);
    float c1 = sdCircle(uv - p1, radius);
    float b1 = sdBox(uv - p1, size);
    float c2 = sdCircle(uv - p2, radius);
    float b2 = sdBox(uv - p2, size);
    float c3 = sdCircle(uv - p3, radius);
    float b3 = sdBox(uv - p3, size);
    
    float c4 = sdEquilateralTriangle(uv, 0.25);
    float b4 = sdCircle(uv, 0.1);
    float d4 = opSubtraction(b4, c4);
    
    // Combine the box with the circle
    float d0 = opUnion(c0, b0);
    
    // Remove the circle from the box
    float d1 = opSubtraction(c1, b1);
    
    // Combine the smallest overlapping areas of the circle and the box
    float d2 = opIntersection(c2, b2);
    
    // Positive where circle & box overlap, otherwise negative within circle or box
    float d3 = opXor(c3, b3);
    
    float s0 = 1.0 - step(0.0, d0);
    float s1 = 1.0 - step(0.0, d1);
    float s2 = 1.0 - step(0.0, d2);
    float s3 = 1.0 - step(0.0, d3);
    float s4 = 1.0 - step(0.0, d4);
    
    vec3 r = vec3(1.0, 0.0, 0.0);
    vec3 g = vec3(0.0, 1.0, 0.0);
    vec3 b = vec3(0.0, 0.0, 1.0);
    vec3 gr1 = vec3(mod(uv0, 0.1) * 10.0, 0.0);
    vec3 gr2 = vec3(mod(uv0, 0.1) * 10.0, 1.0);
    //vec3 gradient = vec3(uv0, sin(u_time * 4.0) * 0.5 + 0.5);
    vec3 rgb = r * s0 + g * s1 + b * s2 + gr1 * s3 + gr2 * s4;
    fragColor = vec4(rgb, 1.0);
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
