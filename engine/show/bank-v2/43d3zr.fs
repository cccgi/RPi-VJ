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
// A Fractal shader.
// Use if you want, CC0? Is that legal? Provided under CC0 license. 

// This checks for an intersection in a single axis, since
// all axis share that behavior.
//
// The behavior is nonstandard. it returns the color if there's
// a hit, but the coordinates for the next level if it's a miss.
// This is communicated with the w component, which is the distance
// but is posiitve on hit and negative on miss. 
vec4 cast_ray_1d(vec3 vector_pos, vec3 vector_dir, int mdim, int dim2, int dim3) {
    float closest_wall = 1000.0;
    vec3 norm = vec3(0);
    bool flip = false;
    
    // Flipping the direction if negative simplifies this code.
    if (vector_dir[mdim] < 0.0) {
        vector_dir *= -1.0;
        vector_pos *= -1.0;
        flip = true;
    }
    
    if (vector_pos[mdim] < 1.0) {
        float dist_n = (1.0 - vector_pos[mdim])/vector_dir[mdim];
        
        vec3 hit_pos = vector_pos + (vector_dir * dist_n);
        
        if (abs(hit_pos[dim2])<=1.0 && abs(hit_pos[dim3])<=1.0) {
            float dist = distance(hit_pos, vector_pos);
            
            if (dist < closest_wall) {
                closest_wall = dist;
                norm[mdim] = 0.5;
                
                // Check if a hit.
                if (abs(hit_pos[dim2])<=0.5 && abs(hit_pos[dim3])<=0.5) {
                    // This is a miss, so you calculate the location of 
                    // the ray for the next iteration.
                    if (flip) {
                        hit_pos *= -1.0;
                    }
                    norm = vec3(hit_pos)*2.0;
                    norm[mdim] = -1.0;
                    if (flip) {
                        norm[mdim] = 1.0;
                    }
                    return vec4(norm, -closest_wall);
                } else {
                    // Color the sides to imply some lighting.
                    if (mdim == 0) {
                        norm = vec3(0.4);
                    } else if (mdim == 1) {
                        norm = vec3(0.5);
                    } else if (mdim == 2) {
                        norm = vec3(0.6);
                    }
                    
                    // This is to add a border for looks.
                    if ((abs(hit_pos[dim2])<=0.6 && abs(hit_pos[dim3])<=0.6) ||
                        (abs(hit_pos[dim2])>=0.9 || abs(hit_pos[dim3])>=0.9) ) {
                        norm *= 0.8;
                    }
                }
            }
        }
    }
    
    return vec4(norm, closest_wall);
}

vec3 cast_ray(vec3 vector_pos, vec3 vector_dir) {
    vec4 ret = vec4(0); 
    float dist = 0.0;

    // pseudo-recursive loop for the fractal.
    for (int i=0; i<8; i++) {  
        // Check each axis.
        vec4 x_int = cast_ray_1d(vector_pos, vector_dir, 0, 1, 2); 
        vec4 y_int = cast_ray_1d(vector_pos, vector_dir, 1, 0, 2); 
        vec4 z_int = cast_ray_1d(vector_pos, vector_dir, 2, 0, 1); 

        // Find the closest hit. This works despite the potentially
        // flipped x because it only flipps if it's a miss inside
        // the wall, misses which miss the wall entirely are set
        // to distance 1000.
        if (x_int.w < y_int.w) {
            if (x_int.w < z_int.w) { 
                ret = x_int; 
            } else { 
                ret = z_int; 
            } 
        } else { 
            if (y_int.w < z_int.w) { 
                ret = y_int; 
            } else { 
                ret = z_int; 
            } 
        } 
        
        // This is for fog.
        dist += pow(abs(ret.w/2.0),2.0);

        if (ret.w < 0.0) { 
            vector_pos = ret.xyz;
        } else {
            // Apply fog if a hit.
            return ret.xyz * (1.0-(dist/6.0));
        }
        
    }

}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Normalized pixel coordinates (from 0 to 1)
    vec2 uv = fragCoord/vec3(u_resolution, 1.0).xy;
    
    uv.x -= 0.5;
    uv.y -= 0.5;

    // Direction of the ray.
    vec3 in_vec_d = vec3(sin(u_time/5.0),0,cos(u_time/5.0));
    
    // Position of the ray, this has a modulo to simiulate
    // infinite movement.
    vec3 in_vec_p = vec3(0, 0, ((pow(mod(u_time*-0.3501, 1.0), (1.0+(in_vec_d.z/-5.0))) * 2.0)-1.0));

    float fov = 1.0;
    vec3 in_vec_d_hold = in_vec_d;

    in_vec_d.x += (in_vec_d_hold.z*(uv.x*fov));
    in_vec_d.z -= (in_vec_d_hold.x*(uv.x*fov));
    
    in_vec_d.y += uv.y*fov/1.8;

    vec3 norm = cast_ray(in_vec_p, in_vec_d);

    vec3 col = norm;

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
