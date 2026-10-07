#!/usr/bin/env python3
"""Crawl a paginated Shadertoy search into a dedicated library.

The browser is used only as an authenticated transport. Rendering remains native
Rust.  The crawler is intentionally a separate process so it can be started
after the long profile crawl has completed.
"""
import base64
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from playwright.sync_api import sync_playwright

TOOL_ROOT = Path(__file__).resolve().parents[2] / "target" / "ShadertoyTool"
sys.path.insert(0, str(TOOL_ROOT))
from fetch_shadertoy import (  # noqa: E402
    browser_cookie3,
    collect_asset_entries,
    extract_shader_obj,
    find_edge_executable,
    write_glsl,
)

ROOT = Path("E:/__Projects/RPi-VJ/target/shadertoy-offline")
OUT = Path("E:/__Projects/RPi-VJ/target/shadertoy-downloads")
FRACTALS = Path("E:/__Projects/RPi-VJ/target/shadertoy-fractals")
EDGE_PROFILE = Path(os.environ.get(
    "SHADERTOY_EDGE_PROFILE",
    str(Path(tempfile.gettempdir()) / f"shadertoy-edge-profile-run-{os.getpid()}")
))
PAGES = 72
PAGE_SIZE = 12
QUERY = "fractals"

FETCH = """async id=>{const p=new URLSearchParams();p.set('s',JSON.stringify({shaders:[id]}));p.set('nt','1');p.set('nl','1');p.set('np','1');const r=await fetch('https://www.shadertoy.com/shadertoy',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8'},credentials:'include',body:p.toString()});return {status:r.status,text:await r.text()}}"""
ASSET = """async url=>{const r=await fetch(url,{credentials:'include'});if(!r.ok)return {status:r.status};const b=new Uint8Array(await r.arrayBuffer());let s='';for(let i=0;i<b.length;i+=0x8000)s+=String.fromCharCode(...b.subarray(i,i+0x8000));return {status:r.status,b64:btoa(s)}}"""


def search_urls(page_no: int):
    offset = page_no * PAGE_SIZE
    # Current Shadertoy uses results?query=...; the second form keeps this
    # crawler compatible with older deployments that expose /search.
    yield f"https://www.shadertoy.com/results?query={QUERY}&from={offset}&num={PAGE_SIZE}"
    yield f"https://www.shadertoy.com/search?query={QUERY}&from={offset}&num={PAGE_SIZE}"


