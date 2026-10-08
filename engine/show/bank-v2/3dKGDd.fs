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
//Consts
#define PI  3.14159265359
#define TAU (PI*2.)
#define PHI 1.61803399
#define deg2rad 0.01745329251
#define SQRT2 (sqrt(2.))
#define SQRT3 (sqrt(3.))
#define BIG 1e8
#define ETA 1e-3
#define eqf(a,b) ( abs((a)-(b))<ETA )

//Aliases
#define fc (gl_FragCoord.xy)
#define res (vec3(u_resolution, 1.0).xy)
#define ires ivec2(vec3(u_resolution, 1.0).xy)
#define aspect (res.x/res.y)
#define asp aspect
#define aspinv (1./aspect)
#define vec1 float
#define ivec1 int
#define uvec1 uint
#define len length
#define lerp mix
#define norm normalize
#define sat saturate
#define sats saturate_signed
#define smooth(x) smoothstep(0.,1.,x)
#define time float(u_time)
#define mouse ((iMouse.xy-res/2.)/(res*2.))
#define mouse_ang (mouse*TAU)
#define tex texture
#define ass(pred,ret) if(!(pred)) return (ret);

//normalized map to signed
//[ 0,1]->[-1,1]
vec1 nmaps(vec1 x){ return x*2.-1.; }
vec2 nmaps(vec2 x){ return x*2.-1.; }
vec3 nmaps(vec3 x){ return x*2.-1.; }
vec4 nmaps(vec4 x){ return x*2.-1.; }
//normalized map to unsigned
//[-1,1]->[ 0,1]
vec1 nmapu(vec1 x){ return x*.5+.5; }
vec2 nmapu(vec2 x){ return x*.5+.5; }
vec3 nmapu(vec3 x){ return x*.5+.5; }
vec4 nmapu(vec4 x){ return x*.5+.5; }

float quant(float x, float q){ return floor(x*q)/q; }

float maxv( vec2 a){ return                 max(a.x,a.y)  ;}
float maxv( vec3 a){ return         max(a.z,max(a.x,a.y)) ;}
float maxv( vec4 a){ return max(a.w,max(a.z,max(a.x,a.y)));}
float minv( vec2 a){ return                 min(a.x,a.y)  ;}
float minv( vec3 a){ return         min(a.z,min(a.x,a.y)) ;}
float minv( vec4 a){ return min(a.w,min(a.z,min(a.x,a.y)));}
  int maxv(ivec2 a){ return                 max(a.x,a.y)  ;}
  int maxv(ivec3 a){ return         max(a.z,max(a.x,a.y)) ;}
  int maxv(ivec4 a){ return max(a.w,max(a.z,max(a.x,a.y)));}
  int minv(ivec2 a){ return                 min(a.x,a.y)  ;}
  int minv(ivec3 a){ return         min(a.z,min(a.x,a.y)) ;}
  int minv(ivec4 a){ return min(a.w,min(a.z,min(a.x,a.y)));}
//copyright khlorghaal, most rights reserved

#define SNAKE(x,y,w) (x+y*w)
#define ANTI_SNAKE(x,y,w) ivec2 //TODO LOL (x%w+)

#define CELLDIM 64
//vector type? why the fuck not
ivec2 type(ivec2 uv){ return uv/CELLDIM; }
bool border(ivec2 uv){
    return 
      uv.x%CELLDIM==0
    ||uv.y%CELLDIM==0;
}

float tonemap(float x){ return 1.-(1./(1.+x)); }


void mainImage(out vec4 col, vec2 fg){
    ivec2 p= ivec2(fg);

    //zoom
    int Z= 1;
    //bool BIGSCREEN= ires.y/CELLDIM > 6*4;
    //int Z= BIGSCREEN? 4:2;//fuck these constants
    ///PURGED
    //do not touch these
    //the duck will not help you
    //only math will help you
    //and im not math enough right now
    //perhaps my future self will find sufficient math to this
    //   future self: no.
    p= p-(p%Z);//quantize
    p/= Z;//scale
    //zoom test //col.rgb= vec3(p.x)/32.; return;

    ivec2 T= type(p);
    if(border(p)){
        col= vec4(0.);
        return;
    }

    p%= CELLDIM;//gridding
    p= abs(p-CELLDIM/2);//symmetry

    float t= mod(u_time*30.,256.);
    t= abs(t-128.);//loop
    int q= int(t);

    float lum= 0.;
    const bool A= true;
    int i;
    #define CAS(T,LAM) case T: i= LAM; break;
    #define BIGDICKMACROITERATOR( FOOF,\
    I0,I1,I2,I3,I4,I5,I6,I7,I8,I9,I10,I11,I12,I13,I14)\
    FOOF( 0,(I0))\
    FOOF( 1,(I1))\
    FOOF( 2,(I2))\
    FOOF( 3,(I3))\
    FOOF( 4,(I4))\
    FOOF( 5,(I5))\
    FOOF( 6,(I6))\
    FOOF( 7,(I7))\
    FOOF( 8,(I8))\
    FOOF( 9,(I9))\
    FOOF(10,(I10))\
    FOOF(11,(I11))\
    FOOF(12,(I12))\
    FOOF(13,(I13))\
    FOOF(14,(I14))
    
    #define B(OP) p.x OP p.y
    #define C(O,C,K) (p.x O p.y) C (p.x K p.y)
    switch(T.x){
        BIGDICKMACROITERATOR( CAS,
            B(+),
            B(-),
            B(*),
            B(/),
            B(%),
            B(&),
            B(|),
            B(^),
            C(&,+,|),
            C(&,-,|),
            C(&,+,^),
            C(&,-,^),
            ((p.x%p.y)%4)*4,
            0,0
        )
        default:i=i;
    }
        
    #undef B
    #define B(OP) i OP q
    switch(T.y){
        BIGDICKMACROITERATOR( CAS,
            B(+),
            B(-),
            B(*),
            B(/),
            B(%),
            B(&),
            B(|),
            B(^),
            i%(q+i),
            0,
            0,
            0,
            0,
            0,0
        )
        default:i=i;
    }
    /*
    last time i tried this bad things happened
    also glsl does not support macro operator ##
    this is high power intellectual retardation
    */
        
    //luminance based tonemapping
    //coloration is applied after tonemap
    lum= float(i);
    lum/= 128.;
    lum= abs(lum);
    // 0->0 inf->1
    //reciprocal approximate to exponential decay in terms of asytmptotes
    lum= tonemap(lum);
    
    //neednt inverse srgb because tonemap
    
    //coloration
    col.rgb= vec3(lum);
    if(i<0)
        col.rg*= 0.;
    
    float bigv= float(abs(i));
    col.g+= tonemap(bigv/float(0x400))*1.;//big=>green
    
    col= sqrt(col);
    
    col.a= 1.;
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
