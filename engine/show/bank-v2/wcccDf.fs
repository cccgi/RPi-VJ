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
const float PI = 3.1415;

// shapes
const int CIRCLE = 0;
const int ROUNDED_SQUARE = 1;
const int REGULAR_POLYGON = 2;                          // use NB_SIDES_SHAPE to choose the number of sides of the polygon
const int FLOWER = 3;                                   // use NB_SIDES_SHAPE to choose the number of petals of the flower
const int SPIRAL = 4;                                   // use NB_SIDES_SHAPE to choose the number of spirals
const int STAR = 5;                                     // use NB_SIDES_SHAPE to choose the number of branches of the star

// Modify these consts to change the behaviour of the fractal

const bool USE_FRACTAL = true;

const int FRACTAL_SHAPE = FLOWER;                       // shape of the fractal
const int NB_SIDES_SHAPE = 6;                           // number of sides of the shape (used only for REGULAR_SHAPE)
const float SHAPE_ROTATION = 0.;                        // initial rotation of the shape in degrees
const float SHAPE_ROTATION_SPEED = 10.0;                // the speed at which the shape rotates
const float WHOLE_SHAPE_FREQUENCY = 2.;                 // the frequency at which the whole shape appears
const float THICKNESS = 1.;                             // thickness of the lines of the fractal (weird things happen when this is not an int and external shape is activated)
const float DENSITY = 1.5;                              // the density of the fractal
const int FRACTAL_DEPTH = 3;                            // the depth of the fractal (has to be more than 1)
const float SPEED = 1.;                                 // the speed at which the fractals appear/disappear

const int COLOR_SHAPE = CIRCLE;                         // shape of the color
const int NB_SIDES_COLOR_SHAPE = 8;                     // number of sides of the color shape (used only for REGULAR_SHAPE)
const float COLOR_SHAPE_ROTATION = 0.;                  // initial rotation of the color shape in degrees
const float COLOR_SHAPE_ROTATION_SPEED = 10.0;          // the speed at which the color shape rotates
const float COLOR_SHAPE_FREQUENCY = 0.36;               // the frequency at which the color changes according to the color shape
const float COLOR_SPEED = 0.5;                          // the speed at which the color changes
const float DEPTH_COLOR = 50.;                          // how the color changes with the depth of the fractal

const bool BOUNCE = false;                              // whether the shape bounces or not
const float BOUNCE_HEIGHT = 1.0;                        // height of the bounce in percentage of the viewport height

const int EXTERNAL_SHAPE = CIRCLE;                      // external shape of the fractal (activate USE_EXTERNAL_SHAPE to use this)
const int NB_SIDES_EXTERNAL_SHAPE = 4;                  // number of sides of the external shape (used only for REGULAR_SHAPE)
const float EXTERNAL_SHAPE_ROTATION = 0.;               // initial rotation of the external shape in degrees
const float EXTERNAL_SHAPE_ROTATION_SPEED = 10.0;       // the speed at which the external shape rotates
const bool USE_EXTERNAL_SHAPE = true;                   // whether or not the fractal is wrapped inside an external shape


//https://iquilezles.org/articles/palettes/
vec3 palette(float t) 
{
    vec3 a = vec3(0.5, 0.5, 0.5);
    vec3 b = vec3(0.5, 0.5, 0.5);
    vec3 c = vec3(0.5, 0.5, 0.5);
    vec3 d = vec3(0.25, 0.5, 0.75);
    
    return a + b - cos(2. * PI * (c-t+d));
}


