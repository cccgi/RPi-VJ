#!/usr/bin/env python3
"""Build a browser-independent WebGL2 visual verification page for downloaded Shadertoy GLSL."""
import base64
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / "target" / "shadertoy-downloads"
OUT = ROOT / "target" / "shadertoy-preview"

HTML = r'''<!doctype html><meta charset="utf-8"><title>Shadertoy 31 preview</title>
<style>body{margin:0;background:#111;color:#ddd;font:12px monospace}#grid{display:grid;grid-template-columns:repeat(4,256px);gap:8px;padding:10px}.card{background:#222;padding:4px}.card canvas{width:256px;height:144px;display:block}.bad{color:#f88;white-space:pre-wrap;height:144px;overflow:auto}</style>
<div id="grid"></div><script>
const SHADERS=__SHADERS__;
const V=`#version 300 es\nin vec2 p;void main(){gl_Position=vec4(p,0.,1.);}`;
const H=`#version 300 es\n#define HW_PERFORMANCE 1\nprecision highp float;uniform vec3 iResolution;uniform float iTime,iTimeDelta,iFrameRate;uniform int iFrame;uniform vec4 iMouse;uniform float u_bass,u_mid,u_treble,u_volume,u_beat_onset;uniform sampler2D iChannel0,iChannel1,iChannel2,iChannel3;uniform vec3 iChannelResolution[4];out vec4 fragColor;`;
function compile(gl,type,src){let s=gl.createShader(type);gl.shaderSource(s,src);gl.compileShader(s);if(!gl.getShaderParameter(s,gl.COMPILE_STATUS))throw Error(gl.getShaderInfoLog(s));return s}
function program(gl,src){let p=gl.createProgram();gl.attachShader(p,compile(gl,gl.VERTEX_SHADER,V));gl.attachShader(p,compile(gl,gl.FRAGMENT_SHADER,H+src+'\nvoid main(){mainImage(fragColor,gl_FragCoord.xy);}'));gl.linkProgram(p);if(!gl.getProgramParameter(p,gl.LINK_STATUS))throw Error(gl.getProgramInfoLog(p));return p}
function run(card,src,id,assets){src=src.split(/\n\/\/ ===== Pass 1:/)[0];let c=document.createElement('canvas');c.width=512;c.height=288;card.append(c);let gl=c.getContext('webgl2');if(!gl){card.textContent='WebGL2 unavailable';return}let p;try{p=program(gl,src)}catch(e){let x=document.createElement('div');x.className='bad';x.textContent='COMPILE ERROR\n'+e;card.append(x);return}let b=gl.createBuffer();let texs=[];let px=new Uint8Array(256*256*4);for(let i=0;i<px.length;i+=4){let q=(i/4)%256,r=Math.floor(i/4/256);px[i]=q;px[i+1]=r;px[i+2]=(q^r);px[i+3]=255}gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,256,256,0,gl.RGBA,gl.UNSIGNED_BYTE,px);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);for(let unit=0;unit<4;unit++){let tx=gl.createTexture();texs.push(tx);gl.activeTexture(gl.TEXTURE0+unit);gl.bindTexture(gl.TEXTURE_2D,tx);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,256,256,0,gl.RGBA,gl.UNSIGNED_BYTE,px);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MIN_FILTER,gl.LINEAR);gl.texParameteri(gl.TEXTURE_2D,gl.TEXTURE_MAG_FILTER,gl.LINEAR);let su=gl.getUniformLocation(p,'iChannel'+unit);if(su)gl.uniform1i(su,unit)}assets.forEach(a=>{let im=new Image();im.onload=()=>{let tx=texs[a.channel|0];gl.activeTexture(gl.TEXTURE0+(a.channel|0));gl.bindTexture(gl.TEXTURE_2D,tx);gl.texImage2D(gl.TEXTURE_2D,0,gl.RGBA,gl.RGBA,gl.UNSIGNED_BYTE,im)};im.src=a.data});gl.bindBuffer(gl.ARRAY_BUFFER,b);gl.bufferData(gl.ARRAY_BUFFER,new Float32Array([-1,-1,1,-1,-1,1,1,1]),gl.STATIC_DRAW);let loc=gl.getAttribLocation(p,'p'),t0=performance.now();function frame(now){gl.viewport(0,0,c.width,c.height);gl.useProgram(p);gl.bindBuffer(gl.ARRAY_BUFFER,b);gl.enableVertexAttribArray(loc);gl.vertexAttribPointer(loc,2,gl.FLOAT,false,0,0);let t=(now-t0)/1000;for(const [n,v] of [['iResolution',[c.width,c.height,1]],['iChannelResolution[0]',[256,256,1]],['iChannelResolution[1]',[256,256,1]],['iChannelResolution[2]',[256,256,1]],['iChannelResolution[3]',[256,256,1]],['iTime',t],['iTimeDelta',.016],['iFrame',Math.floor(t*60)],['u_bass',.5+.5*Math.sin(t*2.1)],['u_mid',.5+.5*Math.sin(t*1.3)],['u_treble',.5+.5*Math.sin(t*3.7)],['u_volume',.5],['u_beat_onset',Math.max(0,Math.sin(t*2.1*3.14159))]]){let u=gl.getUniformLocation(p,n);if(u){if(Array.isArray(v))gl.uniform3fv(u,v);else if(n==='iFrame')gl.uniform1i(u,v);else gl.uniform1f(u,v)}}gl.drawArrays(gl.TRIANGLE_STRIP,0,4);requestAnimationFrame(frame)}requestAnimationFrame(frame)}
for(const s of SHADERS){let card=document.createElement('div');card.className='card';card.innerHTML='<b>'+s.id+' â€” '+s.name+'</b>';document.querySelector('#grid').append(card);run(card,s.source,s.id,s.assets||[])}
</script>'''


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    shaders = []
    for path in sorted(SOURCE.glob("*.glsl")):
        text = path.read_text(encoding="utf-8", errors="replace")
        name = next((line[8:] for line in text.splitlines() if line.startswith("// Name: ")), path.stem)
        assets = []
        manifest = ROOT / "target" / "shadertoy-offline" / path.stem / "assets.json"
        if manifest.exists():
            for item in json.loads(manifest.read_text(encoding="utf-8")).get("assets", []):
                refs = item.get("refs", [])
                channel = refs[0].get("channel") if refs else None
                asset_path = manifest.parent / str(item.get("filename", ""))
                if channel is not None and asset_path.exists():
                    mime = "image/png" if asset_path.suffix.lower() == ".png" else "image/jpeg"
                    assets.append({"channel": channel, "data": "data:%s;base64,%s" % (mime, base64.b64encode(asset_path.read_bytes()).decode("ascii"))})
        shaders.append({"id": path.stem, "name": name, "source": text, "assets": assets})
    (OUT / "index.html").write_text(HTML.replace("__SHADERS__", json.dumps(shaders)), encoding="utf-8")
    print(OUT / "index.html")


if __name__ == "__main__":
    main()



