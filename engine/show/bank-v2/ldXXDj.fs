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
// Created by inigo quilez - iq/2014
//   https://www.youtube.com/c/InigoQuilez
//   https://iquilezles.org/

// A simple and cheap 2D shader to accompany the Pirates of the Caribean music.

float fbm( vec2 p )
{
    return 0.5000*texture( iChannel1, p*1.00 ).x + 
           0.2500*texture( iChannel1, p*2.02 ).x + 
           0.1250*texture( iChannel1, p*4.03 ).x + 
           0.0625*texture( iChannel1, p*8.04 ).x;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    float time = mod( u_time, 60.0 );
	vec2 p = (2.0*fragCoord-vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
    vec2 i = p;

    // camera
    p += vec2(1.0,3.0)*0.001*2.0*cos( u_time*5.0 + vec2(0.0,1.5) );    
    p += vec2(1.0,3.0)*0.001*1.0*cos( u_time*9.0 + vec2(1.0,4.5) );    
    float an = 0.3*sin( 0.1*time );
    float co = cos(an);
    float si = sin(an);
    p = mat2( co, -si, si, co )*p*0.85;
    
    // water
    vec2 q = vec2(p.x,1.0)/p.y;
    q.y -= 0.9*time;    
    vec2 off = texture( iChannel0, 0.1*q*vec2(1.0,2.0) - vec2(0.0,0.007*u_time) ).xy;
    q += 0.4*(-1.0 + 2.0*off);
    vec3 col = 0.2*sqrt(texture( iChannel0, 0.05*q *vec2(1.0,4.0) + vec2(0.0,0.01*u_time) ).zyx);
    float re = 1.0-smoothstep( 0.0, 0.7, abs(p.x-0.6) - abs(p.y)*0.5+0.2 );
    col += 1.0*vec3(1.0,0.9,0.73)*re*0.2*(0.1+0.9*off.y)*5.0*(1.0-col.x);
    float re2 = 1.0-smoothstep( 0.0, 2.0, abs(p.x-0.6) - abs(p.y)*0.85 );
    col += 0.7*re2*smoothstep(0.35,1.0,texture( iChannel1, 0.075*q *vec2(1.0,4.0) ).x);
    
    // sky
    vec3 sky = vec3(0.0,0.05,0.1)*1.4;
    // stars    
    sky += 0.5*smoothstep( 0.95,1.00,texture( iChannel1, 0.25*p ).x);
    sky += 0.5*smoothstep( 0.85,1.0,texture( iChannel1, 0.25*p ).x);
    sky += 0.2*pow(1.0-max(0.0,p.y),2.0);
    // clouds    
    float f = fbm( 0.002*vec2(p.x,1.0)/p.y );
    vec3 cloud = vec3(0.3,0.4,0.5)*0.7*(1.0-0.85*smoothstep(0.4,1.0,f));
    sky = mix( sky, cloud, 0.95*smoothstep( 0.4, 0.6, f ) );
    sky = mix( sky, vec3(0.33,0.34,0.35), pow(1.0-max(0.0,p.y),2.0) );
    col = mix( col, sky, smoothstep(0.0,0.1,p.y) );

    // horizon
    col += 0.1*pow(clamp(1.0-abs(p.y),0.0,1.0),9.0);

    // moon
    float d = length(p-vec2(0.6,0.5));
    vec3 moon = vec3(0.98,0.97,0.95)*(1.0-0.1*smoothstep(0.2,0.5,f));
    col += 0.8*moon*exp(-4.0*d)*vec3(1.1,1.0,0.8);
    col += 0.2*moon*exp(-2.0*d);
    moon *= 0.85+0.15*smoothstep(0.25,0.7,fbm(0.05*p+0.3));
    col = mix( col, moon, 1.0-smoothstep(0.2,0.22,d) );
    
    // postprocess
    col = pow( 1.4*col, vec3(1.5,1.2,1.0) );    
    col *= clamp(1.0-0.3*length(i), 0.0, 1.0 );

    // fade
    col *=       smoothstep( 3.0, 6.0,time);
    col *= 1.0 - smoothstep(44.0,50.0,time);

    fragColor = vec4( col, 1.0 );
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
