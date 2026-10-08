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
// 08:41 2025-3-4 1558
// 
// Finally managed to put it together.



// thanks to @FabriceNeyret2
void mainImage(out vec4 O, vec2 U) {
        O = vec4(0);
        vec2  R = vec3(u_resolution, 1.0).xy,
              u = 5. * (U+U - R) / R.y;
        int   I = int(iMouse.x * 5. / R.x);
        float r = length(u),
              t = u_time,
              f = fract( atan(u.y, u.x) / 6.283 + .5 - t ),
              k = I == 0 ? 3. + 1.8 * sin(t*.6)
                : I == 2 ? .5 +  .4 * sin(t *.5)
                :          3.,
              h = f - floor( log(r)/log(k) + f );
        u *= pow(k, h);
        
        if( I == 4 ){              
            vec2 v = abs(u); v.x < v.y ? v = v.yx : v;
              //if ( u != vec2(1) ) u /= k;
                O = texture(iChannel0, u/8. + .5);
                u = abs(abs(u) - 1.);
                O.x += exp(-20. * min(u.x,u.y));
            if(iMouse.y>R.y*.5)O -= 1. *exp(-30. *abs(length(v)-1.)), O += 1.*k*exp(-10.*k*abs(length(v)-k ));               
                return;
        }
        u = abs(u);
        u.x < u.y ? u = u.yx : u;
        
        if ( I ==3 && u.y < 1. || u.x > 1. ) u /= k, h--;
        
        k = exp( abs(abs(u.x - 1.)) / .1 );
 
        for (int i; i++ < 4; u-- )
                u = abs(u * 3.) - 1.,
                u.x > 0. || u.y > 0. ? u = abs(u) : u;
  
        O = vec4( dot(u,u) < 3. ? 1. : .5 );
        O.x -= r/k;
        O *= .5 + .5 * cos( h * vec4(.8, 1.6, 2.4, 0) - .3 );
} 




/*
void mainImage(out vec4 O, vec2 U) {
        ivec2 v = ivec2(U);
        vec2 R = vec3(u_resolution, 1.0).xy,
                u = 5. * (U + U - R) / R.y;
        float a = atan(u.y, u.x),
                b = a / 6.2832 + .5,
                r = length(u);
        float k = 3., f = fract(b - u_time);
        int ID = int(iMouse.x * 5. / R.x);
        if (ID == 0 || ID == 4)
                k = 3. + 1.8 * sin(u_time * .6);
        else if (ID == 2)
                k = .5 + .4 * sin(u_time * .5);
        float h = f - floor(log(r) / log(k) + f);
        u *= pow(k, h);
        if(ID==4){
                if (length(max(abs(u) - 1., 0.)) > 0.) u = u / k, h += 1.;
                O = texture(iChannel0, u * .25 + .5);
                O.x += exp(-20. * min(abs(abs(u.x) - 1.), abs(abs(u.y) - 1.)));
                return;
        }
        u = abs(u);
        if (ID ==3) {
                if (u.x < 1. || u.y < 1.) u = u / k, h--;
        } else {
                if (u.x > 1. || u.y > 1.) u = u / k, h--;
        }
        if (u.x < u.y) u = u.yx;
        float c = vec4(exp(-10. * abs(abs(u.x - 1.)))).x;
        // Sierpinski
        for (int i = 0; i < 4; i++) {
                u = abs(u * 3.) - 1.;
                if (u.x > 0. || u.y > 0.) u = abs(u);
                u--;
        }
        O = vec4(0.5);
        if (dot(u, u) < 3.) O = vec4(1);
        c *= r;
        O.x -= c;
        O *= .5 + .5 * cos( -h * 8. * vec4(.1, .2, .3, 0) + .3);
} 

*/










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
