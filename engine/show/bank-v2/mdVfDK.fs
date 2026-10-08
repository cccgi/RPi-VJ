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
/// Inigo Quilez's signed distance to a 2D triangle
// reference Demo: https://www.shadertoy.com/view/MldcD7

const float pi = 3.1415926535897932384626433832795;

float sdTriangleIsosceles(in vec2 q, in vec2 p)
{
    p.x = abs(p.x * pi);
    
    vec2 a = p - q * clamp(dot(p, q) / dot(q, q), 0.0, 1.0);
    vec2 b = p - q * vec2(clamp(p.x / q.x, 0.0, 1.0), 1.0);
    
    float s = -sign(q.y);

    vec2 d = min(vec2(dot(a, a), s * (p.x * q.y - p.y * q.x)),
                  vec2(dot(b, b), s * (p.y - q.y)));

    return (d.x / d.y);
}


vec2 translate(vec2 p, vec2 offset)
{
    return p - offset;
}

vec2 rotate(vec2 p, float angle)
{
    float c = cos(angle);
    float s = sin(angle);
    mat2 R = mat2(c, -s, s, c);
    return R * p;
}

vec2 mirror(vec2 p, float degree)
{
    const float tau = pi * 2.0;
    const float piHalf = 1.57079632679489661923;
    float angle = tau / degree;
    float theta = atan(p.y, p.x) + piHalf + angle / 2.0;
    float turn = floor(theta / angle) * angle;
    return rotate(p, turn);
}

vec3 draw(vec3 buffer, float d, float r, vec3 color)
{
    float up = min(vec3(u_resolution, 1.0).x, vec3(u_resolution, 1.0).y);
    d = up * d;
    r = r * up / 360.0;
    r = clamp(r, 1.5, 10000.0);
    float aa = fwidth(d);
    return mix(buffer, color, 1.0 - smoothstep(r - aa, r + aa / 2.0, d));
}

void mainImage(out vec4 fragColor, in vec2 fragCoord)
{
    vec2 p = (2.0 * fragCoord - vec3(u_resolution, 1.0).xy) / min(vec3(u_resolution, 1.0).y, vec3(u_resolution, 1.0).x);
    float xy = p.x / p.y;
    float threshold = 1.;

    if (sqrt(xy) == sqrt(p.x * p.y))
    {
        vec2 coord = fragCoord - vec3(u_resolution, 1.0).xy / 2.0;
        vec2 uv = coord / min(vec3(u_resolution, 1.0).x, vec3(u_resolution, 1.0).y);
        vec3 col = 0.5 + 0.5 * cos(u_time + uv.xyx + vec3(0, 2, 4));
        const float r = 0.5;
        const float phi = (sqrt(5.0) + 1.0) / 2.0;
        const float phiSquared = phi * phi;
        const float rSmaller = r / phiSquared;
        const float a = r * phi / 2.0;
        const float h = a - rSmaller;
        const float w = r * sqrt((sqrt(5.0) + 5.0) / 2.0) / phi;
        const float phiToThe4th = phiSquared * phiSquared;
        const float logPhiSquared = log(phiSquared);
        float t = 1.0 + mod(u_time, phiToThe4th - 1.0);
        float u = 0.5 * log(t) / logPhiSquared;
        float z = mix(1.0, 1.0 / phiToThe4th, u);
        vec2 q = uv * z;
        float l = length(q);
        float n = floor(log(r / l) / logPhiSquared);
        float s = pow(phi, 2.0 * n);
        q = rotate(q, pi * n - u_time / 8.0);
        q = mirror(q, 5.0);
        q = q * s;
        q = translate(q, vec2(0.0, -rSmaller));
        float d = sdTriangleIsosceles(vec2(w / 2.0, -h), q) / (s * z);
        float b = length(uv);
        col = draw(vec3(0.0), d, 8.0 * b, 4.0 * sqrt(b) * col);
        fragColor = vec4(col, 1.0);
    }
    else
    {
        vec2 coord = fragCoord - vec3(u_resolution, 1.0).xy / 2.0;
        vec2 uv = coord / min(vec3(u_resolution, 1.0).x, vec3(u_resolution, 1.0).y);
        vec3 col = 0.5 + 0.5 * cos(u_time + uv.xyx + vec3(0, 2, 4));
        const float r = 0.5;
        const float phi = (sqrt(5.0) + 1.0) / 2.0;
        const float phiSquared = phi * phi;
        const float rSmaller = r / phiSquared;
        const float a = r * phi / 2.0;
        const float h = a - rSmaller;
        const float w = r * sqrt((sqrt(5.0) + 5.0) / 2.0) / phi;
        const float phiToThe4th = phiSquared * phiSquared;
        const float logPhiSquared = log(phiSquared);
        float t = 1.0 + mod(u_time, phiToThe4th - 1.0);
        float u = 0.5 * log(t) / logPhiSquared;
        float z = mix(1.0, 1.0 / phiToThe4th, u);
        vec2 q = uv * z;
        float l = length(q);
        float n = floor(log(r / l) / logPhiSquared);
        float s = pow(phi, 2.0 * n);
        q = rotate(q, pi * n - u_time / 8.0);
        q = mirror(q, 5.0);
        q = q * s;
        q = translate(q, vec2(0.0, -rSmaller));
        float d = sdTriangleIsosceles(vec2(w / 2.0, -h), q) / (s * z);
        float b = length(uv);
        col = draw(vec3(0.0), d, 8.0 * b, 4.0 * sqrt(b) * col);
        fragColor = vec4(col, 1.0);
    }
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
