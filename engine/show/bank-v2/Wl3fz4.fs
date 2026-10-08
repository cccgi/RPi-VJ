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
// Created by inigo quilez - iq/2013
// License Creative Commons Attribution-NonCommercial-ShareAlike 3.0 Unported License.

// More info here:  https://iquilezles.org/articles/lyapunovfractals
        

//for(i=2e3;i--;x.fillRect(i,t*240,h/=9,4)){I=i/430,Y=t/8-1,h=X=.5
//for(n=9;n--;
// X*=Y*I*(1-X)*(1-I*X*(1-X)));
//h+=Math.log2((Y*I*(1-1.5*X))**2)}

        
vec3 calc( in vec2 p )
{
    float x = 0.5;
	float h = 0.5;
	for( int i=0; i<9; i++ )
	{
    
//x *= p.y * p.x * (1.-x) * ( 1. - p.y * p.x * (1.-x) ) ;
//h += log(abs(p.y * p.x * (1. - 1.5*x)));

//x = p.x * x *(1.0-x);


x *= p.y* p.x *(1.0-x+ sin(u_time)/29. ) * (1.1-(p.x * x *(1.0-x)));

//h += log2( abs( p.x*(1.0-2.0*x + p.y - p.y*1.5*x )) );

h += log2( abs( p.y*p.x*(1.0-1.5*x )) );


//h+=Math.log2((Y*I*(1-1.5*X))**2)


//h += log2(abs(p.x*(1.0-2.0*x) + p.y*(1.0-2.0*x))); //+ log2(abs());

//		x = p.x*x*(1.0-x);



// esse eh **2 ao inves de abs:
  //  h += log2( pow(2.,( p.x*(1.0-2.0*x + p.y - p.y*1.0*x )) ));


//    h += log2( abs( p.x*(1.0-2.0*x + p.y - p.y*1.0*x )) );





//		x = p.x*x*(1.0-x); h += log2(abs(p.x*(1.0-2.0*x)));
//	x = p.x*x*(1.0-x); h += log2(abs(p.x*(1.0-2.0*x)));
//		x = p.x*x*(1.0-x); h += log2(abs(p.x*(1.0-2.0*x)));

//		x = p.y*x*(1.0-x);
//        h += log2(abs(p.y*(1.0-2.0*x)));

//		x = p.y*x*(1.0-x);
//h += log2(abs(p.y*(1.0-2.0*x)));
//		x = p.y*x*(1.0-x); h += log2(abs(p.y*(1.0-2.0*x)));
//		x = p.y*x*(1.0-x); h += log2(abs(p.y*(1.0-2.0*x)));
//		x = p.y*x*(1.0-x); h += log2(abs(p.y*(1.0-2.0*x)));
	}
    h /= 9.0;
	
	
	vec3 col = vec3(0.0);

		//h = abs(h);
		col = 1.0*( vec3(1.0,0.4,0.7) + h );
//		col *= pow(h,0.1);



	return col;
}
	
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{

    vec3 col = calc( vec2(0.7,2.1) + 4.0*(fragCoord)/ vec3(u_resolution, 1.0).x );
    col /= 3.0;

	
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
