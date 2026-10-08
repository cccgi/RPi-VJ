#!/usr/bin/env python3
"""Shared helpers for the shader-bank tooling (catalog, adapt, manifest, report).

Standard library only. Every tool in this package is re-runnable: outputs are
rewritten deterministically from the current inputs, so running again after new
shaders arrive just works.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BANK_OUT = ROOT / "target" / "shader-bank"
CATALOG_PATH = BANK_OUT / "catalog.json"
VARIANTS_DIR = BANK_OUT / "variants"
STAGING_DIR = BANK_OUT / "staging"
ASSETS_DIR = BANK_OUT / "assets"                 # written by fetch_assets.py
ASSET_MANIFEST_PATH = ASSETS_DIR / "manifest.json"

# Plan section 3.1 defaults (Pi-side paths).
PI_SHADER_ROOT = "/srv/rpi-vj-bench/shaders"
PI_RESULTS_PATH = "/var/lib/rpi-vj-bench/results.jsonl"
PI_THUMB_DIR = "/var/lib/rpi-vj-bench/thumbs"
PI_ASSET_ROOT = "/srv/rpi-vj-bench/assets"

# Asset-manifest input type -> channel binding kind in the engine's manifests
# (engine/src/texture.rs). Every other input type (buffer, keyboard, music,
# video, webcam ...) has no file the renderer can bind.
BINDING_KINDS = {"texture": "2d", "cubemap": "cube", "volume": "volume"}
# Catalog channel kind -> binding kind, for checking a result against the catalog.
CATALOG_BINDING_KINDS = BINDING_KINDS
CUBE_FACES = 6

# Render tiers (plan R1): height -> width, 16:9.
TIER_WIDTHS = {1080: 1920, 720: 1280, 540: 960, 360: 640}
DEFAULT_TIERS = (720, 1080)

# Classes (plan R4) plus the owner's hand-built reference set.
CLASS_ORIGINAL = "original18"
CLASS_SELF = "selfcontained"
CLASS_TEXTURE = "needs_texture"
CLASS_MULTIPASS = "multipass"
CLASSES = (CLASS_ORIGINAL, CLASS_SELF, CLASS_TEXTURE, CLASS_MULTIPASS)

# Marker lines written by engine/tools/make_native_shadertoy.py.
SHADERTOY_HEAD_END = "uniform vec3 iChannelResolution[4];"
# Start of the generated audio wrapper: the legacy `main`, or the comment that
# opens the reactive wrapper (REACT_MARKER in make_native_shadertoy.py).
SHADERTOY_WRAPPER_RE = re.compile(r"\n[ \t]*void[ \t]+main[ \t]*\([ \t]*\)[ \t]*\{\s*vec2 center = 0\.5 \* u_resolution;"
                                  r"|\n// rpi-vj react wrapper v\d+")

_COMMENT_RE = re.compile(r"//[^\n]*|/\*.*?\*/", re.S)
_BLANK_RE = re.compile(r"[^\n]")


def strip_comments(text: str) -> str:
    """Blank out GLSL comments, keeping length and newlines so offsets still map."""
    return _COMMENT_RE.sub(lambda m: _BLANK_RE.sub(" ", m.group(0)), text)


def normalize_newlines(text: str) -> str:
    """Analysis-only view: the bank mixes CRLF, CRCRLF and LF."""
    return re.sub(r"\r+\n", "\n", text).replace("\r", "\n")


def decode(data: bytes) -> tuple[str, str]:
    """Decode shader bytes; returns (text, encoding) so a transform can round-trip."""
    try:
        return data.decode("utf-8"), "utf-8"
    except UnicodeDecodeError:
        return data.decode("latin-1"), "latin-1"


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def split_shadertoy(text: str) -> tuple[int, int]:
    """Return (body_start, body_end) of the Shadertoy code inside a generated
    native file: after the fixed header, before the audio wrapper `main`.
    Files that do not follow that layout are treated as all body (after a
    leading ISF metadata comment, if any)."""
    start = text.find(SHADERTOY_HEAD_END)
    if start >= 0:
        start += len(SHADERTOY_HEAD_END)
        match = None
        for match in SHADERTOY_WRAPPER_RE.finditer(text, start):
            pass
        return start, (match.start() if match else len(text))
    stripped = text.lstrip("﻿ \t\r\n")
    offset = len(text) - len(stripped)
    if stripped.startswith("/*"):
        end = stripped.find("*/")
        if end >= 0:
            return offset + end + 2, len(text)
    return 0, len(text)


def read_json(path: Path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def write_json(path: Path, value) -> bool:
    """Write JSON only when the content changed; returns True when written."""
    return write_bytes_if_changed(path, (json.dumps(value, indent=1, ensure_ascii=False) + "\n").encode("utf-8"))


def write_bytes_if_changed(path: Path, data: bytes) -> bool:
    path = Path(path)
    if path.is_file() and path.read_bytes() == data:
        return False
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_bytes(data)
    os.replace(tmp, path)
    return True


def rel(path: Path) -> str:
    """Repo-relative POSIX path where possible (catalog paths are portable)."""
    try:
        return Path(path).resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return Path(path).as_posix()


def load_catalog(path: Path = CATALOG_PATH) -> dict:
    if not Path(path).is_file():
        raise SystemExit(f"catalog not found: {path} (run catalog.py first)")
    return read_json(path)


def load_asset_manifest(path: Path = ASSET_MANIFEST_PATH):
    """The asset store manifest, or None when the store has not been built."""
    return read_json(path) if Path(path).is_file() else None


def channel_bindings(asset_manifest, shader_id: str) -> tuple[list, list]:
    """Image-pass inputs of one shader as (bindings, unbound).

    `bindings` are the channels a file in the asset store satisfies:
    {"channel", "file", "kind", "filter", "wrap", "vflip"} with `file` the
    store file name (the first face for a cubemap). `unbound` lists the other
    inputs as {"channel", "type", "reason"}: inputs with no file (buffer,
    keyboard, music ...) and file inputs whose asset is not in the store."""
    shader = ((asset_manifest or {}).get("shaders") or {}).get(shader_id)
    if not shader:
        return [], []
    assets = {asset["source_path"]: asset for asset in asset_manifest.get("assets", [])}
    image = next((p for p in shader.get("passes", []) if p.get("type") == "image"), None)
    bindings, unbound = [], []
    for item in sorted((image or {}).get("channels", []), key=lambda c: c.get("channel", 0)):
        channel, kind = item.get("channel"), BINDING_KINDS.get(item.get("type"))
        asset = assets.get(item.get("asset")) if item.get("asset") else None
        if kind is None:
            unbound.append({"channel": channel, "type": item.get("type"), "reason": "no file input"})
        elif not asset or asset.get("status") != "ok" or not asset.get("local_file"):
            unbound.append({"channel": channel, "type": item.get("type"), "reason": "asset missing from the store"})
        else:
            sampler = item.get("sampler") or {}
            bindings.append({"channel": channel, "file": asset["local_file"], "kind": kind,
                             "filter": sampler.get("filter", "mipmap"), "wrap": sampler.get("wrap", "repeat"),
                             "vflip": bool(sampler.get("vflip", True))})
    return bindings, unbound


def binding_files(binding: dict) -> list[str]:
    """Store file names one binding needs: six faces for a cubemap
    (`<hash>.<ext>`, `<hash>_1.<ext>` .. `<hash>_5.<ext>`), else the file."""
    name = binding["file"]
    if binding["kind"] != "cube":
        return [name]
    stem, dot, extension = name.rpartition(".")
    if not dot:
        stem, extension = name, ""
    return [name] + [f"{stem}_{face}{dot}{extension}" for face in range(1, CUBE_FACES)]


def manifest_channels(bindings, asset_root: str) -> list:
    """Bindings in the engine's manifest spelling, with paths under `asset_root`
    (absolute on the Pi for a bench manifest, relative for a show manifest)."""
    root = asset_root.rstrip("/")
    return [{"channel": b["channel"], "path": f"{root}/{b['file']}", "kind": b["kind"], "filter": b["filter"],
             "wrap": b["wrap"], "vflip": b["vflip"]} for b in bindings]


def parse_tiers(spec) -> list[list[int]]:
    """'720,1080' or '1280x720,1920x1080' -> [[1280, 720], [1920, 1080]] in the given order."""
    items = spec.split(",") if isinstance(spec, str) else list(spec)
    tiers = []
    for item in items:
        item = str(item).strip().lower()
        if not item:
            continue
        if "x" in item:
            width, height = (int(v) for v in item.split("x", 1))
        else:
            height = int(item.rstrip("p"))
            width = TIER_WIDTHS.get(height) or (height * 16 // 9)
        if width <= 0 or height <= 0:
            raise ValueError(f"invalid tier {item!r}")
        if [width, height] not in tiers:
            tiers.append([width, height])
    if not tiers:
        raise ValueError("no tiers given")
    return tiers


def read_id_list(path: Path) -> list[str]:
    """Ids from a text file (one per line, '#' comments) or a JSON list of ids /
    objects with an 'id' field."""
    text = Path(path).read_text(encoding="utf-8")
    if text.lstrip().startswith(("[", "{")):
        value = json.loads(text)
        if isinstance(value, dict):
            value = value.get("ids") or value.get("qualified") or value.get("shaders") or []
        return [v["id"] if isinstance(v, dict) else str(v) for v in value]
    return [line.split("#", 1)[0].strip() for line in text.splitlines() if line.split("#", 1)[0].strip()]


def iter_results(paths) -> tuple[list[dict], list[str]]:
    """Read results JSONL files in order. Malformed lines (a crash can leave a
    torn last line) are skipped and reported, never fatal."""
    events, warnings = [], []
    for path in paths:
        path = Path(path)
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            for number, line in enumerate(handle, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    event = json.loads(line)
                except json.JSONDecodeError:
                    warnings.append(f"{path.name}:{number}: malformed line skipped")
                    continue
                if not isinstance(event, dict) or "id" not in event:
                    warnings.append(f"{path.name}:{number}: line without id skipped")
                    continue
                event.setdefault("variant", "base")
                events.append(event)
    return events, warnings


ERROR_STATUSES = ("prepare_error", "compile_error", "too_slow", "gl_error", "crash", "hang")
ALL_STATUSES = ("ok",) + ERROR_STATUSES


def merge_results(events) -> dict:
    """Fold result events into {(id, variant): record}.

    Later lines win per (id, variant, tier): tier measurements are merged key
    by key, so a later 540/360 run adds to an earlier 720/1080 run. Status,
    error, compile time and thumbnail come from the latest result line; pixel
    statistics are kept from the latest line that carried them. `channels`
    (which input textures were bound, an engine extension to the result line)
    is the latest line's: a shader re-measured without its textures counts as
    unbound again.

    Worst-case audio (the second engine extension): `tiers` is the 128 BPM
    pattern as always; `tiers_max` holds the same tiers measured with every
    audio input pinned at maximum. A line that measures a tier replaces both
    patterns for that tier, so an older worst-case figure is never paired with
    a newer 128 BPM one. `pixels_max` and `thumb_max` follow `pixels` and
    `thumb`. Lines without these fields merge exactly as before."""
    merged: dict = {}
    for event in events:
        if event.get("event", "result") != "result":
            continue
        key = (str(event["id"]), str(event.get("variant", "base")))
        record = merged.setdefault(key, {"id": key[0], "variant": key[1], "tiers": {}, "tiers_max": {}, "lines": 0})
        record["lines"] += 1
        record["status"] = event.get("status", "ok")
        record["error"] = event.get("error")
        record["channels"] = event["channels"] if isinstance(event.get("channels"), dict) else {}
        for field in ("compile_ms", "thumb", "temp_c", "throttled"):
            if event.get(field) is not None:
                record[field] = event[field]
        if isinstance(event.get("pixels"), dict):
            record["pixels"] = event["pixels"]
            # the worst-case picture belongs to the same run as the 128 BPM one
            for field in ("pixels_max", "thumb_max"):
                record.pop(field, None)
                if event.get(field) is not None:
                    record[field] = event[field]
        loud = event.get("tiers_max") if isinstance(event.get("tiers_max"), dict) else {}
        for tier, data in (event.get("tiers") or {}).items():
            if isinstance(data, dict):
                record["tiers"][str(tier)] = data
                record["tiers_max"].pop(str(tier), None)
                if isinstance(loud.get(tier), dict):
                    record["tiers_max"][str(tier)] = loud[tier]
    return merged
