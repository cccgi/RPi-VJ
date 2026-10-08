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
// Concept: an animation of wave harmonics
// show 1st, 2nd, 3rd order harmonics somehow 

float divs = 2.2;

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec4 m = iMouse; 

	float dist = m.x - ( vec3(u_resolution, 1.0).x / 2.0 ); // centererd
    float speed = 0.0009; // 0.9 camera movement to plane 
    float timespeed =   0.1 ; // 0.1 wave movement on the ribbons
    float igt = u_time; // shorter version of time
   
    //float t = 7.0 - (u_time * speed);
    
    // MOUSE: multiple by my or mx to get value between 1 and 2
    float my = float( (m.y + vec3(u_resolution, 1.0).y ) / vec3(u_resolution, 1.0).y );
    float mx = float( (m.x + vec3(u_resolution, 1.0).y ) / vec3(u_resolution, 1.0).x );

    float t = 8.0 - speed ;

    // divs create staggered / staged division:
    //divs += t / 0.20;
    
    // let the mouse distance be the divs number, usually 1
    // idea: mouse to the far right equals 1
    divs += t / ( ( m.x + 2. ) /  vec3(u_resolution, 1.0).x );
    
	vec2 div = vec2( divs, divs*vec3(u_resolution, 1.0).y/vec3(u_resolution, 1.0).x );
	vec2 uv = fragCoord.xy / vec3(u_resolution, 1.0).xy;
    
    
    // center on screen
    
    uv -= 0.1 + (u_time*0.0001);	
    
    
    float b = 1.0*divs/vec3(u_resolution, 1.0).x;	// blur over 2.4 pixels
    //float b = 1000.1;	// sharp - no blur

    vec2 xy = div*uv;
    
    vec2 S; // speed of fundamental
    vec2 S2; // speed of 1st octave
	vec2 S3; // speed of 2nd harmonic (perfect fifth?)

    
	S.x = (xy.x + xy.y)*(xy.x - xy.y)*0.5;		// "velocity potential"
	S2.x = (xy.x + xy.y)*(xy.x - xy.y)*0.5;		// "velocity potential"
	S3.x = (xy.x + xy.y)*(xy.x - xy.y)*0.5;		// "velocity potential"

    S.y = xy.x*xy.y;						// stream function
	S2.y = xy.x*xy.y;						// stream function	
	S3.y = xy.x*xy.y;						// stream function

    
    
    // speed of the dots
    S.x   -=  u_time        *timespeed;						// animate stream
    S2.x  -=  u_time        *timespeed + (my * 20. );						// animate stream
    S3.x  -=  u_time        *timespeed + (my * 20.);						// animate stream

    //S2.x -= (u_time + my) *timespeed; //  * (m.y - (vec3(u_resolution, 1.0).y/2.) / vec3(u_resolution, 1.0).y) ;						// animate stream
	//S3.x -= (u_time + my * 2. ) *timespeed; //  * (m.y - (vec3(u_resolution, 1.0).y/2.) / vec3(u_resolution, 1.0).y) ;						// animate stream
    
    
    
	// HERE IS THE WAVE HARMONICS 
    // SXY IS FUNDAMENTAL WAVE2 IS 2ND HARMONIC
    // sxy is *probably* the main wave
    // wave2 is the 2nd harmonic
    
    vec2 sxy =   sin(3.14159 * S  * 1.0 );
    vec2 wave2 = sin(3.14159 * S2 * 2.0 ); // 2.00 is double
    vec2 wave3 = sin(3.14159 * S3 * 3.0 ); // 3.00 is double
    
    
    // w2 is the 2nd harmonic
    float a = sxy.x * sxy.y;		// combine sine waves using product
    float w2 = wave2.x * wave2.y;	// combine sine waves using product
    float w3 = wave3.x * wave3.y;	// combine sine waves using product
    
    // not sure what this does but we will do it to w2, w3 as well
    a = 0.5*a + 0.5;							// remap to [0..1]
	a = smoothstep( 0.85-b, 0.85+b, a );		// threshold
    
    w2 = 0.5*a + 0.5;							// remap to [0..1]
	w2 = smoothstep( 0.85-b, 0.85+b, a );		// threshold
    
    w3 = 0.5*a + 0.5;							// remap to [0..1]
	w3 = smoothstep( 0.85-b, 0.85+b, a );		// threshold
    
    float c = sqrt( a  );						// correct for gamma
	float w2gamma = sqrt( w2 );					// correct for gamma
	float w3gamma = sqrt( w3 );					// correct for gamma
    
    float red, green, blue;
    
    red   =    sxy.x - sxy.y ;
    green =  wave2.x - wave2.y;
    blue  =  wave3.x - wave3.y;
    if (red < 0.) {
        red = red * -1.;
        green = green * -1.;
        blue = blue * -1.;

    }
    
    
    fragColor = vec4( red, green, blue, 1.0);
    //fragColor = vec4(sxy.x, c, sxy.y, 1.0);

    
    //red =  sxy.x + wave2.y;
    //green =  sxy.x + wave2.y;
    //blue =  sxy.x + wave2.y;
    
        
    //red =  sxy.x + wave2.y;
    //green =  sxy.x + wave2.y;
    //blue =  sxy.x + wave2.y;
    
        
    //red =  sxy.x + sxy.y + wave2.x + wave2.y + wave3.x + wave3.y;
    //green =   wave2.x - wave2.y;
    //blue =   wave3.x - wave3.y;
    
    
    // NICE WHITE RIBBON TYPE WAVE
    //red =  sxy.x + sxy.y + wave2.x + wave2.y + wave3.x + wave3.y;
    //green =  sxy.x + sxy.y + wave2.x + wave2.y + wave3.x + wave3.y;
    //blue =  sxy.x + sxy.y + wave2.x + wave2.y + wave3.x + wave3.y;

    // blue lines yellow and white balls
    // fragColor = vec4(c, c, sxy.y, 1.0);
    
    
    // blue lines yellow and white balls
    //fragColor = vec4(  (c*2.0) - (w2gamma*2.0)  - (w3gamma*3.0) , c*2.0, sxy.y + wave2.y, 1.0);
    //fragColor = vec4(  (c*2.0) - (w2gamma*2.0)  , c*2.0, sxy.y + wave2.y, 1.0);
    //fragColor = vec4(c*2.0, wave2.y, sxy.y + wave2.y, 1.0);

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
