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
// Curve Fractals? || v.0.0.1 / version 1!
/*



ChangeLog:
 â€¢ Added Reference-To-English Translation.
 â€¢ Fractal Window Updated IN Beta 1.



         \
          \
==========)>============-------------------------
          /
         /



           "Youtube": Kino Bacaltos,
           "Gmail": My Name ( ^ ) + @gmail.com,
           "Minecraft: Pocket - Edition" Xbox Live: Imsure1200q,
           ("@xawgamer If YOU, xawgamer, Are Visiting This Site, Read My Email"),




          
*/
//Function mainImage: Display The Shader Around The Function's Parrenthesis On The Screen
void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    //Declare A 3-component vector.
    vec3 p = vec3(1.0,2.0,1.5);
    //Declare The Position ( Instead Of A Vertex, Vertices Will Act With This Like gl_Position.)
	vec2 uv = fragCoord.xy / vec3(u_resolution, 1.0).xy;
    //Add Noise Background To vec3 UV.
	vec4 fragColorpart = vec4(uv,0.5+0.5*sin(u_time),1.0);
    //Add Colors To Front, Light, And Back ( of the noise editors )
    vec4 light = vec4(1.0,1.0,1.0,1.0);
    vec4 glasscolor = vec4(1.0,1.0,1.0,0.5);
    vec4 fragGlassRefract = refract(glasscolor,fragColorpart,2.0);
    vec4 fragGlassReflect = reflect(glasscolor,light);
    //Make Our First New Color By Mixing Our Reflect & Refract Vectors.
    vec4 newCol = mix(fragGlassReflect,fragGlassRefract,2.0);
    //Add A Vector That Displays More Than One Screen Of The Main Shader.
    vec4 fragColSeg = fract(newCol/10.0);
    
    //Adjustment:
    
    //Start Making Curve Shave Using A Float.
    float fragColorSphere = dot(fragColSeg.yyx,fragColSeg.yyx);
    //Make A Vector To Blend It,
    float blend = float(fragColorpart.yyxy)/(u_time*(360.0/36.0)*10.0);
    //To Stretch It,
    float stretch = blend/pow(blend,blend);
    //And To Combine When Stretched!
    float combine = normalize(dot(p,p));
    //Add A Mixing 4-component Vector That Then Mixes The Adjustment Tools.
    vec4 spherex = vec4(
        //First Mix Input
        mix(
            //Put Second Mix Input. Add fragColorSphere And Stretch It By 3.1 secs.
            mix(fragColorSphere, stretch, 3.1),
            //Add Another Second Mix Input. Make It Blend The Stretch Function So That Combine Will Repeat
            //It.
            mix(blend, mix(
                combine, stretch, 2.0
            ),
                //Make Durability TO 2.0.
                2.0),
            //And 3.0.
            3.0)
        );
    //You May Not Expect Why I Put "vec4(spherex * spherex);" In Vector Sphere, But, As You Can See,
    //If You Take Out " * spherex" In The Vector "sphere", Then A Curve Won't Slightly Appear.
    //It Will Possibly Look Like A Piece Of Circle. So I Added The " * spherex" To The Vector "sphere".
    vec4 sphere = vec4(spherex * spherex);
    //Finally, Make The Color Of Our Fragment Equal TO The 4-component vector "sphere".
    fragColor = sphere;
    //End The Void.
}
//THE END
//X   X    DDDD    ||
// X X     D   D  ||||
//  X      D   D   ||
// X X     D   D
//X   X    DDDD    ()
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
