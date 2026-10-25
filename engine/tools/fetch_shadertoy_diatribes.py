"""Discover Diatribes once, then retrieve only missing sources from its saved ID queue."""
import json
import os
import re
import sys
import tempfile
import time
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path("E:/__Projects/RPi-VJ/target")
TOOL = ROOT / "ShadertoyTool"
sys.path.insert(0, str(TOOL))
from fetch_shadertoy import browser_cookie3, extract_shader_obj, find_edge_executable, write_glsl  # noqa: E402

OFFLINE = ROOT / "shadertoy-offline"
DOWNLOADS = ROOT / "shadertoy-downloads"
STATE = ROOT / "shadertoy-diatribes"
ID_FILE = STATE / "profile_ids.json"
PROFILE = Path(tempfile.gettempdir()) / f"rpi-vj-diatribes-{os.getpid()}"
PAGES, PAGE_SIZE = 99, 8
FETCH = """async id=>{const p=new URLSearchParams();p.set('s',JSON.stringify({shaders:[id]}));p.set('nt','1');p.set('nl','1');p.set('np','1');const r=await fetch('/shadertoy',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8'},credentials:'include',body:p.toString()});return {status:r.status,text:await r.text()}}"""


def complete(sid):
    return (DOWNLOADS / f"{sid}.glsl").is_file() and (OFFLINE / sid / "shader.json").is_file()


def add_edge_cookies(context):
    if browser_cookie3 is None:
        return 0
    cookies = []
    try:
        for domain in (".shadertoy.com", "www.shadertoy.com"):
            for c in browser_cookie3.edge(domain_name=domain):
                cookies.append({"name": c.name, "value": c.value, "domain": c.domain or domain,
                                "path": c.path or "/", "secure": bool(c.secure)})
        if cookies:
            context.add_cookies(cookies)
    except Exception as error:
        print(f"EDGE_COOKIE_IMPORT_FAILED {error}", flush=True)
    return len(cookies)


def discover(page):
    ids = []
    for number in range(PAGES):
        url = ("https://www.shadertoy.com/user/diatribes" if number == 0 else
               f"https://www.shadertoy.com/user/diatribes/sort=popular&from={number * PAGE_SIZE}&num={PAGE_SIZE}")
        page.goto(url, wait_until="domcontentloaded", timeout=90000)
        page.wait_for_timeout(8000)
        for href in page.eval_on_selector_all('a[href*="/view/"]', 'els=>els.map(e=>e.href)'):
            hit = re.search(r"/view/([A-Za-z0-9]{6})", href)
            if hit and hit.group(1) not in ids:
                ids.append(hit.group(1))
        print(f"PROFILE_PAGE {number + 1}/{PAGES} ids={len(ids)}", flush=True)
    ID_FILE.write_text(json.dumps(ids, indent=2), encoding="utf-8")
    return ids


def main():
    STATE.mkdir(parents=True, exist_ok=True); OFFLINE.mkdir(parents=True, exist_ok=True); DOWNLOADS.mkdir(parents=True, exist_ok=True)
    with sync_playwright() as p:
        ctx = p.chromium.launch_persistent_context(str(PROFILE), executable_path=find_edge_executable(), headless=False,
                                                   args=["--disable-blink-features=AutomationControlled"])
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        print(f"EDGE_COOKIES_IMPORTED {add_edge_cookies(ctx)}", flush=True)
        ids = list(dict.fromkeys(json.loads(ID_FILE.read_text(encoding="utf-8")))) if ID_FILE.is_file() else discover(page)
        queue = [sid for sid in ids if not complete(sid)]
        (STATE / "incomplete_ids.json").write_text(json.dumps(queue, indent=2), encoding="utf-8")
        print(f"PROFILE_IDS {len(ids)}; PROFILE_NEW {len(queue)}", flush=True)
        if not queue:
            ctx.close(); return
        page.goto(f"https://www.shadertoy.com/view/{queue[0]}", wait_until="domcontentloaded", timeout=90000)
        page.wait_for_timeout(5000)
        try: probe = page.evaluate(FETCH, queue[0])
        except Exception: probe = {"status": 0}
        if probe.get("status") != 200:
            input("Complete any Shadertoy/Cloudflare check in Edge, then press Enter: ")
        for pos, sid in enumerate(queue, 1):
            try: result = page.evaluate(FETCH, sid)
            except Exception as error:
                print(f"FAIL {pos}/{len(queue)} {sid}: {error}", flush=True); time.sleep(15); continue
            if result.get("status") != 200:
                print(f"FAIL {pos}/{len(queue)} {sid}: HTTP {result.get('status')}", flush=True); time.sleep(12); continue
            try: shader = extract_shader_obj(json.loads(result["text"]))
            except Exception: shader = None
            if not shader:
                print(f"FAIL {pos}/{len(queue)} {sid}: payload", flush=True); time.sleep(12); continue
            folder = OFFLINE / sid; folder.mkdir(parents=True, exist_ok=True)
            (folder / "shader.json").write_text(json.dumps(shader, indent=2), encoding="utf-8")
            write_glsl(sid, shader, DOWNLOADS)
            print(f"OK {pos}/{len(queue)} {sid}", flush=True); time.sleep(12)
        ctx.close()


if __name__ == "__main__":
    main()
