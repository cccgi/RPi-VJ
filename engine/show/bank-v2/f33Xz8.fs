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
// The MIT License
// Copyright © 2015 Inigo Quilez
// Copyright © 2026 Yakov Galka
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions: The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

// Signed distance to a 2D rounded box.
// This is based on https://www.shadertoy.com/view/4llXD7
// However, it supports corner radii that are greater than 
// the radius of the box (Much like CSS).


// SDF of a single round corner
float sdRoundCorner(vec2 p, vec2 b, float r)
{
    vec2 q = p - (b - r);
    return min(max(q.x, q.y), 0.0) + length(max(q, 0.0)) - r;
}

// b.x = half width
// b.y = half height
// r.x = roundness top-right  
// r.y = roundness bottom-right
// r.z = roundness top-left
// r.w = roundness bottom-left
float sdRoundBox(vec2 p, vec2 b, vec4 r)
{
    // can skip the normalization if 
    //   sum of the radii along an edge don't exceed box width/height
    //   or having a crease is ok (it's still continuous regardless)
#if 1
    vec4 f = (r.xxww + r.zyyz)/b.xyxy;
    f.xy = max(f.xy, f.zw);
    f.x = max(f.x, f.y);
    r /= max(0.5*f.x, 1.0);
#endif

    vec4 q = vec4(p.xy, -p.xy);
    float d = sdRoundCorner(q.xy, b, r.x);
    d = max(d, sdRoundCorner(q.xw, b, r.y));
    d = max(d, sdRoundCorner(q.zy, b, r.z));
    d = max(d, sdRoundCorner(q.zw, b, r.w));
    return d;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
	vec2 p = (2.0*fragCoord-vec3(u_resolution, 1.0).xy)/vec3(u_resolution, 1.0).y;
    vec2 m = (2.0*iMouse.xy-vec3(u_resolution, 1.0).xy)/vec3(u_resolution, 1.0).y;

	vec2 si = vec2(0.9,0.6) + 0.3*cos(u_time+vec2(0,2));
    vec4 ra = 0.5 + 0.5*cos( 2.0*u_time + vec4(0,1,2,3) );
    
	float d = sdRoundBox( p, si, ra );

    vec3 col = (d>0.0) ? vec3(0.9,0.6,0.3) : vec3(0.65,0.85,1.0);
	col *= 1.0 - exp2(-20.0*abs(d));
	col *= 0.8 + 0.2*cos(120.0*d);
	col = mix( col, vec3(1.0), 1.0-smoothstep(0.0,0.01,abs(d)) );

    if( iMouse.z>0.001 )
    {
    d = sdRoundBox(m, si, ra );
    col = mix(col, vec3(1.0,1.0,0.0), 1.0-smoothstep(0.0, 0.005, abs(length(p-m)-abs(d))-0.0025));
    col = mix(col, vec3(1.0,1.0,0.0), 1.0-smoothstep(0.0, 0.005, length(p-m)-0.015));
    }

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
