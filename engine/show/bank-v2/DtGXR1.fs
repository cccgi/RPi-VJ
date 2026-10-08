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
#define PI     3.14159

// Shapes
// Basic: https://iquilezles.org/articles/distfunctions2d/
// Cut-up: https://iquilezles.org/articles/distgradfunctions2d/
// Gradient: https://iquilezles.org/articles/distgradfunctions2d/

vec3 palette(float t, vec3 a, vec3 b, vec3 c, vec3 d) {
    // Test values: http://dev.thi.ng/gradients/
    // Each value = vec3(red, green, blue)
    
    // a: y-Offset
    // b: Amp
    // c: Frequency
    // d: x-Offset (phase)
    
    // Oscilates c times with phase of d
    // Scaled and biased by a and b for ccontrast and brightness
    // If palette needs to cycle over 0..1 exactly make c = [int] * 0.5
    // For continuity make c = [int]
    return a + b*cos( 2.0*PI *(c*t+d) );
}

vec3 usePalette(float t) {
    // Test values: http://dev.thi.ng/gradients/
    vec3 a = vec3(0.5, 0.5, 2.0);
    vec3 b = vec3(0.5, 0.2, 0.5);
    vec3 c = vec3(2.0, 3.0, 1.0);
    vec3 d = vec3(1.358, 0.5, 0.5);
    
    return palette(t, a, b, c, d);
}

// !!! Very useful for testing:
// Applies one value to the left half of the screen and another to the right side so you can see the difference side by side.
float halfScreenValuePicker(float x, float leftVal, float rightVal) {
    if (x < vec3(u_resolution, 1.0).x / 2.0) {
        return leftVal;
    }
    return rightVal;
}

// Range canvas from -0.5 to 0.5
vec2 centeredNormCanvas(vec2 fragCoord) {
    return (fragCoord * 2.0 - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
}

vec2 addSpatialRepetion(vec2 uv, float scale) {
    return fract(uv * scale) - 0.5;
}

// pointOriginal: value of point when initilized
float pointDistanceCurved(vec2 point, vec2 pointOriginal) {
    return length(point) * exp(-length(pointOriginal));
}

float sinRings(float d, float t, float speed, float width) {
    return sin(d*speed + t)/width;
}

float glow(float d, float intensity, float contrast) {
    return pow(intensity / d, contrast);
}

vec3 scaleColorWithDistance(vec3 col, float d) {
    return col * d;
}

vec3 invert(vec3 v) {
    return vec3(1.0) - v;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // Range canvas from -0.5 to 0.5
    vec2 uv = centeredNormCanvas(fragCoord);
    // Original uv
    vec2 uv0 = uv;
    vec3 finalColor = vec3(0.0);
    
    // Iterate to create layers
    // Increase limit to add more layers
    for (float i = 0.0; i < 3.0; i++) {
        // Add spatial repetion
        // Non-int scalar for patterns
        // -0.5 lines fract up with middle
        uv = addSpatialRepetion(uv, 1.8);
    
        // Distance from center with slight curve added
        float d = pointDistanceCurved(uv, uv0);
        
        // Rings changes with each iteration
        vec3 col = usePalette(length(uv0) + i*1.5 + u_time*0.4);
        
        // Repeating rings in different phases with scalable with
        d = sinRings(d, u_time, 8., 8.);
        
        // Inside also gives positive 
        d = abs(d);
        
        // Glow intensity, second: contrast
        d = glow(d, 0.013, 2.);
        
        // Glow changes with distance
        finalColor += scaleColorWithDistance(col, d);
    }

    // Output to screen
    fragColor = vec4(finalColor,1.0);
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
