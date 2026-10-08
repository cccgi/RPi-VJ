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
vec2 DOMAIN_XY = vec2(16., 9.); // RAZMER OBLASTI
float speed = 6.;              // skorost' metamorfoz
                              // KIFS-konstanti
const int N = 3;             // N - kol-vo vektorov simmetrii
const int N_but_fancy = 3;  // esli hochu uzaty tol'ko chasty vektorov
const int ITER = 5;       // WHYYY IT MUST BE CONSTANT, STUPID GLSL LOOPS

                         //colors constants
vec3 white = vec3(1.,1.,1.);
vec3 black = vec3(0.,0.,0.);
vec3 red   = vec3(1.,0.,0.);
vec3 blue  = vec3(0.,0.,1.);
vec3 green = vec3(0.,1.,0.);
                     //      funksii      kogda tut dve leviye (lol) kosiye cherti, ploho))
float gauss(in vec2 p, in float s) {return exp( -(p.x*p.x+p.y*p.y)/(2.*s*s) );}
        //tupoy 2d gauss s centrom v (0,0)
float gauss_1d(float x, float m, float s) {return exp( -((x-m)*(x-m))/(2.*s*s) );}
   //1d gauss, ne delil na sigmu (s), no pohuy ( return (1/.s)*exp(...) )
vec2 a2v(in float a){return vec2(cos(radians(a)), sin(radians(a)));}
  //gradus -> vektor simmetrii




void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    // sozdal prostranstvo (uv) razmerom v DOMAIN s tsentrom v (0,0)
    vec2 uv = fragCoord*DOMAIN_XY/vec3(u_resolution, 1.0).xy - .5*DOMAIN_XY;

                         //  | | |
 // KIFS KIFS KIFS KIFS KIFS v v v
// -------- initialization ----------
    float scale = 2.;
    
        vec2 symm[N];                                //symm[0] = a2v(60.); symm[1] = a2v(-60.);
    
    symm[0] = a2v(0.);           //a2v(60. + u_time*speed); //symm[0] = a2v(-60. + iMouse.x); symm[1] = a2v(60. + iMouse.y);
    symm[1] = a2v(45.); //a2v(-60.);
    symm[2] = a2v(135.);
    
        float rot[N];
    
    rot[0] = 0.;
    rot[1] = 90.; // in mandelbulber alpha (Rotate_Z) = 90.
    rot[2] = 0.; // 0 or 90 ???
    
        mat2 rot_matr[N];
    
    rot_matr[1][0][0] = cos(radians(rot[1])); rot_matr[1][0][1] = -sin(radians(rot[1]));
    rot_matr[1][1][0] = sin(radians(rot[1])); rot_matr[1][1][1] = cos(radians(rot[1]));
    
        bool mirror[N];
        
    mirror[0] =
    mirror[1] = false;
    mirror[2] = true; // in mandelbulber gamma (Rotate_(X or Y)) = 180.
    
    
    vec2 offset = vec2(.0,.0);                //vec2 offset = vec2(-1.+2.*iMouse.x/vec3(u_resolution, 1.0).x, -1.+2.*iMouse.y/vec3(u_resolution, 1.0).y);
        
        float dist[N];
    
    dist[0] = -1.;
    dist[1] = -1.;
    dist[2] = -1.;
    
    
// --------  algorithm  ------------- 
    for(int i=0; i<ITER; i++) {
        for(int n=0; n<N_but_fancy; n++) {
            // rotate and mirror-rotate
            if ( rot[n] != 0.0 ) {
                uv = rot_matr[n] * uv;
            }
        
          // symm vecs 
            float len = dot(uv, symm[n]);
            if( len < dist[n] ) {
                uv -= 2.0*(len - dist[n])*symm[n]; //uv -= 2.0*(len - dist[i])*symm[i]
            }
        }
        
        // scaling
        uv*=scale;
        uv -= (scale - 1.) * offset;
        
        if(float(i) > mod(30.*u_time, float(ITER))) break; // stroboscope
    }
  // --------------------------------
 // Output to screen
 
    vec3 koltso  = gauss_1d ( sqrt(dot(uv, uv)), sin(u_time) , 1. ) * red;
    vec3 koltso2 = gauss_1d ( sqrt(dot(uv, uv)), sin(u_time)+1.5*sin(.6*u_time)+3. , 1.5 )
                        *
                   (.9*green + .2*blue);
    vec3 col_g = gauss(uv, .1)*white;
   


    //vec2 norm = fragCoord/vec3(u_resolution, 1.0).xy;
    //vec3 col_ex = 0.5 + 0.5*cos(2.35+norm.xyx+vec3(0,2,4));
    //col_ex *= step( max( mod(uv.x, mod_const), mod(uv.y, mod_const) ), 0.9*mod_const );
    
    
    
    float mod_const = pow(scale, float(ITER)); // eta konstanta delaet tolschinu
                                              // polos, kotoraya ZAVISIT ot ITER, POSTOYANNOY

    float w = 0.05 * mod_const; // tolschina polosok napravlennikh vdol' osey
    
    vec3 x_positive = float(int((uv.x>0.0)&&(uv.y<w)&&(uv.y>-w)))*red; // osi
    vec3 x_negative = float(int((uv.x<0.0)&&(uv.y<w)&&(uv.y>-w)))*0.2*red; 
    vec3 y_positive = float(int((uv.y>0.0)&&(uv.x<w)&&(uv.x>-w)))*(red+green);
    vec3 y_negative = float(int((uv.y<0.0)&&(uv.x<w)&&(uv.x>-w)))*0.2*(red+green);
    vec3 osi = x_positive + x_negative + y_positive + y_negative;

    vec2 xy_examp = fragCoord/vec3(u_resolution, 1.0).xy;
    vec3 col_examp = 0.5 + 0.5*cos(u_time+xy_examp.xyx+vec3(0,2,4));
     // col_ex *= .75*sin(u_time)+.5;
    //fragColor = vec4(col+col_g+koltso,1.0);
   // fragColor = vec4(col_g+koltso+koltso2+col_ex,1.0);
   
    vec3 result = col_g + // mertsaniye
                  koltso + 
                  koltso2 +
                  .5*(-osi*col_examp + osi + col_examp) //poloski + tsvetovoye resheniye
                  ;

    fragColor = vec4(result ,1.0);                             
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
