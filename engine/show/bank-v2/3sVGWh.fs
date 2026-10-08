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
        Copyright (c) 2019 Joshua Dyer

		Special thanks to The Art of Code (https://www.youtube.com/watch?v=il_Qg9AqQkE) for the basis of this shader.

		I modified it some for my own understanding and to add parameters for playing around with it.
		Any errors/bugs in the code (or comments) are my own.
		Shader Variables (Params) are found at the top of mainImage. Feel free to play around with them.
		Comments may or may not make 100% sense. Watch the video linked above for a better understanding of the shader.

		Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation
		files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy,
		modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
		Software is furnished to do so, subject to the following conditions:

		The above copyright notice and this permission notice shall be included in all copies or substantial portions of the 
		Software.


		THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE
		WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
		COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE,
		ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/

//Calculate normal vector.
vec2 N(float angle) {
    return vec2(sin(angle), cos(angle));
}

//Rotate around an arbitrary line by a given angle.
vec2 rotate(vec2 uv, vec2 cp, float a, bool side) {
    vec2 n = N(a * 3.14159);
    float d = dot(uv - cp, n);
    if (side) {
	    uv -= n * max(0.0, d) * 2.0;
    } else {
        uv -= n * min(0.0, d) * 2.0;
    }
    return uv;
}

//Used if needing the distance for showing the mirroring line.
float dist(vec2 uv, vec2 cp, float a) {
    vec2 n = N(a * 3.14159);
    return dot(uv - cp, n);
}

void mainImage( out vec4 fragColor, in vec2 fragCoord)
{
    /* Shader Variables */
    int iterations = 5;//Number of fractal iterations.
    float thickness = 2.0;//Thickness of the lines to be drawn.
    bool mouseOn = false;//Should click dragging affect the rotation?
    bool trippyInAndOut = true;//Should the image trip in and out over time?
    bool trippyTexture = true;//Should the image use the texture in iChannel0, changing over time?
    bool showUVFolding = false;//Use primary colors to show UV folding/mirroring?
    /* End Shader Variables */
    
    if (iterations < 1) {
        iterations = 1;
    }
    
    if (thickness <= 0.0) {
        thickness = 1.0;
    }
    
    vec2 uv = (fragCoord - 0.5 * vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;//Center Origin, Remap to -0.5 to +0.5, and square using aspect ratio.
    
    vec2 mouse = iMouse.xy / vec3(u_resolution, 1.0).xy;//Useful for finding exact mirroring angles to use, just place mouse.x in place of angle
    if (!mouseOn) {
        mouse = vec2(1.0);//If not using mouse, lock to 1.0, 1.0 (because it is multiplied by a value below.)
    }
    
	uv *= 1.25; //Zoom out.
    uv.y += tan((5.0 / 6.0) * 3.14159) * 0.5;// Re-Center
    
    vec3 col = vec3(0);//Set all black.
    
    uv.x = abs(uv.x);//Mirror on Y axis.
    
    uv = rotate(uv, vec2(0.5, 0.0), 5.0 / 6.0, true);//Rotate UV around a line passing through (0.5, 0.0) by (5.0 / 6.0) angle.

    //col += smoothstep(0.01, 0.0, abs(dist(uv, vec2(0.5, 0.0), mouse.x)));//Show the mirroring line.
   
    float scale = 1.0;//Set initial scale.
    uv.x += 0.5;//Shift right by 1 /2 unit.
    for (int i = 0; i < iterations; i++) {//Loop through the number of iterations for the fractal.
        uv *= 3.0;//Scale UV space by a factor of 3
        scale *= 3.0;//Keep track of total scale change.
        uv.x -= 1.5;//Shift left by 1.5 Units
        
        uv.x = abs(uv.x);//Mirror on Y axis
        
        uv.x -= 0.5;//Shift left by 1/2 Unit
        uv = rotate(uv, vec2(0.0, 0.0), mouse.y * 2.0 / 3.0, false);//Fold to create mirrored rotated segments. (The ^ part.)
    }
    
    if (trippyInAndOut) {
        //For trippy effect.
        uv *= cos(u_time * 0.5);
        uv = rotate(uv, vec2(0.0, 0.0), cos(u_time), false);
    }
    
    if (trippyTexture) {
        //Very trippy effect!
        uv /= scale;
        col += texture(iChannel0, uv * 2.0 - u_time * 0.1).rgb;
    } else {
        //Calculate the color based on the distance from the line. Until now, just shifting, scaling, mirroring UV space.
        //Remember uv space has been mirrored repeatedly to create the fractal outline. 
        //So we are only drwaing one line, but it is crumpled up.
        float d = length(uv - vec2(clamp(uv.x, -1.0, 1.0), 0));
        col += smoothstep(thickness / vec3(u_resolution, 1.0).y, 0.0, d / scale);//Smooth out and thicken the lines. and adjust based on scale.
    }
    
    if (showUVFolding) {
    	col.rg += uv;//Demonstrate UV space folding/mirroring.
    }
    
    fragColor = vec4(col, 1.0);//Output the color.
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
