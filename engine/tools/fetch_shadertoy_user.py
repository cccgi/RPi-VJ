"""Resumable Shadertoy user-profile importer with source and asset recovery."""
import argparse, base64, json, os, re, sys, tempfile, time
from pathlib import Path
from playwright.sync_api import sync_playwright

TARGET = Path("E:/__Projects/RPi-VJ/target")
TOOL = TARGET / "ShadertoyTool"
sys.path.insert(0, str(TOOL))
from fetch_shadertoy import browser_cookie3, collect_asset_entries, extract_shader_obj, find_edge_executable, write_glsl  # noqa: E402

OFFLINE, DOWNLOADS = TARGET / "shadertoy-offline", TARGET / "shadertoy-downloads"
FETCH = """async id=>{const p=new URLSearchParams();p.set('s',JSON.stringify({shaders:[id]}));p.set('nt','1');p.set('nl','1');p.set('np','1');const r=await fetch('/shadertoy',{method:'POST',headers:{'Content-Type':'application/x-www-form-urlencoded; charset=UTF-8'},credentials:'include',body:p.toString()});return {status:r.status,text:await r.text()}}"""
ASSET = """async url=>{const r=await fetch(url,{credentials:'include'});if(!r.ok)return {status:r.status};const b=new Uint8Array(await r.arrayBuffer());let s='';for(let i=0;i<b.length;i+=0x8000)s+=String.fromCharCode(...b.subarray(i,i+0x8000));return {status:r.status,b64:btoa(s)}}"""


def user_state(name):
    return TARGET / "shadertoy-users" / name.lower()


URL_ID_RE = re.compile(r"shadertoy\.com/view/([A-Za-z0-9]{6})(?![A-Za-z0-9])", re.I)
BARE_ID_RE = re.compile(r"^[A-Za-z0-9]{6}$")


def read_ids(values, list_file):
    """Return ordered unique IDs from URLs, bare IDs, and an optional text file.

    A file may have one item per line, but accepting pasted prose and URLs makes
    it safe to use the same importer for a curator's URL list.  Invalid tokens
    are reported rather than silently becoming a network request.
    """
    raw = list(values or [])
    if list_file:
        source = Path(list_file).read_text(encoding="utf-8-sig", errors="replace")
        try:
            parsed = json.loads(source)
            if isinstance(parsed, list):
                raw.extend(map(str, parsed))
            else:
                raw.append(source)
        except ValueError:
            raw.append(source)
    ids, rejected = [], []
    for item in raw:
        for line in str(item).splitlines():
            value = line.strip()
            if not value or value.startswith("#"):
                continue
            matches = URL_ID_RE.findall(value)
            if not matches and BARE_ID_RE.fullmatch(value):
                matches = [value]
            if not matches:
                rejected.append(value[:120])
            for sid in matches:
                if sid not in ids:
                    ids.append(sid)
    if rejected:
        print(f"ID_INPUT_IGNORED {len(rejected)} malformed item(s): {rejected[:3]}", flush=True)
    return ids


def complete(sid):
    folder, source = OFFLINE / sid, DOWNLOADS / f"{sid}.glsl"
    manifest = folder / "assets.json"
    if not source.is_file() or not (folder / "shader.json").is_file() or not manifest.is_file():
        return False
    try: assets = json.loads(manifest.read_text(encoding="utf-8")).get("assets", [])
    except (OSError, ValueError): return False
    return all((folder / str(a.get("filename") or "")).is_file() for a in assets)


def source_complete(sid):
    return (DOWNLOADS / f"{sid}.glsl").is_file() and (OFFLINE / sid / "shader.json").is_file()


def import_cookies(ctx):
    if browser_cookie3 is None: return 0
    rows = []
    try:
        for domain in (".shadertoy.com", "www.shadertoy.com"):
            for c in browser_cookie3.edge(domain_name=domain):
                rows.append({"name":c.name,"value":c.value,"domain":c.domain or domain,
                             "path":c.path or "/","secure":bool(c.secure)})
        if rows: ctx.add_cookies(rows)
    except Exception as error: print(f"EDGE_COOKIE_IMPORT_FAILED {error}", flush=True)
    return len(rows)


