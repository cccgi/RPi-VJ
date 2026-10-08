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
/*
This documentation refers to the GitHub Repo: <link to be added>

(PT - Br) Documentação em português começa na linha 8.
(En) English documentation starts on line 30.

(PT - Br)
Documentação em português:
Este é um shader voltado para a renderização matemática do Carpete de Sierpinski, um fractal definido
por uma regra de substituição geométrica. Foi descoberto pelo mesmo matemático que descobriu o mais
conhecido Triângulo de Sierpinski.

Para a construção desse shader foram utilizadas diversas fontes:
- The Art of Code: https://www.youtube.com/channel/UCcAlTqd9zID6aNX3TzwxJXg
- The Coding Train: https://www.youtube.com/channel/UCvjgXvBlbQiydffZU7m1_aw
- Sebastian Lague: https://www.youtube.com/user/Cercopithecan

Este shader está sob a licença MIT.
Cheque "License.txt" para detalhes sobre a licensa.

Instruções para compilar:
- Entre no site https://www.shadertoy.com
- No canto superior direito, clique em "new". Você será redirecionado para uma página com uma caixa
de texto onde voce pode escrever e uma tela.
- Apague todo o conteúdo da caixa de texto.
- Copie este código e cole-o diretamente na ciaxa de texto.
- Se nada mudar, aperte "alt" + "enter" e o shader deve compilar.

(En)
English documentation:
This shader targets to achieve a mathematical render of Sierpinski's Carpet, a fractal defined by a rule
of geometrical substituion. It was discovered by the same mathemathician that discovered the more famous
Sierpinski's Triangle fractal.

For the creation of this shader, several resources were used:
- The Art of Code: https://www.youtube.com/channel/UCcAlTqd9zID6aNX3TzwxJXg
- The Coding Train: https://www.youtube.com/channel/UCvjgXvBlbQiydffZU7m1_aw
- Sebastian Lague: https://www.youtube.com/user/Cercopithecan

This shader in under the MIT license.
Refer to "LICENSE.txt" for the details of the license.

Instructions to compile:
- Follow this url: https://www.shadertoy.com.
- On the upper right portion of the screen, click on the "new" button. You will be redirected to a page
with a text box you can write on and a screen.
- Delete all the text on the text box.
- Copy this code and paste it on the text box.
- If nothing happnes, press "alt" + "enter" and the shader should compile.
*/

#define PI 3.141592653589793238

// Folds the 2d space to generate the fractal and returns the distance to it
float sierpinskiCarpet (inout vec2 uv, int steps) {
  float scale = 4.0; // Scale of the UV
  uv *= 4.0; // Scales the UV to make the fractal fit on the screen
  uv = abs (uv); // Creates a reflection line in each (X and Y) axis

  for (int i = 0; i < steps; ++i) {
    scale *= 3.0;
    uv *= 3.0;
    uv = abs (uv);
    uv *= 1.0 - step (uv.x, 3.0) * step (uv.y, 3.0);
    uv -= vec2 (0, 3);
    uv = abs (uv);
    uv -= vec2 (0, 3);
    uv = abs (uv);
    uv -= vec2 (3, 0);
    uv = abs (uv);
    uv -= vec2 (3, 0);
    uv = abs (uv);
  }

  float d = length (uv - vec2 (min (uv, vec2 (1)))) / scale;
  uv /= scale;
  return d;
}

// Main function of the shader
void mainImage (out vec4 fragColor, in vec2 fragCoord) {
  vec2 uv = 2.0 * (fragCoord - 0.5 * vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y; // Normalized pixel coordinates (from 0 to 1)
  vec2 uv2 = uv; // Creates a copy of the uvs for coloring

  vec3 col = vec3 (0.0); // Color to be drawn on the screen

  int recursionCount = 0 + int (mod (u_time, 12.0) * 0.5); // Number of iterations of the fractal (increases with time)

  float d = sierpinskiCarpet (uv, recursionCount); // Distance to the fractal

  // Coloring the fractal
  float lineSmoothness = 3.0 / vec3(u_resolution, 1.0).y; // Smoothness of the line (higher number = smoother, lower numbers = sharper)
  float offset = 0.5; // Offset for the blending of the colors in the middle
  // Red channel
  float r = smoothstep (lineSmoothness, 0.0, d) * 0.5 * (uv2.x * 0.5 + 0.5 + offset); // Generates a gradient of red on the positive x axis
  col.r = r;
  // Blue channel
  float b = smoothstep (lineSmoothness, 0.0, d) * 0.5 * (uv2.y * 0.5 + 0.5 + offset); // Generates a gradient of blue on the positive y axis
  col.b = b;
  // Green channel
  float g = smoothstep (lineSmoothness, 0.0, d) * 0.5 * (-uv2.x * 0.5 + 0.5 + offset); // Generates a gradient of green on the negative x axis
  col.g = g;

  // Drawing the axis
  lineSmoothness = 2.0 / vec3(u_resolution, 1.0).y; // Descreases line smoothness for sharper / thinner lines
  col.rb += smoothstep (lineSmoothness, 0.0, length (uv.y)); // x axis
  col.bg += smoothstep (lineSmoothness, 0.0, length (uv.x)); // y axis

  fragColor = vec4 (col.rgb, 1.0); // Outputs the result color to the screen
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
