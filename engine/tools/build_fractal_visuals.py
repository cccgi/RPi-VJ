#!/usr/bin/env python3
"""Build the native fractal show and a polished cyclic deep-zoom visual."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LIB = ROOT / "target" / "shadertoy-fractals"
NATIVE = ROOT / "target" / "native-13-shaders"

ZOOM_SHADER = r'''/* {"LABEL":"Mandelbrot Infinity Dive"} */
#define LOOK_ID 0
precision highp float;
uniform vec2 u_resolution;
uniform float u_time;
uniform float u_bass;
uniform float u_mid;
uniform float u_treble;
uniform float u_volume;
uniform float u_beat_onset;

vec3 palette(float t) {
#if LOOK_ID == 1
    return 0.50 + 0.48*cos(6.2831853*(t + vec3(0.03,0.10,0.19)));
#elif LOOK_ID == 2
    return 0.52 + 0.48*cos(6.2831853*(t + vec3(0.00,0.21,0.43)));
#elif LOOK_ID == 3
    return 0.50 + 0.50*cos(6.2831853*(t + vec3(0.02,0.35,0.68)));
#elif LOOK_ID == 4
    return vec3(0.10,0.40,0.55)+0.50*cos(6.2831853*(t+vec3(0.52,0.68,0.83)));
#elif LOOK_ID == 5
    return vec3(0.55,0.13,0.025)+0.70*cos(6.2831853*(t+vec3(0.03,0.13,0.19)));
#elif LOOK_ID == 6
    return 0.48 + 0.46*cos(6.2831853*(t + vec3(0.08,0.38,0.69)));
#elif LOOK_ID == 7
    return 0.52 + 0.48*cos(6.2831853*(t + vec3(0.76,0.89,0.03)));
#else
    return 0.52 + 0.48*cos(6.2831853*(t + vec3(0.00,0.31,0.67)));
#endif
}

vec2 mulc(vec2 a, vec2 b) { return vec2(a.x*b.x-a.y*b.y, a.x*b.y+a.y*b.x); }

vec3 mandel(vec2 c, float colorPhase) {
    vec2 z=vec2(0.0), dz=vec2(0.0);
    float trap=10.0;
    float n=0.0;
    for(int i=0;i<192;i++) {
        dz=2.0*mulc(z,dz)+vec2(1.0,0.0);
        z=mulc(z,z)+c;
        trap=min(trap,abs(z.x)+0.7*abs(z.y));
        n=float(i);
        if(dot(z,z)>256.0) break;
    }
    float r=length(z);
    if(r<=16.0) {
        float interiorGlow=exp(-42.0*trap);
        return vec3(0.018,0.012,0.045)+interiorGlow*vec3(0.06,0.11,0.18);
    }
    float smoothN=n+1.0-log2(max(1e-6,log2(r)));
    float de=0.5*log(max(r,1.00001))*r/max(length(dz),1e-8);
    float edgeGlow=exp(-150.0*de);
    float orbitGlow=exp(-18.0*trap);
    vec3 col=palette(0.055*smoothN+colorPhase);
    col*=0.35+0.65*smoothstep(0.0,0.12,fract(smoothN*0.20));
    col+=edgeGlow*vec3(1.1,0.50,1.35);
    col+=orbitGlow*vec3(0.05,0.28,0.55);
#if LOOK_ID == 0
    float lace=0.78+0.22*cos(42.0*atan(z.y,z.x)+12.0*trap);
    col*=lace;
#elif LOOK_ID == 1
    col=pow(max(col,vec3(0.0)),vec3(0.72))*vec3(1.08,0.94,1.12);
#elif LOOK_ID == 2
    col+=0.20*exp(-24.0*abs(fract(smoothN*0.13)-0.5))*vec3(1.0,0.58,0.20);
#elif LOOK_ID == 3
    col*=0.70+0.30*sin(72.0*atan(z.y,z.x)+smoothN*0.45);
#elif LOOK_ID == 4
    col*=vec3(0.70,1.05,1.18);
#elif LOOK_ID == 5
    col=col*vec3(1.35,0.62,0.23)+edgeGlow*vec3(0.9,0.18,0.025);
#elif LOOK_ID == 6
    col*=0.52+0.48*step(0.48,fract(smoothN*0.36+0.08*sin(atan(z.y,z.x)*8.0)));
#elif LOOK_ID == 7
    col+=0.12*exp(-14.0*trap)*vec3(0.55,0.12,0.90);
#endif
    return col;
}

void main() {
    vec2 uv=(gl_FragCoord.xy-0.5*u_resolution)/u_resolution.y;
    float cycleLength=42.0;
    float phase=mod(u_time,cycleLength)/cycleLength;
    float leg=floor(u_time/cycleLength);
    float zoomProgress=smoothstep(0.0,0.94,phase);
    float zoom=exp2(mix(0.0,15.0,zoomProgress));
    vec2 sites[4];
    sites[0]=vec2(-0.7436438870,0.1318259042);
    sites[1]=vec2(-0.1010963638,0.9562865108);
    sites[2]=vec2(-0.2351250000,0.8272150000);
    sites[3]=vec2(-0.7450000000,0.1860000000);
    int idx=int(mod(leg,4.0));
    int nextIdx=(idx+1)%4;
    float handoff=smoothstep(0.94,1.0,phase);
    float beat=u_beat_onset*0.025+u_bass*0.008;
    vec2 pan=vec2(sin(u_time*0.071),cos(u_time*0.053))*0.000025;
    vec2 c0=sites[idx]+pan+(uv/zoom)*(1.0+beat);
    float nextZoom=exp2(mix(0.0,15.0,smoothstep(0.0,0.94,handoff*0.07)));
    vec2 c1=sites[nextIdx]+pan+(uv/nextZoom)*(1.0+beat);
    float hue=u_time*0.018+u_mid*0.08+u_treble*0.025;
    vec3 color=mix(mandel(c0,hue),mandel(c1,hue),handoff);
    float vignette=1.0-0.26*smoothstep(0.25,0.76,length(uv));
    color*=vignette*(0.88+0.24*u_volume+0.13*u_beat_onset);
    color+=0.10*exp(-32.0*length(uv))*vec3(0.2,0.42,1.0);
    gl_FragColor=vec4(clamp(color,0.0,1.0),1.0);
}
'''


def main():
    (LIB / "shaders").mkdir(parents=True, exist_ok=True)
    looks = [
        ("lacework", "Mandelbrot • Lacework glow"),
        ("pastel-waves", "Mandelbrot • Pastel Waves"),
        ("easter-eggs", "Mandelbrot • Easter Egg orbit traps"),
        ("rainbow-strings", "Mandelbrot • Rainbow Strings"),
        ("setting-sail", "Mandelbrot • Setting Sail"),
        ("fire-crab", "Mandelbrot • Fire Crab bloom"),
        ("zebrafish", "Mandelbrot • Zebrafish bands"),
        ("infinite-spiral", "Mandelbrot • Infinite Spiral"),
    ]

    index_path = LIB / "index.json"
    index = json.loads(index_path.read_text(encoding="utf-8")) if index_path.exists() else {"shaders": []}
    items = []
    for row in index.get("shaders", []):
        sid = row["id"]
        wrapper = NATIVE / f"{sid}.fs"
        if wrapper.exists():
            items.append({"label": f"{sid} {row.get('name', sid)}", "path": f"../native-13-shaders/{wrapper.name}", "blend": "Additive", "opacity": 0.9})
    zoom_items = []
    for look_id, (slug, label) in enumerate(looks):
        name = f"mandelbrot-{slug}.fs"
        (LIB / "shaders" / name).write_text(ZOOM_SHADER.replace("#define LOOK_ID 0", f"#define LOOK_ID {look_id}"), encoding="utf-8")
        item = {"label": label + " • animated deep zoom", "path": f"shaders/{name}", "blend": "Additive", "opacity": 1.0}
        items.append(item)
        zoom_items.append(item)
    show = {"version": 2, "bpm": 128, "audio_device": None, "clips": [], "pages": [], "shaders": items}
    (LIB / "show.json").write_text(json.dumps(show, indent=2), encoding="utf-8")
    zoom_show = {"version": 2, "bpm": 128, "audio_device": None, "clips": [], "pages": [], "shaders": zoom_items}
    (LIB / "infinity-show.json").write_text(json.dumps(zoom_show, indent=2), encoding="utf-8")
    print(f"FRACTAL_SHOW {len(items)} entries")


if __name__ == "__main__":
    main()
