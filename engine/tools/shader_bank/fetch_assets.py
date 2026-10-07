#!/usr/bin/env python3
"""Inventory, validate and fetch the Shadertoy input assets used by the shader bank.

Reads the raw Shadertoy metadata in target/shadertoy-offline/<id>/shader.json for
every bank shader (the .fs stems in target/native-13-shaders plus the fractal
library), works out which distinct texture / cubemap / volume files the Image,
Buffer and Cubemap passes bind, and fills one canonical store:

    target/shader-bank/assets/<shadertoy hash filename>
    target/shader-bank/assets/manifest.json

Re-runnable and incremental: a file already in the store that still validates
is never fetched again. Files that were downloaded per shader by the earlier
crawls (target/shadertoy-offline/<id>/<hash>.<ext>) are validated and copied in
before anything is requested from the network.

Fetch methods (--method):
    auto     plain HTTPS first; if the site refuses the first request, switch
             to the browser session for the rest (default)
    http     plain HTTPS only
    browser  browser session only
    none     no network; inventory, harvest local copies, validate, manifest

The browser session is the method that worked for the original crawl
(target/ShadertoyTool/batch_persistent_edge.py): a separate Edge window with its
own profile directory, one real /view/<id> page load, then same-origin fetch()
from that page. The profile directory lives outside the repository (see
--profile-dir). No cookies, keys or account data are read or written by default.

Needs Pillow (image decoding). The browser method additionally needs Playwright
and an installed Microsoft Edge.
"""
from __future__ import annotations

import argparse
import base64
import datetime as _dt
import hashlib
import json
import os
import re
import shutil
import struct
import sys
import tempfile
import time
import urllib.error
import urllib.request
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
TARGET = ROOT / "target"
OFFLINE = TARGET / "shadertoy-offline"
BANK_FS_DIR = TARGET / "native-13-shaders"
FRACTAL_DIR = TARGET / "shadertoy-fractals"
STORE = TARGET / "shader-bank" / "assets"
MANIFEST = STORE / "manifest.json"
SITE = "https://www.shadertoy.com"
USER_AGENT = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"
)

# Passes whose inputs become GPU sampler bindings. Sound passes are ignored
# (the appliance has no Shadertoy sound synthesis); Common has no inputs.
VISUAL_PASS_TYPES = ("image", "buffer", "cubemap")
# Input types backed by a static file the renderer must upload as a texture.
TEXTURE_TYPES = ("texture", "cubemap", "volume")
# Static files that are not textures; fetched only with --media, never blocking.
MEDIA_TYPES = ("music", "video")
# Inputs produced at run time; nothing to download.
RUNTIME_TYPES = ("buffer", "cubemap_buffer", "keyboard", "webcam", "mic", "musicstream")
CUBE_FACES = ("+X", "-X", "+Y", "-Y", "+Z", "-Z")
ID_RE = re.compile(r"^[A-Za-z0-9]{6}$")

ASSET_JS = """async url => {
  const r = await fetch(url, {credentials: 'include'});
  if (!r.ok) return {status: r.status};
  const b = new Uint8Array(await r.arrayBuffer());
  let s = '';
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return {status: r.status, b64: btoa(s)};
}"""


# --------------------------------------------------------------------------- bank

def bank_ids() -> dict[str, list[str]]:
    """Shader id -> the bank locations that contain it."""
    found: dict[str, list[str]] = {}
    if BANK_FS_DIR.is_dir():
        for path in sorted(BANK_FS_DIR.glob("*.fs")):
            if ID_RE.match(path.stem):
                found.setdefault(path.stem, []).append("native-13-shaders")
    index = FRACTAL_DIR / "index.json"
    if index.is_file():
        try:
            for row in json.loads(index.read_text(encoding="utf-8")).get("shaders", []):
                sid = str(row.get("id") or "")
                if ID_RE.match(sid):
                    found.setdefault(sid, []).append("shadertoy-fractals")
        except (ValueError, OSError):
            pass
    sources = FRACTAL_DIR / "sources"
    if sources.is_dir():
        for path in sorted(sources.glob("*.glsl")):
            if ID_RE.match(path.stem) and "shadertoy-fractals" not in found.get(path.stem, []):
                found.setdefault(path.stem, []).append("shadertoy-fractals")
    return dict(sorted(found.items()))