def main():
    ROOT.mkdir(parents=True, exist_ok=True)
    OUT.mkdir(parents=True, exist_ok=True)
    FRACTALS.mkdir(parents=True, exist_ok=True)
    id_file = FRACTALS / "search_ids.json"
    with sync_playwright() as p:
        ctx = p.chromium.launch_persistent_context(
            str(EDGE_PROFILE.resolve()),
            executable_path=find_edge_executable(), headless=False,
            args=["--disable-blink-features=AutomationControlled"],
        )
        page = ctx.pages[0] if ctx.pages else ctx.new_page()
        if browser_cookie3 is not None:
            try:
                cookies = []
                for domain in (".shadertoy.com", "www.shadertoy.com"):
                    for cookie in browser_cookie3.edge(domain_name=domain):
                        cookies.append({"name": cookie.name, "value": cookie.value,
                                        "domain": cookie.domain or domain,
                                        "path": cookie.path or "/", "secure": bool(cookie.secure)})
                if cookies:
                    ctx.add_cookies(cookies)
                    print(f"EDGE_COOKIES_IMPORTED {len(cookies)}", flush=True)
            except Exception as error:
                print(f"EDGE_COOKIE_IMPORT_FAILED {error}", flush=True)
        if id_file.is_file():
            ids = list(dict.fromkeys(json.loads(id_file.read_text(encoding="utf-8"))))
            print(f"ID_LIST_REUSED {len(ids)} (no search-page browsing)", flush=True)
        else:
            ids = []
            for page_no in range(PAGES):
                found = []
                for url in search_urls(page_no):
                    try:
                        page.goto(url, wait_until="domcontentloaded", timeout=90000)
                        page.wait_for_timeout(8000)
                        found = page.eval_on_selector_all(
                            'a[href*="/view/"]', 'els=>els.map(e=>e.href)'
                        )
                    except Exception:
                        continue
                    if found:
                        break
                if not found:
                    for _ in range(12):
                        page.wait_for_timeout(5000)
                        found = page.eval_on_selector_all(
                            'a[href*="/view/"]', 'els=>els.map(e=>e.href)'
                        )
                        if found:
                            break
                for href in found:
                    match = re.search(r"/view/([A-Za-z0-9]{6})", href)
                    if match and match.group(1) not in ids:
                        ids.append(match.group(1))
                print(f"SEARCH_PAGE {page_no + 1}/{PAGES} ids={len(ids)}", flush=True)
            if "ldGGRG" not in ids:
                ids.insert(0, "ldGGRG")
            id_file.write_text(json.dumps(ids, indent=2), encoding="utf-8")
            print(f"ID_LIST_SAVED {len(ids)} (future runs skip search pages)", flush=True)
        print(f"FRACTAL_IDS {len(ids)}", flush=True)

        existing = {x.stem for x in OUT.glob("*.glsl")}
        ids = [sid for sid in ids if sid not in existing]
        print(f"FRACTAL_NEW {len(ids)}", flush=True)
        if ids:
            first = ids[0]
            page.goto(f"https://www.shadertoy.com/view/{first}", wait_until="domcontentloaded", timeout=90000)
            page.wait_for_timeout(5000)
            print(f"SESSION_READY url={page.url} title={page.title()!r}", flush=True)
            try:
                probe = page.evaluate(FETCH, first)
            except Exception:
                probe = {"status": 0}
            if probe.get("status") != 200:
                input("Complete any Shadertoy/Cloudflare check in the Edge window, then press Enter to retry: ")
        for n, sid in enumerate(ids, 1):
            try:
                result = page.evaluate(FETCH, sid)
            except Exception as error:
                # A Cloudflare challenge or transient browser-network reset
                # must not discard the remaining persisted ID queue.
                print(f"FAIL {n}/{len(ids)} {sid}: fetch exception {error}", flush=True)
                page.wait_for_timeout(15000)
                continue
            if result.get("status") != 200:
                print(f"FAIL {n}/{len(ids)} {sid}: HTTP {result.get('status')}", flush=True)
                time.sleep(12)
                continue
            try:
                shader = extract_shader_obj(json.loads(result["text"]))
            except Exception:
                shader = None
            if not shader:
                print(f"FAIL {n}/{len(ids)} {sid}: payload", flush=True)
                time.sleep(12)
                continue
            sd = ROOT / sid
            sd.mkdir(parents=True, exist_ok=True)
            (sd / "shader.json").write_text(json.dumps(shader, indent=2), encoding="utf-8")
            write_glsl(sid, shader, OUT)
            entries = collect_asset_entries(shader)
            downloaded = 0
            for i, entry in enumerate(entries):
                try:
                    asset = page.evaluate(ASSET, str(entry.get("url") or ""))
                except Exception:
                    asset = {"status": 0}
                if asset.get("status") == 200 and asset.get("b64"):
                    name = str(entry.get("filename") or f"asset-{i}.bin")
                    (sd / name).write_bytes(base64.b64decode(asset["b64"]))
                    downloaded += 1
            (sd / "assets.json").write_text(
                json.dumps({"shader_id": sid, "assets": entries, "downloaded": downloaded}, indent=2),
                encoding="utf-8",
            )
            print(f"OK {n}/{len(ids)} {sid}: {downloaded}/{len(entries)} assets", flush=True)
            time.sleep(12)
        ctx.close()

    # Turn the crawl into a self-contained native library once all sources are
    # present: classify fractals, generate Rust-renderer wrappers, then publish
    # a dedicated manifest that includes the cyclic Mandelbrot zoom piece.
    for tool in ("organize_fractal_library.py", "make_native_shadertoy.py", "build_fractal_visuals.py"):
        subprocess.run([sys.executable, str(Path(__file__).with_name(tool))], check=True)


if __name__ == "__main__":
    main()
