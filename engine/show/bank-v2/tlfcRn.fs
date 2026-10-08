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
// Copyright 2020 Google LLC
// SPDX-License-Identifier: Apache-2.0
// NOT an official Google project; see Google Open Source guidelines for more info on copyright transfer
// written by William Cadegan-Schlieper 

vec3 color(vec3 z) {
    int i;
    float res = 0.0;
    for (i=0; i<160; i++) {
        z.x = mod(z.x,4.0);
        if (z.x > 3.0) {
            z.x -= 4.0;
        }
        if (z.x > 1.0) {
            z.x = 2.0-z.x;
        }
        
        if (z.y < -1.0) {
            z.y = -2.0 - z.y;
        }
        res += float(int((z.y+1.0) / 2.0));
        z.y = mod(z.y + 1.0, 4.0) - 1.0;
        if (z.y > 1.0) {
            z.y = 2.0 - z.y;
            res += 1.0;
        }
        if (dot(z,z) < 2.0) {
            z /= -0.5 * dot(z,z);
            z.z = abs(z.z);
            res = -res;
        } else {
            break;
        }
    }
    if (i%2!=0) {
        res = -res;
    }
    float col = 1.0 - 1.0 / abs(res);
    if (res > 0.0) {
        return col * vec3(1.0,0.5,0.0);
    } else {
        return col * vec3(0.0,0.5,1.0);
    }
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = 2.0 * (fragCoord - vec3(u_resolution, 1.0).xy * 0.5)/vec3(u_resolution, 1.0).y;
    float ds = 1.0 / vec3(u_resolution, 1.0).y;

    
    float t = u_time * 0.5;
    
    float period = 4.0 * log(2.0 + sqrt(5.0));
    while (t > period * 0.5) {
        t -= period;
    }
    
    
   
    
    
    vec3 z = vec3(uv, 1.3 * ds);
    z *= exp(-t);
    
    z.x -= sqrt(0.05);
    
    z /= dot(z,z);
    
    z.x += sqrt(5.0);
    
    
    fragColor = vec4(color(z),1.0);
    fragColor = pow(fragColor, vec4(1.0/2.2));
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