def _flag(value) -> bool:
    return str(value).strip().lower() == "true"


def norm_sampler(raw) -> dict:
    raw = raw if isinstance(raw, dict) else {}
    return {
        "filter": str(raw.get("filter") or "linear"),
        "wrap": str(raw.get("wrap") or "clamp"),
        "vflip": _flag(raw.get("vflip")),
        "srgb": _flag(raw.get("srgb")),
        "internal": str(raw.get("internal") or "byte"),
    }


def classify(inp: dict) -> tuple[str, str]:
    """(kind, source path) for one input. kind refines Shadertoy's `type`."""
    kind = str(inp.get("type") or inp.get("ctype") or "")
    src = str(inp.get("filepath") or inp.get("src") or "")
    if kind == "cubemap" and "/media/previz/" in src:
        # Output of the shader's own "Cube A" pass, not a file.
        return "cubemap_buffer", "/media/previz/" + src.rsplit("/", 1)[-1]
    return kind, src


def cube_face_names(name: str) -> list[str]:
    """Shadertoy loads face 0 from <hash>.<ext> and faces 1..5 from <hash>_<n>.<ext>."""
    stem, dot, ext = name.rpartition(".")
    return [name] + [f"{stem}_{n}{dot}{ext}" for n in range(1, 6)]


# --------------------------------------------------------------------- validation