def discover(page, user, pages, state):
    ids, empty = [], 0
    for n in range(pages):
        url = (f"https://www.shadertoy.com/user/{user}" if n == 0 else
               f"https://www.shadertoy.com/user/{user}/sort=popular&from={n * 8}&num=8")
        page.goto(url, wait_until="domcontentloaded", timeout=90000); page.wait_for_timeout(8000)
        links = page.eval_on_selector_all('a[href*="/view/"]', 'els=>els.map(e=>e.href)')
        before = len(ids)
        for href in links:
            hit = re.search(r"/view/([A-Za-z0-9]{6})", href)
            if hit and hit.group(1) not in ids: ids.append(hit.group(1))
        empty = empty + 1 if len(ids) == before else 0
        print(f"PROFILE_PAGE {n+1}/{pages} ids={len(ids)}", flush=True)
        if n > 0 and empty >= 2: break
    (state / "ids.json").write_text(json.dumps(ids, indent=2), encoding="utf-8")
    return ids


def save_assets(page, sid, shader):
    folder = OFFLINE / sid; folder.mkdir(parents=True, exist_ok=True)
    entries, downloaded = collect_asset_entries(shader), 0
    for index, item in enumerate(entries):
        name = str(item.get("filename") or f"asset-{index}.bin")
        dest = folder / name
        if dest.is_file(): downloaded += 1; continue
        try: result = page.evaluate(ASSET, str(item.get("url") or ""))
        except Exception: result = {"status": 0}
        if result.get("status") == 200 and result.get("b64"):
            dest.write_bytes(base64.b64decode(result["b64"])); downloaded += 1
    (folder / "assets.json").write_text(json.dumps({"shader_id":sid,"assets":entries,"downloaded":downloaded}, indent=2), encoding="utf-8")
    return downloaded, len(entries)


def recover_from_403(page, sid, challenge_wait):
    """Wait for the browser's normal challenge flow, then retry the same ID."""
    print(f"HTTP_403_CHECKPOINT {sid}: refreshing page before retry", flush=True)
    try:
        page.reload(wait_until="domcontentloaded", timeout=90000)
        page.wait_for_timeout(challenge_wait * 1000)
        print(f"HTTP_403_PAGE url={page.url} title={page.title()!r}", flush=True)
    except Exception as error:
        print(f"HTTP_403_REFRESH_ERROR {error}", flush=True)
    print(f"HTTP_403_WAITED {challenge_wait}s; retrying {sid}", flush=True)
    try:
        return page.evaluate(FETCH, sid)
    except Exception as error:
        print(f"HTTP_403_RETRY_ERROR {error}", flush=True)
        return {"status": 0}


