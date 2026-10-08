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
vec2 rotate(vec2 space, vec2 center, float amount){
    return vec2(cos(amount) * (space.x - center.x) + sin(amount) * (space.y - center.y),
        cos(amount) * (space.y - center.y) - sin(amount) * (space.x - center.x));
}

float reflection(inout vec2 pos, float angle){
    vec2 normal = vec2(cos(angle),sin(angle));
    float d = dot(pos, normal);
    pos -= normal*min(0.,d)*2.;
    return smoothstep(0.1,0.,abs(d));
}
vec3 cosPalette(  float t,  vec3 a,  vec3 b,  vec3 c, vec3 d ){
    return a + b*cos( 6.28318*(c*t+d) );
}

float apollonian(vec2 z){
    float s =1.;
    float PI =3.14;
    
    for(int i=0;i<10;i++){
        float f = 2. / dot(z,z);     
        z *= f; s *= f;
        
        reflection(z,-PI/2.);
        z= rotate(z,vec2(0.),PI/2.);
        z.y = 2.*fract(z.y*0.5) - 1.;
        
    }

    return (length(z)-.3)/s;
}

void mainImage( out vec4 fragColor, in vec2 fragCoord )
{	
    vec2 uv = (fragCoord.xy / vec3(u_resolution, 1.0).xy * 2.0 -1.0) 
        		* vec2(vec3(u_resolution, 1.0).x/vec3(u_resolution, 1.0).y, 1.0);
    vec2 pos = uv *1.5;
    vec4 color;
    
    vec3 a = vec3(0.2);  
    vec3 b = vec3(0.5);  
    vec3 c = vec3(3.,1.5,2.);     
     vec3 d = vec3(.1,0.17,0.23); 
    
    float cir = 1.5-length(pos);
    float t = min(cir, apollonian(pos));
    color.rgb = cosPalette(t +u_time*0.041,a,b,c,d);

    
    fragColor = color;
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