def sha256_of(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def looks_like_text_error(head: bytes) -> bool:
    sample = head[:512].lstrip().lower()
    return sample.startswith((b"<!doctype", b"<html", b"<?xml", b"bad request", b"error", b"{"))


def validate_file(path: Path, kind: str) -> dict:
    """Decode the file fully. Returns {"ok": bool, "reason"|properties...}."""
    try:
        size = path.stat().st_size
    except OSError:
        return {"ok": False, "reason": "absent"}
    if size == 0:
        return {"ok": False, "reason": "empty file"}
    with path.open("rb") as handle:
        head = handle.read(512)
    if looks_like_text_error(head):
        return {"ok": False, "reason": "HTML or text error page, not media"}
    ext = path.suffix.lower()
    info: dict = {"ok": True, "bytes": size}
    if kind in ("texture", "cubemap"):
        try:
            from PIL import Image
        except ImportError:
            raise SystemExit("Pillow is required to validate images: pip install pillow")
        expected = {".png": "PNG", ".jpg": "JPEG", ".jpeg": "JPEG"}.get(ext)
        try:
            with Image.open(path) as image:
                image.verify()
            with Image.open(path) as image:
                image.load()  # full decode: catches truncated files
                fmt, mode = image.format, image.mode
                width, height = image.size
                bands = len(image.getbands())
        except Exception as error:  # Pillow raises many types for bad data
            return {"ok": False, "reason": f"does not decode: {error}"}
        if expected and fmt != expected:
            return {"ok": False, "reason": f"decodes as {fmt}, extension says {expected}"}
        info.update(format=fmt, width=width, height=height, channels=bands, mode=mode,
                    power_of_two=(width & (width - 1) == 0 and height & (height - 1) == 0))
        return info
    if kind == "volume":
        # Shadertoy volume: "BIN\0", u32 w,h,d, u8 channels, u8 layout, u16 format
        # (0 = 8-bit unorm, 10 = 32-bit float), then tightly packed texels.
        if size < 20 or head[:4] != b"BIN\x00":
            return {"ok": False, "reason": "missing BIN volume header"}
        width, height, depth = struct.unpack_from("<III", head, 4)
        channels, layout = head[16], head[17]
        (bin_format,) = struct.unpack_from("<H", head, 18)
        texel = {0: 1, 10: 4}.get(bin_format)
        if texel is None or not 1 <= channels <= 4:
            return {"ok": False, "reason": f"unknown volume format {bin_format}/{channels}"}
        expected_size = 20 + width * height * depth * channels * texel
        if size != expected_size:
            return {"ok": False, "reason": f"truncated volume: {size} bytes, header implies {expected_size}"}
        info.update(format="shadertoy-bin", width=width, height=height, depth=depth, channels=channels,
                    component="u8" if texel == 1 else "f32", layout=layout, header_bytes=20)
        return info
    magic = {
        ".mp3": (b"ID3", b"\xff\xfb", b"\xff\xfa", b"\xff\xf3", b"\xff\xf2"),
        ".ogv": (b"OggS",), ".ogg": (b"OggS",), ".webm": (b"\x1a\x45\xdf\xa3",),
        ".mp4": (),
    }.get(ext)
    if magic and not head.startswith(magic):
        return {"ok": False, "reason": f"not a {ext} container"}
    info.update(format=ext.lstrip("."))
    return info


# ----------------------------------------------------------------------- fetchers

class HttpFetcher:
    name = "http"

    def get(self, url: str) -> tuple[int, bytes, str]:
        request = urllib.request.Request(url, headers={
            "User-Agent": USER_AGENT, "Referer": SITE + "/", "Accept": "*/*",
        })
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                return response.status, response.read(), ""
        except urllib.error.HTTPError as error:
            return error.code, b"", error.read(200).decode("utf-8", "replace").strip()
        except (urllib.error.URLError, OSError) as error:
            return 0, b"", str(error)

    def close(self) -> None:
        pass


class BrowserFetcher:
    """A separate Edge window with its own profile; requests run inside a real page."""
    name = "browser"

    def __init__(self, profile_dir: Path, prime_id: str, interactive: bool, import_cookies: bool):
        try:
            from playwright.sync_api import sync_playwright
        except ImportError:
            raise SystemExit("The browser method needs Playwright: pip install playwright")
        edge = next((str(p) for p in (
            Path(os.environ.get("ProgramFiles(x86)", "C:/Program Files (x86)")) / "Microsoft/Edge/Application/msedge.exe",
            Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "Microsoft/Edge/Application/msedge.exe",
        ) if p.is_file()), None)
        profile_dir.mkdir(parents=True, exist_ok=True)
        try:
            profile_dir.resolve().relative_to(ROOT.resolve())
        except ValueError:
            pass
        else:
            if TARGET.resolve() not in profile_dir.resolve().parents:
                raise SystemExit("Refusing a browser profile inside the tracked repository tree")
        self._pw = sync_playwright().start()
        options = dict(headless=False, args=["--disable-blink-features=AutomationControlled"])
        if edge:
            options["executable_path"] = edge
        try:
            self._ctx = self._pw.chromium.launch_persistent_context(str(profile_dir), **options)
        except Exception as error:
            self._pw.stop()
            raise SystemExit(f"Could not start the browser session (locked profile?): {error}")
        if import_cookies:
            self._import_cookies()
        self._page = self._ctx.pages[0] if self._ctx.pages else self._ctx.new_page()
        print(f"[browser] priming with {SITE}/view/{prime_id}", flush=True)
        self._page.goto(f"{SITE}/view/{prime_id}", wait_until="domcontentloaded", timeout=90000)
        for _ in range(6):  # allow an automatic (non-interactive) check to settle
            self._page.wait_for_timeout(5000)
            title = self._page.title()
            if "just a moment" not in title.lower() and "shadertoy" in self._page.url:
                break
        print(f"[browser] page title {self._page.title()!r}", flush=True)
        if "just a moment" in self._page.title().lower():
            if interactive and sys.stdin and sys.stdin.isatty():
                input("Complete the Cloudflare check in the Edge window, then press Enter: ")
            else:
                self.close()
                raise SystemExit(
                    "Cloudflare is showing an interactive check. Re-run from a terminal with "
                    "--method browser --interactive and complete it by hand."
                )

    def _import_cookies(self) -> None:
        try:
            import browser_cookie3
            cookies = []
            for domain in (".shadertoy.com", "www.shadertoy.com"):
                for cookie in browser_cookie3.edge(domain_name=domain):
                    cookies.append({"name": cookie.name, "value": cookie.value,
                                    "domain": cookie.domain or domain, "path": cookie.path or "/",
                                    "secure": bool(cookie.secure)})
            if cookies:
                self._ctx.add_cookies(cookies)
            print(f"[browser] imported {len(cookies)} Shadertoy cookies into the session profile", flush=True)
        except Exception as error:
            print(f"[browser] cookie import unavailable: {error}", flush=True)

    def get(self, url: str) -> tuple[int, bytes, str]:
        try:
            result = self._page.evaluate(ASSET_JS, url)
        except Exception as error:
            return 0, b"", f"page fetch failed: {str(error).splitlines()[0]}"
        status = int(result.get("status") or 0)
        if status == 200 and result.get("b64") is not None:
            return 200, base64.b64decode(result["b64"]), ""
        return status, b"", ""

    def close(self) -> None:
        for step in (lambda: self._ctx.close(), lambda: self._pw.stop()):
            try:
                step()
            except Exception:
                pass


# -------------------------------------------------------------------------- model

def build_inventory(ids: dict[str, list[str]]):
    """Returns (assets by source path, per-shader bindings, non-file inputs)."""
    assets: dict[str, dict] = {}
    shaders: dict[str, dict] = {}
    for sid, where in ids.items():
        meta_path = OFFLINE / sid / "shader.json"
        entry: dict = {"bank": where, "passes": []}
        shaders[sid] = entry
        try:
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            entry["status"] = "no_metadata"
            entry["note"] = "no target/shadertoy-offline/<id>/shader.json; inputs unknown"
            entry["channels_referenced_in_source"] = channels_in_source(sid)
            continue
        entry["name"] = str((meta.get("info") or {}).get("name") or "")
        passes = [p for p in meta.get("renderpass") or [] if isinstance(p, dict)]
        producer = {}
        for index, rp in enumerate(passes):
            for out in rp.get("outputs") or []:
                producer[str(out.get("id"))] = {"pass_index": index, "pass": str(rp.get("name") or ""),
                                                "pass_type": str(rp.get("type") or "")}
        for index, rp in enumerate(passes):
            ptype = str(rp.get("type") or "")
            if ptype not in VISUAL_PASS_TYPES:
                continue
            pname = str(rp.get("name") or "") or ptype.capitalize()
            channels = []
            for inp in sorted(rp.get("inputs") or [], key=lambda i: int(i.get("channel") or 0)):
                kind, src = classify(inp)
                sampler = norm_sampler(inp.get("sampler"))
                binding = {"channel": int(inp.get("channel") or 0), "type": kind, "sampler": sampler}
                if kind in TEXTURE_TYPES or kind in MEDIA_TYPES:
                    binding["asset"] = src
                    asset = assets.setdefault(src, {"source_path": src, "type": kind, "used_by": []})
                    if asset["type"] != kind:  # same file bound as two kinds; keep the texture kind
                        asset.setdefault("also_bound_as", sorted({kind} - {asset["type"]}))
                    asset["used_by"].append({"shader": sid, "pass": pname, "pass_type": ptype,
                                             "channel": binding["channel"], "sampler": sampler})
                elif kind in ("buffer", "cubemap_buffer"):
                    binding["from_pass"] = producer.get(str(inp.get("id")))
                    binding["input_id"] = str(inp.get("id") or "")
                else:
                    binding["source"] = src
                channels.append(binding)
            entry["passes"].append({
                "index": index, "name": pname, "type": ptype,
                "output_ids": [str(o.get("id")) for o in rp.get("outputs") or []],
                "channels": channels,
            })
    return dict(sorted(assets.items())), shaders


def channels_in_source(sid: str) -> list[int]:
    for path in (TARGET / "shadertoy-downloads" / f"{sid}.glsl", BANK_FS_DIR / f"{sid}.fs"):
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        text = re.sub(r"uniform[^;]*;", "", text)
        return sorted({int(n) for n in re.findall(r"\biChannel([0-3])\b", text)})
    return []


def asset_files(asset: dict) -> list[str]:
    name = asset["source_path"].rsplit("/", 1)[-1]
    return cube_face_names(name) if asset["type"] == "cubemap" else [name]


def asset_url(asset: dict, filename: str) -> str:
    src = asset["source_path"]
    base = src if src.startswith("http") else SITE + src
    return base.rsplit("/", 1)[0] + "/" + filename


def local_copies() -> dict[str, list[Path]]:
    """filename -> earlier per-shader downloads, so nothing is fetched twice."""
    index: dict[str, list[Path]] = {}
    if OFFLINE.is_dir():
        for folder in OFFLINE.iterdir():
            if not folder.is_dir():
                continue
            for path in folder.iterdir():
                if path.name not in ("shader.json", "assets.json") and path.is_file():
                    index.setdefault(path.name, []).append(path)
    return index


def write_atomic(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    handle, temp = tempfile.mkstemp(dir=str(path.parent), prefix=".part-")
    with os.fdopen(handle, "wb") as out:
        out.write(data)
    os.replace(temp, path)


# --------------------------------------------------------------------------- main

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--method", choices=("auto", "http", "browser", "none"), default="auto")
    parser.add_argument("--media", action="store_true", help="also fetch music/video files (not textures)")
    parser.add_argument("--delay", type=float, default=1.5, help="seconds between requests (default 1.5)")
    parser.add_argument("--profile-dir", default=os.environ.get("SHADER_BANK_BROWSER_PROFILE", ""),
                        help="browser session profile directory; default is a folder under the system temp dir")
    parser.add_argument("--interactive", action="store_true", help="wait for a manual Cloudflare check")
    parser.add_argument("--import-cookies", action="store_true",
                        help="copy Shadertoy cookies from Edge into the session profile (needs browser_cookie3)")
    parser.add_argument("--max-failures", type=int, default=6, help="stop fetching after this many consecutive failures")
    args = parser.parse_args()

    ids = bank_ids()
    assets, shaders = build_inventory(ids)
    STORE.mkdir(parents=True, exist_ok=True)
    previous = {}
    try:
        for row in json.loads(MANIFEST.read_text(encoding="utf-8")).get("assets", []):
            for item in row.get("files", []):
                previous[item["file"]] = item
    except (OSError, ValueError, KeyError):
        pass

    copies = local_copies()
    state: dict[str, dict] = {}  # filename -> validation result + origin
    queue: list[tuple[dict, str]] = []
    for asset in assets.values():
        wanted = asset["type"] in TEXTURE_TYPES or args.media
        for name in asset_files(asset):
            dest = STORE / name
            result = validate_file(dest, asset["type"])
            if result["ok"]:
                result["origin"] = previous.get(name, {}).get("origin", "store")
            else:
                for candidate in copies.get(name, []):
                    check = validate_file(candidate, asset["type"])
                    if check["ok"]:
                        shutil.copyfile(candidate, dest)
                        result = validate_file(dest, asset["type"])
                        result["origin"] = "copied from " + candidate.relative_to(ROOT).as_posix()
                        break
            if not result["ok"]:
                old = previous.get(name, {})
                for key in ("http_status", "error"):
                    if key in old:
                        result[key] = old[key]
                if wanted:
                    queue.append((asset, name))
            state[name] = result

    print(f"bank shaders {len(ids)}; distinct assets {len(assets)}; "
          f"files valid locally {sum(1 for r in state.values() if r['ok'])}/{len(state)}; to fetch {len(queue)}")

    fetcher = None
    method_used: Counter = Counter()
    if queue and args.method != "none":
        profile = Path(args.profile_dir) if args.profile_dir else Path(tempfile.gettempdir()) / "rpi-vj-shadertoy-session"
        prime = next((u["shader"] for a, _ in queue for u in a["used_by"]), next(iter(ids)))

        def browser():
            return BrowserFetcher(profile, prime, args.interactive, args.import_cookies)

        fetcher = browser() if args.method == "browser" else HttpFetcher()
        failures = 0
        try:
            for position, (asset, name) in enumerate(queue, 1):
                url = asset_url(asset, name)
                status, body, error = fetcher.get(url)
                if status in (401, 403, 429, 503) and fetcher.name == "http" and args.method == "auto":
                    print(f"[http] refused with HTTP {status} ({error or 'no body'}); switching to the browser session")
                    fetcher = browser()
                    time.sleep(args.delay)
                    status, body, error = fetcher.get(url)
                dest = STORE / name
                result = {"ok": False, "http_status": status}
                if status == 200:
                    write_atomic(dest, body)
                    result = validate_file(dest, asset["type"])
                    if result["ok"]:
                        result["origin"] = f"fetched ({fetcher.name}) {_dt.date.today().isoformat()}"
                        method_used[fetcher.name] += 1
                    else:
                        dest.unlink(missing_ok=True)  # only the invalid body just written
                        result["http_status"] = 200
                else:
                    result["reason"] = f"HTTP {status}" if status else "request failed"
                    if error:
                        result["error"] = error[:200]
                state[name] = result
                failures = 0 if result["ok"] else failures + 1
                print(f"{'OK  ' if result['ok'] else 'FAIL'} {position}/{len(queue)} {name} "
                      f"{result.get('bytes', '')} {result.get('reason', '')}", flush=True)
                if failures >= args.max_failures:
                    print(f"stopping: {failures} consecutive failures (no retry storm)")
                    break
                time.sleep(args.delay)
        finally:
            fetcher.close()

    manifest = build_manifest(ids, assets, shaders, state, method_used)
    MANIFEST.write_text(json.dumps(manifest, indent=1, ensure_ascii=False) + "\n", encoding="utf-8", newline="\n")
    counts = manifest["counts"]
    print(json.dumps(counts, indent=1))
    print(f"manifest: {MANIFEST}")
    return 0 if counts["texture_assets_missing"] == 0 else 2


def build_manifest(ids, assets, shaders, state, method_used) -> dict:
    asset_rows = []
    for src, asset in assets.items():
        files = []
        for face, name in enumerate(asset_files(asset)):
            result = state.get(name, {"ok": False, "reason": "absent"})
            row = {"file": name, "url": asset_url(asset, name), "status": "ok" if result["ok"] else "missing"}
            if asset["type"] == "cubemap":
                row["face"] = CUBE_FACES[face]
            if result["ok"]:
                row["sha256"] = sha256_of(STORE / name)
            for key in ("bytes", "format", "width", "height", "depth", "channels", "mode", "power_of_two",
                        "component", "header_bytes", "origin", "reason", "http_status", "error"):
                if key in result:
                    row[key] = result[key]
            files.append(row)
        ok = all(f["status"] == "ok" for f in files)
        first = files[0]
        row = {
            "source_path": src, "type": asset["type"],
            "layout": {"cubemap": "cube6", "volume": "volume3d"}.get(
                asset["type"], "2d" if asset["type"] == "texture" else "media"),
            "blocking": asset["type"] in TEXTURE_TYPES,
            "status": "ok" if ok else ("partial" if any(f["status"] == "ok" for f in files) else "missing"),
            "local_file": first["file"] if first["status"] == "ok" else None,
            "sha256": first.get("sha256"), "bytes": sum(f.get("bytes", 0) for f in files),
        }
        for key in ("width", "height", "depth", "channels", "format", "component"):
            if key in first:
                row[key] = first[key]
        if asset["type"] == "cubemap":
            row["face_order"] = list(CUBE_FACES)
            sizes = {(f.get("width"), f.get("height")) for f in files if f["status"] == "ok"}
            if ok and (len(sizes) != 1 or first.get("width") != first.get("height")):
                row["status"] = "invalid"
                row["reason"] = f"cube faces are not six equal squares: {sorted(sizes)}"
        if "also_bound_as" in asset:
            row["also_bound_as"] = asset["also_bound_as"]
        row["files"] = files
        row["use_count"] = len(asset["used_by"])
        row["shader_count"] = len({u["shader"] for u in asset["used_by"]})
        row["sampler_variants"] = sorted({json.dumps(u["sampler"], sort_keys=True) for u in asset["used_by"]})
        row["sampler_variants"] = [json.loads(v) for v in row["sampler_variants"]]
        row["used_by"] = asset["used_by"]
        asset_rows.append(row)
    status_of = {row["source_path"]: row["status"] for row in asset_rows}

    runtime_inputs: Counter = Counter()
    sampler_usage: Counter = Counter()
    for sid, entry in shaders.items():
        if entry.get("status") == "no_metadata":
            continue
        missing, media_missing, runtime = [], [], set()
        for rp in entry["passes"]:
            for ch in rp["channels"]:
                if ch["type"] in TEXTURE_TYPES:
                    ch["asset_status"] = status_of[ch["asset"]]
                    sampler_usage[(ch["type"], ch["sampler"]["filter"], ch["sampler"]["wrap"],
                                   ch["sampler"]["vflip"], ch["sampler"]["srgb"])] += 1
                    if ch["asset_status"] != "ok":
                        missing.append(ch["asset"])
                elif ch["type"] in MEDIA_TYPES:
                    ch["asset_status"] = status_of[ch["asset"]]
                    runtime.add(ch["type"])
                    if ch["asset_status"] != "ok":
                        media_missing.append(ch["asset"])
                else:
                    runtime.add(ch["type"])
                    runtime_inputs[ch["type"]] += 1
        uses_textures = any(ch["type"] in TEXTURE_TYPES for rp in entry["passes"] for ch in rp["channels"])
        entry["texture_inputs"] = ("none" if not uses_textures else "missing" if missing else "satisfied")
        entry["missing_assets"] = sorted(set(missing))
        entry["non_texture_inputs"] = sorted(runtime)
        if media_missing:
            entry["missing_media"] = sorted(set(media_missing))
        entry["multipass"] = any(rp["type"] != "image" for rp in entry["passes"])
        entry["status"] = "ok" if not missing else "missing_assets"

    def count(pred):
        return sum(1 for row in asset_rows if pred(row))

    with_meta = [e for e in shaders.values() if e.get("status") != "no_metadata"]
    counts = {
        "bank_shaders": len(ids),
        "bank_shaders_without_metadata": len(shaders) - len(with_meta),
        "shaders_with_texture_inputs": sum(1 for e in with_meta if e["texture_inputs"] != "none"),
        "shaders_texture_inputs_satisfied": sum(1 for e in with_meta if e["texture_inputs"] == "satisfied"),
        "shaders_texture_inputs_missing": sum(1 for e in with_meta if e["texture_inputs"] == "missing"),
        "shaders_without_any_input": sum(1 for e in with_meta if not any(rp["channels"] for rp in e["passes"])),
        "distinct_assets_by_type": dict(sorted(Counter(row["type"] for row in asset_rows).items())),
        "texture_assets": count(lambda r: r["blocking"]),
        "texture_assets_ok": count(lambda r: r["blocking"] and r["status"] == "ok"),
        "texture_assets_missing": count(lambda r: r["blocking"] and r["status"] != "ok"),
        "media_assets_ok": count(lambda r: not r["blocking"] and r["status"] == "ok"),
        "media_assets_missing": count(lambda r: not r["blocking"] and r["status"] != "ok"),
        "files_ok": sum(1 for r in asset_rows for f in r["files"] if f["status"] == "ok"),
        "files_missing": sum(1 for r in asset_rows for f in r["files"] if f["status"] != "ok"),
        "store_bytes": sum(f.get("bytes", 0) for r in asset_rows for f in r["files"] if f["status"] == "ok"),
        "fetched_this_run": dict(method_used),
        "runtime_input_bindings": dict(sorted(runtime_inputs.items())),
    }
    return {
        "schema": 1,
        "generated_by": "engine/tools/shader_bank/fetch_assets.py",
        "generated_utc": _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "store": "target/shader-bank/assets",
        "note": "Shadertoy media, for the owner's local use; not redistributable; lives under git-ignored target/.",
        "cube_face_order": list(CUBE_FACES),
        "counts": counts,
        "sampler_usage": [
            {"type": k[0], "filter": k[1], "wrap": k[2], "vflip": k[3], "srgb": k[4], "bindings": v}
            for k, v in sorted(sampler_usage.items(), key=lambda kv: (-kv[1], str(kv[0])))
        ],
        "assets": asset_rows,
        "shaders": shaders,
    }


if __name__ == "__main__":
    sys.exit(main())
