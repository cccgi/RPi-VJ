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
    "Noise Example" by @XorDev

    Top:    | Hash   | Value   | Perlin  |
    Bottom: | Worley | Voronoi | Fractal |

    This was written as a guide for my newest tutorial on noise functions:
    
    https://mini.gmshaders.com/p/gm-shaders-mini-noise-1437243

*/

//Typical pseudo-random hash (white noise)
float hash1(vec2 p)
{
    //Generate a pseudo random number from 'p'.
    return fract(sin(p.x*0.129898 + p.y*0.78233) * 43758.5453);
}
//vec2 version of the hash function
vec2 hash2(vec2 p)
{
    //Generate a pseudo random vec2 from 'p'
    return fract(sin(p * mat2(0.129898, 0.81314, 0.78233,  0.15926)) * 43758.5453);
}
//vec2 unit-vector version of the hash function
vec2 hash2_norm(vec2 p)
{
    //Returns a random normalized direction vector
    return normalize(hash2(p) - 0.5);
}
//Standard value noise
float value_noise(vec2 p)
{
    //Cell (whole number) coordinates
    vec2 cell = floor(p);
    //Sub-cell (fractional) coordinates
    vec2 sub = p - cell;
    //Cubic interpolation (use sub for linear interpolation)
    vec2 cube = sub*sub*(3.-2.*sub);
    //Offset vector
    const vec2 off = vec2(0,1); 

    //Sample cell corners and interpolate between them.
    return mix( mix(hash1(cell+off.xx), hash1(cell+off.yx), cube.x),
                mix(hash1(cell+off.xy), hash1(cell+off.yy), cube.x), cube.y);
}
//Standard Perlin noise
float perlin_noise(vec2 p)
{
    //Cell (whole number) coordinates
    vec2 cell = floor(p);
    //Sub-cell (fractional) coordinates
    vec2 sub = p - cell; 
    //Quintic interpolation
    vec2 quint = sub*sub*sub*(10.0 + sub*(-15.0 + 6.0*sub));
    //Offset vector
    const vec2 off = vec2(0,1); 
    
    //Compute corner hashes and gradients
    float grad_corner00 = dot(hash2_norm(cell+off.xx), off.xx-sub);
    float grad_corner10 = dot(hash2_norm(cell+off.yx), off.yx-sub);
    float grad_corner01 = dot(hash2_norm(cell+off.xy), off.xy-sub);
    float grad_corner11 = dot(hash2_norm(cell+off.yy), off.yy-sub);

    //Interpolate between the gradient values them and map to 0 - 1 range
    return mix(mix(grad_corner00, grad_corner10, quint.x),
               mix(grad_corner01, grad_corner11, quint.x), quint.y) * 0.7 + 0.5;
}
//Standard Worley noise function
float worley_noise(vec2 p)
{
    //Cell (whole number) coordinates
    vec2 cell = floor(p);
    //Initialize distance at a high-number
    float dist = 9.0;
    
    //Iterate through [3,3] neighbor cells
    for(int x = -1; x<=1; x++)
    for(int y = -1; y<=1; y++)
    {
        //Get sample cell coordinates
        vec2 sample_cell = cell + vec2(x,y);
        //Compute difference from pixel to worley cell
        vec2 worley_dif = hash2(sample_cell) + sample_cell - p;
        //Save the nearest distance
        dist = min(dist, length(worley_dif));
    }
    return dist;
}
//Standard Voronoi noise function
float voronoi_noise(vec2 p)
{
    //Cell (whole number) coordinates
    vec2 cell = floor(p);
    //Initialize distance at a high-number
    float dist = 9.0;
    //Store the nearest voronoi cell
    vec2 voronoi_cell = cell;
    
    //Iterate through [3,3] neighbor cells
    for(int x = -1; x<=1; x++)
    for(int y = -1; y<=1; y++)
    {
        //Get sample cell coordintaes
        vec2 sample_cell = cell+vec2(x,y);
        //Compute difference from pixel to worley cell
        vec2 worley_dif = hash2(sample_cell) + sample_cell - p;
        //Compute the worley distance        
        float new_dist = length(worley_dif);
        //If the new distance is the nearest
        if (dist > new_dist)
        {
            //Store the new distance and cell coordinates
            dist = new_dist;
            voronoi_cell = sample_cell;
        }
    }
    //Get a random value using cell coordinates
    return hash1(voronoi_cell);
}
//Generate fractal value noise from multiple octaves
//oct - The number of octave passes
//per - Octave persistence value (should be between 0 and 1)
float fractal_noise(vec2 p, int oct, float per)
{
    float noise_sum = 0.0; //Noise total
    float weight_sum = 0.0; //Weight total
    float weight = 1.0; //Octave weight

    for(int i = 0; i < oct; i++) //Iterate through octaves
    {
        //Add noise octave to total
        noise_sum += value_noise(p) * weight; 
        //Add octave weight to total
        weight_sum += weight;
        //Reduce octave amplitude with persistence value
        weight *= per;
        //Rotate and scale for next octave
        p *= mat2(1.6,1.2,-1.2,1.6); 
    }
    //Compute weighted average
    return noise_sum / weight_sum; 
}

//Function demo
void mainImage( out vec4 fragColor, in vec2 fragCoord)
{
    //Compute 3x2 uv cells
    vec2 uv = fragCoord / vec3(u_resolution, 1.0).xy * vec2(3,2);
    
    //Use scaled coordinates
    vec2 coord = fragCoord/vec3(u_resolution, 1.0).y*10.0;
    //Scroll horizontally
    coord.x += u_time;
    
    //Initialize noise value
    float noise = 0.0;
    //Top-side
    if (uv.y>1.0)
    {
        //Hash noise
        if (uv.x<1.0) noise = hash1(mod(coord,7.31)*1e2);
        //Value noise
        else if (uv.x<2.0) noise = value_noise(coord);
        //Perlin noise
        else if (uv.x<3.0) noise = perlin_noise(coord);
    }
    //Bottom-side
    else
    {
        //Worley
        if (uv.x<1.0) noise = worley_noise(coord);
        //Voronoi
        else if (uv.x<2.0) noise = voronoi_noise(coord);
        //Fractal value noise
        else if (uv.x<3.0) noise = fractal_noise(coord, 6, 0.7);
    }
    //Compute shaded divider bars
    vec2 bars = pow(smoothstep(0.5,0.4,abs(fract(uv)-0.5)), vec2(0.1));
    
    //Output noise + divider bars
    fragColor = vec4(noise * bars.x * bars.y);
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
