#!/usr/bin/env python3
"""Convert verified Shadertoy Image passes to the Rust renderer's input profile."""
import json
import re
from pathlib import Path

IDS = "ttKGDt tsXBzS tlVGDt sX3Xzn 733Xzn 3sySRK tdG3Rd Ml2XRD 3sc3z4 wtcSzN MsKcRh 4tcGDr 3lsSzf NslGRN MdX3zr mtyGWy lsl3RH MdXyzX lscczl 4dfGzs 3l23Rh ltffzl XsX3RB ldl3W8 XsBXWt 4sfGWX MsXGWr 4tjSDt ldlcRf ftt3R7 ll2GD3".split()
ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "target" / "native-13-shaders"
HEAD = '''/* {"LABEL":"Shadertoy native"} */
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
'''

def main():
    OUT.mkdir(parents=True, exist_ok=True)
    shaders = []
    for sid in [p.stem for p in sorted((ROOT / 'target' / 'shadertoy-downloads').glob('*.glsl'))]:
        metadata = ROOT / "target" / "shadertoy-offline" / sid / "shader.json"
        if metadata.exists():
            document = json.loads(metadata.read_text(encoding="utf-8"))
            code = "\n".join([p.get("code", "") for p in document.get("renderpass", []) if p.get("type") == "common" or p.get("name") == "Common"] + [document["renderpass"][0]["code"]])
            label = document.get("info", {}).get("name", sid)
            if sid in {"MsKcRh", "NdcXR8"}: code = document["renderpass"][0]["code"]
        else:
            raw = (ROOT / "target" / "shadertoy-downloads" / f"{sid}.glsl").read_text(encoding="utf-8")
            image_part = raw.split("===== Pass 0:", 1)[1]
            image = image_part.split("\n", 1)[1].split("\n// ===== Pass 1:", 1)[0]
            common = ""
            if "===== Pass 1: Common" in raw:
                common = raw.split("===== Pass 1: Common", 1)[1].split("\n", 1)[1]
            code = common + "\n" + image
            label = next((line[8:] for line in raw.splitlines() if line.startswith("// Name: ")), sid)
        code = re.sub(r"\biResolution\b", "vec3(u_resolution, 1.0)", code)
        code = re.sub(r"\biTimeDelta\b", "TIMEDELTA", code)
        code = re.sub(r"\biTime\b", "u_time", code)
        code = re.sub(r"\biFrame\b", "FRAMEINDEX", code)
        code = code.replace("texture(iChannel0, u*d +o)", "texture(iChannel0, u.xy*d +o.xy)")
        code = code.replace("vec3 vec3(u_resolution, 1.0)", "vec3 iResolution")
        code = code.replace("texture(iChannel0, s)", "texture(iChannel0, s.xy)")
        code = code.replace("texture(iChannel0,(p)/18.)", "texture(iChannel0,(p).xy/18.)")
        code = code.replace("T,a/180.)", "T,a.xy/180.)")
        code = code.replace("texture(iChannel0, (c.y*t.x > 3. ? t.zxy : t.yzx)-3.)", "texture(iChannel0, (c.y*t.x > 3. ? t.zxy : t.yzx).xy-3.)")
        code = code.replace("texture(iChannel0, dir *", "texture(iChannel0, dir.xy *")
        code = code.replace("texture(iChannel0, s * 0.003)", "texture(iChannel0, s.xy * 0.003)")
        code = code.replace("texture(iChannel0, s * 0.014 + vec3(0.618))", "texture(iChannel0, s.xy * 0.014 + vec2(0.618))")
        code = code.replace("texture(iChannel0, s * 0.047 + 2.0 * vec3(0.618))", "texture(iChannel0, s.xy * 0.047 + 2.0 * vec2(0.618))")
        code = code.replace("T,ceil(p*8.)/16.)", "T,ceil(p.xy*8.)/16.)")
        code = code.replace("texture( iChannel0, A*p )", "texture( iChannel0, (A*p).xy )")
        code = code.replace("texture(iChannel0,p)", "texture(iChannel0,p.xy)")
        code = code.replace("texture(iChannel0,tan(O.rgb=round(p)))", "texture(iChannel0,tan(O.rgb=round(p)).xy)")
        code = code.replace("texture(iChannel0,(p)/1.9)", "texture(iChannel0,(p).xy/1.9)")
        code = code.replace("texture(iChannel0,(p)/19.)", "texture(iChannel0,(p).xy/19.)")
        code = code.replace("texture(iChannel0,d)", "texture(iChannel0,d.xy)")
        code = code.replace("texture(iChannel0,(p+t)*.2)", "texture(iChannel0,(p+t).xy*.2)")
        code = code.replace("texture(iChannel0, dir)", "texture(iChannel0, dir.xy)")
        code = code.replace("texture(iChannel0, a/3e2)", "texture(iChannel0, a.xy/3e2)")
        code = code.replace("texture(iChannel1, rf)", "texture(iChannel1, rf.xy)")
        code = code.replace("texture(iChannel0, (p+t)*.2)", "texture(iChannel0,(p+t).xy*.2)")
        code = code.replace("texture(iChannel0,Position*2.+u_time*Speed*.2)", "texture(iChannel0,(Position*2.+u_time*Speed*.2).xy)")
        code = code.replace("texture(iChannel0,P)", "texture(iChannel0,P.xy)")
        code = code.replace("texture(iChannel0,P*vec3(.5,1,.5))", "texture(iChannel0,(P*vec3(.5,1,.5)).xy)")
        code = code.replace("texture(iChannel0,P*5.1)", "texture(iChannel0,(P*5.1).xy)")
        code = code.replace("texture(iChannel0,uvw*6.0)", "texture(iChannel0,(uvw*6.0).xy)")
        code = code.replace("texture(iChannel0,dir.xzy)", "texture(iChannel0,dir.xzy.xy)")
        code = code.replace("texture(iChannel3, tcD, lod)", "texture(iChannel3, tcD.xy, lod)")
        code = code.replace("fbm1(iChannel0,120.0*uvw)", "fbm1(iChannel0,120.0*uvw.xy)")
        code = code.replace("texture(iChannel2,dir.xzy)", "texture(iChannel2,dir.xzy.xy)")
        code = re.sub(r"textureLod\(iChannel3\s*,\s*tcD\s*,\s*lod\)", "textureLod(iChannel3, tcD.xy, lod)", code)
        code = re.sub(r"texture\(iChannel0,\s*(a\s*=\s*ceil\(p\s*\+=\s*min\(a\s*=\s*mod\(-p,sign\(d\)\)/d\+3e-8,\s*min\(a\.y,a\.z\)\)\.x\*d\)/3e2)\)", r"texture(iChannel0, (\1).xy)", code)
        code = code.replace("vec2 vec3(u_resolution, 1.0)", "vec2 iResolution")
        if sid == "s3KGWc": code = re.sub(r"\bTIME\b", "STIME", code)
        if sid == "NdcXR8" and "//[259]" in code: code = code.split("//[259]", 1)[0]
        if sid in {"MsKcRh", "NdcXR8"} and code.count("void mainImage") > 1:
            parts = code.split("void mainImage")
            code = parts[0] + "void mainImage" + parts[1]
        path = OUT / f"{sid}.fs"
        wrapper = """
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
"""
        path.write_text(HEAD + code + wrapper, encoding="utf-8")
        shaders.append({"label": f"{sid} {label}", "path": path.name, "blend": "Additive", "opacity": 0.9})
    (OUT / "show.json").write_text(json.dumps({"version": 2, "bpm": 128, "audio_device": None, "clips": [], "pages": [], "shaders": shaders}, indent=2), encoding="utf-8")

if __name__ == "__main__":
    main()







