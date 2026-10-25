"""Fetch the second Shadertoy batch through one authenticated Edge context.

The legacy POST endpoint is used deliberately: it returns the shader JSON without
depending on Chromium for the renderer.  The browser is only the authenticated
transport and asset downloader.
"""
import base64, json, time
from pathlib import Path
from playwright.sync_api import sync_playwright
from fetch_shadertoy import extract_shader_obj, find_edge_executable, collect_asset_entries, write_glsl

IDS = "3ljBDK stBSzt 3sc3WN cdsXzf sd3SD2 DtXGDn sljGRW lljyRt Nljyzh t3GXDK mtdyDs tlf3zn ftVyRK lcVXRy wXyGDR stVczd cdf3R4 dtyczc WXd3Dn msfSz8 ftjczG ft2GR1 fdScDR slBcRK 3XyXzm NtSGDD sl2yRK NtXBzB stfBRB 7llBzS fdcyR7 t33czn 7dffWf 7dVGRW 3XSSzK Ds2GWd wXfXWS WXtGWr 3ftGDX WfKGz1 cs33Dj dlVyWG dlVGzh wXcyzn mdf3Wn".split()
FETCH = """async id => { const p=new URLSearchParams(); p.set('s',JSON.stringify({shaders:[id]})); p.set('nt','1');p.set('nl','1');p.set('np','1'); const r=await fetch('https://www.shadertoy.com/shadertoy',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8'},credentials:'include',body:p.toString()}); return {status:r.status,text:await r.text()}; }"""
ASSET = """async url => { const r=await fetch(url,{credentials:'include'}); if(!r.ok)return {status:r.status}; const b=new Uint8Array(await r.arrayBuffer()); let s=''; for(let i=0;i<b.length;i+=0x8000)s+=String.fromCharCode(...b.subarray(i,i+0x8000)); return {status:r.status,b64:btoa(s)}; }"""

def main():
    root = Path('E:/__Projects/RPi-VJ/target/shadertoy-offline'); root.mkdir(parents=True, exist_ok=True)
    out = Path('E:/__Projects/RPi-VJ/target/shadertoy-downloads'); out.mkdir(parents=True, exist_ok=True)
    edge = find_edge_executable()
    with sync_playwright() as p:
        ctx = p.chromium.launch_persistent_context(str(Path('E:/__Projects/RPi-VJ/target/shadertoy-edge-profile').resolve()), executable_path=edge, headless=False, args=['--disable-blink-features=AutomationControlled'])
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        page.goto('https://www.shadertoy.com/view/f33Xzr', wait_until='domcontentloaded', timeout=90000); page.wait_for_timeout(3000)
        for n, sid in enumerate(dict.fromkeys(IDS), 1):
            r = page.evaluate(FETCH, sid)
            if r.get('status') != 200:
                print(f'FAIL {n:02d}/{len(set(IDS))} {sid}: HTTP {r.get("status")}', flush=True); time.sleep(12); continue
            try: shader = extract_shader_obj(json.loads(r['text']))
            except Exception as e: shader = None; print(f'PARSE {sid}: {e}', flush=True)
            if not shader:
                print(f'FAIL {n:02d}/{len(set(IDS))} {sid}: payload', flush=True); time.sleep(12); continue
            sd = root / sid; sd.mkdir(parents=True, exist_ok=True); (sd/'shader.json').write_text(json.dumps(shader, indent=2), encoding='utf-8')
            write_glsl(sid, shader, out)
            entries = collect_asset_entries(shader); ok = 0
            for i, ent in enumerate(entries):
                url = str(ent.get('url') or ''); name = str(ent.get('filename') or f'asset-{i}.bin')
                try: a = page.evaluate(ASSET, url)
                except Exception: a = {'status': 0}
                if a.get('status') == 200 and a.get('b64'):
                    (sd/name).write_bytes(base64.b64decode(a['b64'])); ok += 1
            (sd/'assets.json').write_text(json.dumps({'shader_id':sid,'assets':entries,'downloaded':ok}, indent=2), encoding='utf-8')
            print(f'OK {n:02d}/{len(set(IDS))} {sid}: {ok}/{len(entries)} assets', flush=True)
            time.sleep(12)
        ctx.close()
if __name__ == '__main__': main()