def prime_session(page, sid, challenge_wait, attempts):
    """Wait through transient challenge/rate-limit responses without stdin."""
    for attempt in range(1, attempts + 1):
        try:
            if attempt == 1:
                page.goto(f"https://www.shadertoy.com/view/{sid}", wait_until="domcontentloaded", timeout=90000)
            else:
                page.reload(wait_until="domcontentloaded", timeout=90000)
            page.wait_for_timeout(challenge_wait * attempt * 1000)
            print(f"SESSION_PAGE attempt={attempt}/{attempts} url={page.url} title={page.title()!r}", flush=True)
            probe = page.evaluate(FETCH, sid)
        except Exception as error:
            print(f"SESSION_PROBE_ERROR attempt={attempt}/{attempts} {error}", flush=True)
            probe = {"status": 0}
        status = probe.get("status")
        print(f"SESSION_PROBE_STATUS {status}", flush=True)
        if status == 200:
            return True
        if attempt < attempts:
            print(f"SESSION_BACKOFF {challenge_wait * attempt}s before retry", flush=True)
    return False


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("user", nargs="?", help="Shadertoy account name; omit when using --id or --ids-file")
    source = ap.add_mutually_exclusive_group()
    source.add_argument("--ids-file", type=Path,
                        help="text file containing Shadertoy URLs and/or six-character IDs")
    source.add_argument("--id", action="append", default=[],
                        help="one Shadertoy URL or six-character ID; repeat as needed")
    ap.add_argument("--name", help="stable batch name for --id/--ids-file state (default: file stem or manual)")
    ap.add_argument("--max-pages", type=int, default=200)
    ap.add_argument("--refresh-ids", action="store_true", help="browse profile again even when ids.json exists")
    ap.add_argument("--delay", type=float, default=12)
    ap.add_argument("--checkpoint-every", type=int, default=150,
                    help="refresh after this many successful records; 0 disables")
    ap.add_argument("--challenge-wait", type=int, default=25,
                    help="seconds to wait after a session refresh before retrying a 403")
    ap.add_argument("--prime-attempts", type=int, default=6,
                    help="automatic attempts to establish a usable session before preserving the queue and exiting")
    args = ap.parse_args()
    direct_ids = read_ids(args.id, args.ids_file)
    if not args.user and not direct_ids:
        ap.error("supply a profile user, --id, or --ids-file")
    if args.user and direct_ids:
        ap.error("choose either a profile user or a direct ID list")
    state_name = (args.user or args.name or (args.ids_file.stem if args.ids_file else "manual"))
    state = user_state(state_name); state.mkdir(parents=True, exist_ok=True)
    OFFLINE.mkdir(parents=True, exist_ok=True); DOWNLOADS.mkdir(parents=True, exist_ok=True)
    ids_file = state / "ids.json"; profile = Path(tempfile.gettempdir()) / f"rpi-vj-user-{os.getpid()}"
    with sync_playwright() as p:
        ctx = p.chromium.launch_persistent_context(str(profile), executable_path=find_edge_executable(), headless=False,
                                                   args=["--disable-blink-features=AutomationControlled"])
        page = ctx.pages[0] if ctx.pages else ctx.new_page(); print(f"EDGE_COOKIES_IMPORTED {import_cookies(ctx)}", flush=True)
        legacy_ids = TARGET / "shadertoy-diatribes" / "profile_ids.json"
        if direct_ids:
            ids = direct_ids
            ids_file.write_text(json.dumps(ids, indent=2), encoding="utf-8")
            print(f"DIRECT_IDS {len(ids)}; STATE {state.name}", flush=True)
        elif args.refresh_ids:
            ids = discover(page, args.user, args.max_pages, state)
        elif ids_file.is_file():
            ids = json.loads(ids_file.read_text(encoding="utf-8"))
        elif args.user.lower() == "diatribes" and legacy_ids.is_file():
            ids = json.loads(legacy_ids.read_text(encoding="utf-8"))
            ids_file.write_text(json.dumps(ids, indent=2), encoding="utf-8")
            print(f"ID_LIST_MIGRATED {len(ids)} from legacy Diatribes queue", flush=True)
        else:
            ids = discover(page, args.user, args.max_pages, state)
        source_missing = [sid for sid in dict.fromkeys(ids) if not source_complete(sid)]
        asset_incomplete = [sid for sid in dict.fromkeys(ids) if source_complete(sid) and not complete(sid)]
        queue = source_missing + asset_incomplete
        (state / "incomplete_ids.json").write_text(json.dumps(queue, indent=2), encoding="utf-8")
        print(f"USER_IDS {len(ids)}; SOURCE_MISSING {len(source_missing)}; "
              f"ASSET_INCOMPLETE {len(asset_incomplete)}; RETRY_QUEUE {len(queue)}", flush=True)
        if not queue: ctx.close(); return
        print(f"PRIMING_SESSION {queue[0]}", flush=True)
        if not prime_session(page, queue[0], args.challenge_wait, args.prime_attempts):
            print("SESSION_UNAVAILABLE; queue preserved for the next run", flush=True)
            ctx.close(); return
        completed = 0
        for pos, sid in enumerate(queue, 1):
            try: result = page.evaluate(FETCH, sid)
            except Exception as error: print(f"FAIL {pos}/{len(queue)} {sid}: {error}", flush=True); time.sleep(args.delay); continue
            if result.get("status") == 403:
                result = recover_from_403(page, sid, args.challenge_wait)
            if result.get("status") != 200:
                print(f"FAIL {pos}/{len(queue)} {sid}: HTTP {result.get('status')}", flush=True)
                time.sleep(args.delay); continue
            try: shader = extract_shader_obj(json.loads(result["text"]))
            except Exception: shader = None
            if not shader: print(f"FAIL {pos}/{len(queue)} {sid}: payload", flush=True); time.sleep(args.delay); continue
            folder = OFFLINE / sid; folder.mkdir(parents=True, exist_ok=True)
            (folder / "shader.json").write_text(json.dumps(shader, indent=2), encoding="utf-8")
            write_glsl(sid, shader, DOWNLOADS); got, total = save_assets(page, sid, shader)
            completed += 1
            print(f"OK {pos}/{len(queue)} {sid}: {got}/{total} assets", flush=True)
            if args.checkpoint_every and completed % args.checkpoint_every == 0 and pos < len(queue):
                print(f"CHECKPOINT {completed}: refreshing the verified page", flush=True)
                try:
                    page.reload(wait_until="domcontentloaded", timeout=90000)
                    page.wait_for_timeout(args.challenge_wait * 1000)
                    print(f"CHECKPOINT_PAGE url={page.url} title={page.title()!r}", flush=True)
                except Exception as error:
                    print(f"CHECKPOINT_REFRESH_ERROR {error}", flush=True)
                print("CHECKPOINT_COMPLETE; continuing queue", flush=True)
            time.sleep(args.delay)
        ctx.close()


if __name__ == "__main__": main()
