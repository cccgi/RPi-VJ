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
//////////////////////////////////////////////////////////////////////////////////
// Eye of God Zoom - Copyright 2017 Frank Force
// License Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License.
//////////////////////////////////////////////////////////////////////////////////

const float zoomSpeed			= 10.0;	// how fast to zoom (negative to zoom out)
const float zoomScale			= 0.00001;	// how much to multiply overall zoom (closer to zero zooms in)
const int recursionCount		= 8;	// how deep to recurse
const int glyphSize				= 3;	// width & height of glyph in pixels
const float curvature			= -16.0;	// time warp to add curvature

//////////////////////////////////////////////////////////////////////////////////
// Precached values and math

const float glyphSizeF = float(glyphSize);
const float glyphSizeLog = log(glyphSizeF);

const float e = 2.718281828459;
const float pi = 3.14159265359;
float RandFloat(int i) { return (fract(sin(float(i)) * 43758.5453)); }
int RandInt(int i) { return int(100000.0*RandFloat(i)); }

vec3 HsvToRgb(vec3 c) 
{
    float s = c.y * c.z;
    float s_n = c.z - s * .5;
    return vec3(s_n) + vec3(s) * cos(2.0 * pi * (c.x + vec3(1.0, 0.6666, .3333)));
}

//////////////////////////////////////////////////////////////////////////////////
// Color and image manipulation

float GetRecursionFade(int r, float timePercent)
{
    if (r > recursionCount)
        return timePercent;
    
    // fade in and out recusion
    float rt = max(float(r) - timePercent, 0.0);
    float rc = float(recursionCount);
    return rt / rc;
}

vec3 InitPixelColor() { return vec3(0); }
vec3 CombinePixelColor(vec3 color, float timePercent, int i, int r, vec2 pos, ivec2 glyphPos, ivec2 glyphPosLast)
{
    i = (i+r) + (glyphPosLast.y + glyphPos.y);

    vec3 myColor = vec3
    (
    	mix(-0.05, 0.05, RandFloat(i)),
    	mix(0.0, 0.2, RandFloat(i + 10)),
        mix(0.0, 0.2, RandFloat(i + 20))
    );

    // combine with my color
    float f = GetRecursionFade(r, timePercent);
    //myColor.y = pow(myColor.y, 2.0);
    //myColor.z = pow(myColor.z, 2.0);
    color += myColor*f;
    return color;
}

vec3 FinishPixel(vec3 color, vec2 uv)
{
    // color wander
    color.x += 0.01*u_time;
    
    // convert to rgb
    color = HsvToRgb(color);
    return color;
}

vec2 InitUV(vec2 uv)
{
    // spin
    float theta = pi/2.0;//0.05*u_time;
	float c = cos(theta);
	float s = sin(theta);
	uv = vec2((uv.x*c - uv.y*s), (uv.x*s + uv.y*c));
    
	// wave
	uv.x += 0.05*sin(10.0*uv.y + 0.17*u_time);
	uv.y += 0.05*sin(10.0*uv.x + 0.13*u_time);
	uv.x += 0.2*sin(1.0*uv.y + 0.31*u_time);
	uv.y += 0.2*sin(1.0*uv.x + 0.27*u_time);
    return uv;
}


//////////////////////////////////////////////////////////////////////////////////
// Fractal functions

ivec2 GetFocusPos(int i) { return ivec2(glyphSize/2); }
      
// get color of pos, where pos is 0-1 point in the glyph
vec3 GetPixelFractal(vec2 pos, int iterations, float timePercent)
{
	ivec2 glyphPosLast = GetFocusPos(-2);
	ivec2 glyphPos =     GetFocusPos(-1);
	vec3 color = InitPixelColor();
    
	for (int r = 0; r <= recursionCount + 1; ++r)
	{
        color = CombinePixelColor(color, timePercent, iterations, r, pos, glyphPos, glyphPosLast);
        if (r > recursionCount)
			return color;
           
        // update pos
        pos *= glyphSizeF;

        // get glyph and pos within that glyph
        glyphPosLast = glyphPos;
        glyphPos = ivec2(pos);
        
        // next glyph
		pos -= vec2(floor(pos));
	}
}
 
//////////////////////////////////////////////////////////////////////////////////
	
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
	// use square aspect ratio
	vec2 uv = fragCoord;
	uv = fragCoord / vec3(u_resolution, 1.0).y;
	uv -= vec2(0.5*vec3(u_resolution, 1.0).x / vec3(u_resolution, 1.0).y, 0.5);
    uv = InitUV(uv);
    
    // time warp
    float time = u_time + curvature*pow(length(uv), 0.2);
    
	// get time 
	float timePercent = time*zoomSpeed;
	int iterations = int(floor(timePercent));
	timePercent -= float(iterations);
	
	// update zoom, apply pow to make rate constant
	float zoom = pow(e, -glyphSizeLog*timePercent);
	zoom *= zoomScale;
    
	// get offset
	vec2 offset = vec2(0);
    const float gsfi = 1.0 / glyphSizeF;
	for (int i = 0; i < 13; ++i)
		offset += (vec2(GetFocusPos(i)) * gsfi) * pow(gsfi,float(i));
    
	// apply zoom & offset
    vec2 uvFractal = uv * zoom + offset;
	
	// check pixel recursion depth
	vec3 pixelFractalColor = GetPixelFractal(uvFractal, iterations, timePercent);
    pixelFractalColor = FinishPixel(pixelFractalColor, uv);
    
	// apply final color
	fragColor = vec4(pixelFractalColor, 1.0);
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
