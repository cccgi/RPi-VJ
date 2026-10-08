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
// Complex Number math by julesb
// https://github.com/julesb/glsl-util
// Additions by Johan Karlsson (DonKarlssonSan) & me

#define PI 3.1415926539
#define cx_1 vec2(1.0, 0.0)
#define cx_i vec2(0.0, 1.0)
#define cx_0 vec2(0.0, 0.0)
#define cx_mul(a, b) vec2(a.x*b.x-a.y*b.y, a.x*b.y+a.y*b.x)
#define cx_div(a, b) vec2(((a.x*b.x+a.y*b.y)/(b.x*b.x+b.y*b.y)),((a.y*b.x-a.x*b.y)/(b.x*b.x+b.y*b.y)))
#define cx_modulus(a) length(a)
#define cx_conj(a) vec2(a.x, -a.y)
#define cx_arg(a) atan(a.y, a.x)
#define cx_sin(a) vec2(sin(a.x) * cosh(a.y), cos(a.x) * sinh(a.y))
#define cx_cos(a) vec2(cos(a.x) * cosh(a.y), -sin(a.x) * sinh(a.y))
#define cx_exp(a) exp(a.x) * vec2(cos(a.y), sin(a.y))

vec2 cx_sqrt(vec2 a) {
    float r = length(a);
    float rpart = sqrt(0.5*(r+a.x));
    float ipart = sqrt(0.5*(r-a.x));
    if (a.y < 0.0) ipart = -ipart;
    return vec2(rpart,ipart);
}

vec2 cx_tan(vec2 a) {return cx_div(cx_sin(a), cx_cos(a)); }

vec2 cx_log(vec2 a) {
    float rpart = sqrt((a.x*a.x)+(a.y*a.y));
    float ipart = atan(a.y,a.x);
    if (ipart > PI) ipart=ipart-(2.0*PI);
    return vec2(log(rpart),ipart);
}

vec2 cx_mobius(vec2 a) {
    vec2 c1 = a - vec2(1.0,0.0);
    vec2 c2 = a + vec2(1.0,0.0);
    return cx_div(c1, c2);
}

vec2 cx_z_plus_one_over_z(vec2 a) {
    return a + cx_div(vec2(1.0,0.0), a);
}

vec2 cx_z_squared_plus_c(vec2 z, vec2 c) {
    return cx_mul(z, z) + c;
}

vec2 cx_sin_of_one_over_z(vec2 z) {
    return cx_sin(cx_div(vec2(1.0,0.0), z));
}

vec2 cx_to_polar(vec2 a) {
    float phi = atan(a.y, a.x);
    float r = length(a);
    return vec2(r, phi); 
}
    
// Complex power
// Let z = r(cos θ + i sin θ)
// Then z^n = r^n (cos nθ + i sin nθ)
vec2 cx_pow(vec2 a, float n) {
    float angle = atan(a.y, a.x);
    float r = length(a);
    float real = pow(r, n) * cos(n*angle);
    float im = pow(r, n) * sin(n*angle);
    return vec2(real, im);
}

/////////////////

//  Function from Iñigo Quiles
//  https://www.shadertoy.com/view/MsS3Wc
vec3 hsb2rgb( in vec3 c ){
    vec3 rgb = clamp(abs(mod(c.x*6.0+vec3(0.0,4.0,2.0),
                             6.0)-3.0)-1.0,
                     0.0,
                     1.0 );
    rgb = rgb*rgb*(3.0-2.0*rgb);
    return c.z * mix(vec3(1.0), rgb, c.y);
}
// One step of Newton-Raphson root-finding algorithm
// The roots of the polynomial is set by the roots variable. (For now, just three roots)
vec2 newton_step(vec2 z, mediump vec2[3] roots)
{
    // z += vec2(0.0, 0.5);
    const int nRoots = roots.length();
    
    // Polinomial
    vec2 Pz = cx_1;
    for (int i=0; i<nRoots; i++)
    {
        Pz = cx_mul(Pz, (z - roots[i]));
    }
    
    // Derivative of the polinomial
    vec2 DPz = cx_0;
    for (int i=0; i<nRoots; i++)
    {
        vec2 DPzTerm = cx_1;
        for (int j=0; j<nRoots; j++)
        {
            if (i != j)
            {
                DPzTerm = cx_mul(DPzTerm, (z - roots[j]));
            }
        }
        DPz += DPzTerm;
    }
    
    return z - cx_div(Pz, DPz);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Normalized and centralized pixel coordinates
    float aspect_ratio = vec3(u_resolution, 1.0).y/vec3(u_resolution, 1.0).x;
    vec2 uv = fragCoord/vec3(u_resolution, 1.0).x;
    uv -= vec2(0.5, 0.5 * aspect_ratio);

    const int n_iterations = 20;
    
    const float root_scale = 0.2;
    const float root0Y_in = 0.3;
    const float root0Y_out = 0.7;
    const float dilation_0 = 0.1;
    const float dilation_zoom = 0.01;
    float root0Y_0 = dilation_0 * u_time;
    float root0Y_1 = root0Y_in + dilation_zoom * (u_time - root0Y_in/dilation_0);
    float root0Y_2 = root0Y_out + dilation_0 * (u_time - root0Y_in/dilation_0 - (root0Y_out - root0Y_in)/dilation_zoom);
    float root0Y = 0.0;
    if (root0Y_0 <= root0Y_in)
    {
        root0Y = root0Y_0;
    } else if (root0Y_1 <= root0Y_out)
    {
        root0Y = root0Y_1;
    } else if (root0Y_2 > root0Y_out)
    {
        root0Y = root0Y_2;
    }
    // Roots at the vertices of a triangle centered around the origin
    vec2 roots[3]; 
    roots[0] = root_scale*vec2(-1.0/2.0, root0Y);
    roots[1] = root_scale*vec2(-1.0/2.0, -root0Y); 
    roots[2] = root_scale*vec2(1.0, 0.0);
    const int nRoots = roots.length();
    
    // vec2 fz = 3.0*(1.2-tanh(10.0/u_time))*uv;
    
    // Normalized and centralized mouse coordinates
    // vec4 mouse = iMouse / vec3(u_resolution, 1.0).x;
    // mouse.xy -= vec2(0.5, 0.5 * aspect_ratio);
    // float scale = 1.0/(1.0 + u_time/10.0);
    
    const float zoom_in = root0Y_in*root_scale;
    const float zoom_out = root0Y_out*root_scale;
    const float zoom_dt = 0.1*root_scale;
    const float zoom_mag = 0.9;
    float uv_scale = 1.0 -  zoom_mag*(smoothstep(zoom_in-zoom_dt, zoom_in, roots[0].y) - smoothstep(zoom_out-zoom_dt, zoom_out, roots[0].y));
    vec2 new_uv = uv * uv_scale;
    
    vec2 fz = new_uv; // First argument
    for (int i=0; i<n_iterations; i++)
    {
        fz = newton_step(fz, roots);
    }
    vec2 fzPolar = cx_to_polar(fz);
    vec3 col = hsb2rgb(vec3(fzPolar.y / (2.0*PI), 1.0, tanh(5.0*fzPolar.x)));

    // Display roots
    const float rootRadius = 0.005;
    const vec3 rootColor = vec3(0.9, 0.9, 0.9); 
    for (int i=0; i<nRoots; i++)
    {
        if (distance(new_uv, roots[i]) < rootRadius * uv_scale)
        {
            col = rootColor;
        }
    }

    // Output to screen
    fragColor = vec4(col, 1.0);
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