float shape(vec2 vec, int shape, int nb_sides, float rotation)
{
    switch(shape)
    {
        case CIRCLE:
            return length(vec);
            
        case ROUNDED_SQUARE:
            return pow(abs(pow(vec.x, 3.)) + abs(pow(vec.y, 3.)), 1./3.);
            
        case REGULAR_POLYGON:
            float theta = (rotation/180.) * PI;
            vec2 u = vec2(sin(theta), cos(theta));
            float curr_max = vec.x * u.x + vec.y * u.y;
            for (int i = 0; i < nb_sides; i++)
            {
                theta += 2.*PI/float(nb_sides);
                u = vec2(sin(theta), cos(theta));
                curr_max = max(curr_max, vec.x * u.x + vec.y * u.y);
            }
            return curr_max;
        
        case FLOWER:
            rotation = (rotation/180.) * PI - PI;
            float angle = atan(vec.y, vec.x) - rotation;
            float radius = length(vec);

            float sector = 2.0 * PI / float(nb_sides);
            float localAngle = mod(angle + PI, sector) - 0.5 * sector;
            
            float t = clamp(abs(localAngle) / (0.5 * sector), 0.0, 1.0);
            float targetR = mix(1., 0.75, t);

            return radius - targetR;
            
        case SPIRAL:
            rotation = (rotation/180.) * PI - PI;
            
            angle = atan(vec.y, vec.x) - rotation;
            radius = length(vec);

            sector = 2.0 * PI / float(nb_sides);
            float k = floor((angle + PI) / sector);
            localAngle = angle - k * sector;
            
            t = abs(localAngle - 0.5 * sector) / (0.5 * sector);
            targetR = mix(1., 0.5, t);

            return radius - targetR;
            
        case STAR:
            rotation = (rotation/180.) * PI - PI;
            
            angle = atan(vec.y, vec.x) - rotation;
            radius = length(vec);

            sector = PI / float(nb_sides);

            localAngle = mod(angle + sector, 2.0 * sector) - sector;

            k = cos(sector);
            targetR = 1. * (k / cos(localAngle));

            return radius - targetR;
    }
}

vec2 xy2uv(vec2 xy)
{
    return (xy * 2. - vec3(u_resolution, 1.0).xy) / vec3(u_resolution, 1.0).y;
}


void mainImage( out vec4 fragColor, in vec2 fragCoord )
{
    vec2 uv = xy2uv(fragCoord);
    if (iMouse.xy != vec2(0., 0.))
    {
        uv -= xy2uv(iMouse.xy);  // move with mouse
    }
    uv += (vec2(0., -BOUNCE_HEIGHT * 2. * abs(sin(u_time))) + vec2(0., 1.0)) * float(BOUNCE);  // bounce
    vec2 uv0 = uv;
    
    vec3 color = vec3(0., 0., 0.);
    float d;
    
    for (int i = 0; i < FRACTAL_DEPTH; i++) 
    {
        if (USE_FRACTAL)
        {
            uv = fract(uv * DENSITY) - 0.5;
        }
        
        d = shape(uv, FRACTAL_SHAPE, NB_SIDES_SHAPE, SHAPE_ROTATION + SHAPE_ROTATION_SPEED * u_time);  // fractal shape
        float externalRotation = EXTERNAL_SHAPE_ROTATION + EXTERNAL_SHAPE_ROTATION_SPEED * u_time;
        d *= exp(-(shape(uv0, EXTERNAL_SHAPE, NB_SIDES_EXTERNAL_SHAPE, externalRotation)) * float(USE_EXTERNAL_SHAPE));  // external shape
        d = sin(WHOLE_SHAPE_FREQUENCY*d * PI + u_time * SPEED);  // expansion
        d = pow(d, THICKNESS); 
        d = 0.1 / d;  
        
        float colorRotation = COLOR_SHAPE_ROTATION + COLOR_SHAPE_ROTATION_SPEED * u_time;
        float colorShapeFactor = shape(uv0, COLOR_SHAPE, NB_SIDES_COLOR_SHAPE, colorRotation) * COLOR_SHAPE_FREQUENCY;  // color shape
        color += palette(colorShapeFactor + float(i*i) * DEPTH_COLOR * DEPTH_COLOR + u_time * COLOR_SPEED) * d;
    }


    fragColor = vec4(color, 1);
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
