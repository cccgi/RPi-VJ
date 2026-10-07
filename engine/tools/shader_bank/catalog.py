#!/usr/bin/env python3
"""Catalog the shader bank -> target/shader-bank/catalog.json.

Scans the three shader libraries, dedupes by id and by content hash, joins the
raw Shadertoy metadata, classifies each shader per plan R4 and extracts static
cost features. Re-run it whenever shaders are added; it prints what changed
since the previous catalog.

    python engine/tools/shader_bank/catalog.py
"""
from __future__ import annotations

import argparse
import collections
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import (CATALOG_PATH, CLASS_MULTIPASS, CLASS_ORIGINAL, CLASS_SELF, CLASS_TEXTURE, ROOT,  # noqa: E402
                    decode, normalize_newlines, read_json, rel, sha256_bytes, split_shadertoy,
                    strip_comments, write_json)
from glsl_features import extract_features  # noqa: E402

CATALOG_VERSION = 1
NATIVE_DIR = ROOT / "target" / "native-13-shaders"
FRACTALS_DIR = ROOT / "target" / "shadertoy-fractals"
ORIGINALS_DIR = ROOT / "engine" / "examples" / "shaders"
OFFLINE_DIR = ROOT / "target" / "shadertoy-offline"
DOWNLOADS_DIR = ROOT / "target" / "shadertoy-downloads"
REVIEW_SHOW = NATIVE_DIR / "show-fixed-first-latest.json"
ENGINE_MAX_SOURCE_BYTES = 256 * 1024  # engine/src/shader.rs MAX_SOURCE_BYTES

# Scan order is also the priority when one id has different content in two
# libraries: the first library wins and the conflict is reported.
LIBRARIES = (
    ("original18", ORIGINALS_DIR),
    ("native13", NATIVE_DIR),
    ("fractals", FRACTALS_DIR / "shaders"),
)

AUDIO_CTYPES = {"music", "musicstream", "mic"}
VIDEO_CTYPES = {"video", "webcam"}
BUFFER_PASS_TYPES = {"buffer", "cubemap"}
_PASS_MARKER_RE = re.compile(r"^// ===== Pass \d+: (.*?) \((\w+)\) =====", re.M)


def load_metadata(shader_id: str, offline_dir: Path = OFFLINE_DIR, downloads_dir: Path = DOWNLOADS_DIR):
    """Shadertoy metadata for an id, or None. Falls back to the pass markers of
    the flat download when shader.json is missing (inputs are then unknown)."""
    path = offline_dir / shader_id / "shader.json"
    if path.is_file():
        try:
            document = read_json(path)
        except (json.JSONDecodeError, UnicodeDecodeError):
            document = None
        if isinstance(document, dict) and isinstance(document.get("renderpass"), list):
            passes = []
            for render_pass in document["renderpass"]:
                inputs = [{"channel": item.get("channel"), "ctype": item.get("ctype") or item.get("type") or "unknown"}
                          for item in render_pass.get("inputs") or []]
                passes.append({"type": render_pass.get("type") or "image", "name": render_pass.get("name") or "", "inputs": inputs})
            info = document.get("info") or {}
            return {"source": rel(path), "inputs_known": True, "name": info.get("name"),
                    "username": info.get("username"), "tags": info.get("tags") or [], "passes": passes}
    flat = downloads_dir / f"{shader_id}.glsl"
    if flat.is_file():
        text = flat.read_text(encoding="utf-8", errors="replace")
        if text.startswith("// ShaderToy ID:"):
            name = next((line[9:].strip() for line in text.splitlines()[:6] if line.startswith("// Name: ")), None)
            passes = [{"type": kind, "name": label, "inputs": []} for label, kind in _PASS_MARKER_RE.findall(text)]
            return {"source": rel(flat), "inputs_known": False, "name": name, "username": None, "tags": [],
                    "passes": passes or [{"type": "image", "name": "Image", "inputs": []}]}
    return None


def classify(body: str, metadata, origin: str) -> dict:
    """Primary class per plan R4 plus the flag set.

    The class describes what the *Image pass as generated* needs, because that
    is all the renderer draws:
      multipass      the Image pass reads a buffer/cubemap pass
      needs_texture  it samples any other iChannel (texture, cubemap, volume,
                     keyboard, audio, video) - none of which the renderer binds
      selfcontained  it references no iChannel at all
    An input that Shadertoy declares but the code never references does not
    count (flag `declared_unused_inputs`).
    """
    code = strip_comments(normalize_newlines(body))
    used = sorted({int(c) for c in re.findall(r"\biChannel([0-3])\b", code)})
    passes = (metadata or {}).get("passes") or []
    pass_counts = collections.Counter(p["type"] for p in passes)
    image = next((p for p in passes if p["type"] == "image"), None)
    declared = {}
    for item in (image or {}).get("inputs", []):
        if isinstance(item.get("channel"), int):
            declared[item["channel"]] = item["ctype"]
    inputs_known = bool(metadata and metadata.get("inputs_known"))
    kinds = {}
    for channel in used:
        kinds[str(channel)] = declared.get(channel) or ("undeclared" if inputs_known else "unknown")
    used_kinds = set(kinds.values())
    has_buffer_pass = any(pass_counts[t] for t in BUFFER_PASS_TYPES)
    all_ctypes = sorted({i["ctype"] for p in passes for i in p["inputs"]})

    reads_buffer = "buffer" in used_kinds
    # Buffer passes exist and the Image pass reads a channel we cannot type:
    # assume it is the buffer.
    ambiguous_buffer = has_buffer_pass and bool(used_kinds & {"undeclared", "unknown"})
    if origin == "original18":
        primary = CLASS_ORIGINAL
    elif reads_buffer or ambiguous_buffer:
        primary = CLASS_MULTIPASS
    elif used:
        primary = CLASS_TEXTURE
    else:
        primary = CLASS_SELF

    flags = {
        "texture": "texture" in used_kinds,
        "buffer": reads_buffer or ambiguous_buffer,
        "cubemap": "cubemap" in used_kinds,
        "volume": "volume" in used_kinds,
        "keyboard": "keyboard" in used_kinds,
        "audio_channel": bool(used_kinds & AUDIO_CTYPES),
        "video_webcam": bool(used_kinds & VIDEO_CTYPES),
        "undeclared_channel": bool(used_kinds & {"undeclared", "unknown"}),
        "declared_unused_inputs": bool(set(declared) - set(used)),
        "mouse": bool(re.search(r"\biMouse\b", code)),
        "uses_frame_counter": bool(re.search(r"\b(?:FRAMEINDEX|iFrame)\b", code)),
        "uses_date": bool(re.search(r"\biDate\b", code)),
        "sound_pass": bool(pass_counts["sound"]),
        "buffer_passes_unused_by_image": has_buffer_pass and primary != CLASS_MULTIPASS and origin != "original18",
        "no_metadata": metadata is None,
        "metadata_inputs_unknown": metadata is not None and not inputs_known,
    }
    return {
        "class": primary,
        "flags": flags,
        "channels": {"used": used, "kinds": kinds,
                     "declared": {str(k): v for k, v in sorted(declared.items())},
                     "declared_unused": sorted(set(declared) - set(used))},
        "passes": dict(sorted(pass_counts.items())),
        "all_input_ctypes": all_ctypes,
    }


def isf_description(text: str):
    stripped = text.lstrip("﻿ \t\r\n")
    if stripped.startswith("/*"):
        end = stripped.find("*/")
        try:
            header = json.loads(stripped[2:end])
        except (json.JSONDecodeError, ValueError):
            return None
        if isinstance(header, dict):
            return header.get("DESCRIPTION") or header.get("LABEL")
    return None


def show_labels(show_path: Path) -> dict:
    """Map resolved shader path -> label for a show manifest (missing file tolerated)."""
    if not show_path.is_file():
        return {}
    labels = {}
    for item in read_json(show_path).get("shaders", []):
        labels[(show_path.parent / item["path"]).resolve()] = item.get("label", "")
    return labels


def review_numbers(show_path: Path = REVIEW_SHOW) -> dict:
    """The `#NNN` labels the owner sees on the Stream Deck: 1-based position in
    show-fixed-first-latest.json (same rule as build_shader_review_show.py)."""
    if not show_path.is_file():
        return {}
    return {item["path"].rsplit(".", 1)[0]: index + 1
            for index, item in enumerate(read_json(show_path).get("shaders", []))}


def build_record(path: Path, origin: str, metadata_loader=load_metadata) -> dict:
    data = path.read_bytes()
    text, _ = decode(data)
    start, end = split_shadertoy(text)
    body = text[start:end]
    shader_id = path.stem
    metadata = metadata_loader(shader_id) if origin != "original18" else None
    record = {"id": shader_id, "origin": origin, "libraries": [origin], "path": rel(path), "other_paths": [],
              "sha256": sha256_bytes(data), "size_bytes": len(data)}
    code = strip_comments(normalize_newlines(body))
    record["code_sha256"] = sha256_bytes(re.sub(r"\s+", " ", code).strip().encode("utf-8"))
    record["name"] = (metadata or {}).get("name") or isf_description(text) or shader_id
    record["author"] = (metadata or {}).get("username")
    record["tags"] = (metadata or {}).get("tags") or []
    record["metadata_source"] = (metadata or {}).get("source")
    record["review_number"] = None
    record.update(classify(body, metadata, origin))
    record["flags"]["exceeds_engine_source_limit"] = len(data) > ENGINE_MAX_SOURCE_BYTES
    record["flags"]["shadertoy_wrapper"] = start > 0 and end < len(text)
    record["features"] = extract_features(body)
    return record


def build_catalog(libraries=LIBRARIES, metadata_loader=load_metadata, review_show: Path = REVIEW_SHOW,
                  fractals_dir: Path = FRACTALS_DIR, native_dir: Path = NATIVE_DIR) -> dict:
    records: dict = {}
    conflicts, same_id_same_content = [], []
    scanned = {}
    for origin, directory in libraries:
        files = sorted(Path(directory).glob("*.fs")) if Path(directory).is_dir() else []
        scanned[origin] = {"dir": rel(directory), "files": len(files)}
        for path in files:
            record = build_record(path, origin, metadata_loader)
            existing = records.get(record["id"])
            if existing is None:
                records[record["id"]] = record
            elif existing["sha256"] == record["sha256"]:
                existing["other_paths"].append(record["path"])
                existing["libraries"].append(origin)
                same_id_same_content.append({"id": record["id"], "paths": [existing["path"], record["path"]]})
            else:
                conflicts.append({"id": record["id"], "kept": existing["path"], "kept_sha256": existing["sha256"],
                                  "dropped": record["path"], "dropped_sha256": record["sha256"]})

    # Membership through show manifests: the fractals library mostly points
    # back into native-13 rather than holding its own copies.
    by_path = {(ROOT / r["path"]).resolve(): r for r in records.values()}
    show_reports = {}
    for library, show_path in (("native13", native_dir / "show.json"), ("fractals", fractals_dir / "show.json")):
        labels = show_labels(show_path)
        missing = []
        for resolved, label in labels.items():
            record = by_path.get(resolved)
            if record is None:
                other = records.get(resolved.stem)
                if other is not None and resolved.is_file():
                    data = resolved.read_bytes()
                    if sha256_bytes(data) != other["sha256"]:
                        conflicts.append({"id": resolved.stem, "kept": other["path"], "kept_sha256": other["sha256"],
                                          "dropped": rel(resolved), "dropped_sha256": sha256_bytes(data)})
                    continue
                missing.append(rel(resolved))
                continue
            if library not in record["libraries"]:
                record["libraries"].append(library)
            record.setdefault("show_label", label)
            if record["name"] == record["id"] and label.startswith(record["id"]):
                record["name"] = label[len(record["id"]):].strip() or record["id"]
        show_reports[library] = {"show": rel(show_path), "entries": len(labels), "entries_missing_file": missing}
    native_show = show_labels(native_dir / "show.json")
    if native_show:
        show_reports["native13"]["files_not_in_show"] = sorted(
            r["id"] for r in records.values() if r["origin"] == "native13" and (ROOT / r["path"]).resolve() not in native_show)
    index_path = fractals_dir / "index.json"
    if index_path.is_file():
        index_ids = {item["id"] for item in read_json(index_path).get("shaders", [])}
        for shader_id in index_ids & set(records):
            records[shader_id]["flags"]["in_fractals_index"] = True
        show_reports["fractals"]["index_ids"] = len(index_ids)
        show_reports["fractals"]["index_ids_without_shader"] = sorted(index_ids - set(records))

    numbers = review_numbers(review_show)
    for shader_id, number in numbers.items():
        if shader_id in records:
            records[shader_id]["review_number"] = number

    by_hash, by_code = collections.defaultdict(list), collections.defaultdict(list)
    for record in records.values():
        by_hash[record["sha256"]].append(record["id"])
        by_code[record["code_sha256"]].append(record["id"])
    identical = sorted(sorted(ids) for ids in by_hash.values() if len(ids) > 1)
    identical_sets = {tuple(ids) for ids in identical}
    same_code = sorted(sorted(ids) for ids in by_code.values() if len(ids) > 1 and tuple(sorted(ids)) not in identical_sets)
    for group in identical:
        for shader_id in group:
            records[shader_id]["duplicate_of"] = group[0] if shader_id != group[0] else None
            records[shader_id]["flags"]["identical_content_group"] = True

    shaders = sorted(records.values(), key=lambda r: (r["origin"] != "original18", r["id"].lower(), r["id"]))
    return {
        "version": CATALOG_VERSION,
        "generated_by": "engine/tools/shader_bank/catalog.py",
        "summary": summarize(shaders),
        "sources": scanned,
        "shows": show_reports,
        "review_show": {"path": rel(review_show), "entries": len(numbers),
                        "entries_without_shader": sorted(set(numbers) - set(records))},
        "duplicates": {"same_id_same_content": same_id_same_content,
                       "identical_content_different_id": identical,
                       "same_code_different_id": same_code},
        "conflicts": conflicts,
        "shaders": shaders,
    }


def summarize(shaders) -> dict:
    by_class = collections.Counter(r["class"] for r in shaders)
    flags = collections.Counter()
    kinds = collections.Counter()
    for record in shaders:
        for name, value in record["flags"].items():
            if value:
                flags[name] += 1
        kinds[",".join(sorted(set(record["channels"]["kinds"].values()))) or "-"] += 1
    feature_counts = {
        "raymarch": sum(1 for r in shaders if r["features"]["raymarch"]),
        "with_loops": sum(1 for r in shaders if r["features"]["loops"]),
        "with_unresolved_loops": sum(1 for r in shaders if r["features"]["loops_unresolved"]),
        "with_aa_defines": sum(1 for r in shaders if r["features"]["aa_defines"]),
        "with_quality_defines": sum(1 for r in shaders if r["features"]["quality_defines"]),
        "uses_hw_performance": sum(1 for r in shaders if r["features"]["uses_hw_performance"]),
        "uses_derivatives": sum(1 for r in shaders if r["features"]["derivatives"]),
        "with_texture_fetches": sum(1 for r in shaders if r["features"]["texture_fetches"]),
        "with_fbm_hint": sum(1 for r in shaders if r["features"]["fbm_octaves_hint"]),
    }
    return {"total": len(shaders), "by_class": dict(sorted(by_class.items())),
            "by_origin": dict(sorted(collections.Counter(r["origin"] for r in shaders).items())),
            "with_review_number": sum(1 for r in shaders if r["review_number"]),
            "flags": dict(sorted(flags.items())),
            "image_channel_kinds": dict(sorted(kinds.items(), key=lambda kv: (-kv[1], kv[0]))),
            "features": feature_counts}


def diff_catalogs(old, new) -> dict:
    """What changed since the previous run (drives incremental re-benching)."""
    before = {r["id"]: r for r in (old or {}).get("shaders", [])}
    after = {r["id"]: r for r in new["shaders"]}
    return {
        "added": sorted(set(after) - set(before)),
        "removed": sorted(set(before) - set(after)),
        "content_changed": sorted(i for i in set(after) & set(before) if after[i]["sha256"] != before[i]["sha256"]),
        "class_changed": sorted(i for i in set(after) & set(before) if after[i]["class"] != before[i]["class"]),
    }


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--out", type=Path, default=CATALOG_PATH, help="catalog.json to write")
    parser.add_argument("--changes-out", type=Path, default=None,
                        help="write ids added or changed since the previous catalog, one per line "
                             "(feed to make_manifest.py --ids-file)")
    args = parser.parse_args(argv)
    previous = read_json(args.out) if args.out.is_file() else None
    catalog = build_catalog()
    changes = diff_catalogs(previous, catalog)
    written = write_json(args.out, catalog)
    summary = catalog["summary"]
    print(f"CATALOG {summary['total']} shaders -> {rel(args.out)}{'' if written else ' (unchanged)'}")
    print("  by class: " + ", ".join(f"{k}={v}" for k, v in summary["by_class"].items()))
    print("  by origin: " + ", ".join(f"{k}={v}" for k, v in summary["by_origin"].items()))
    print(f"  with #NNN review number: {summary['with_review_number']}")
    print("  flags: " + ", ".join(f"{k}={v}" for k, v in summary["flags"].items()))
    print("  features: " + ", ".join(f"{k}={v}" for k, v in summary["features"].items()))
    dup = catalog["duplicates"]
    print(f"  duplicates: same id+content in two libraries={len(dup['same_id_same_content'])}, "
          f"identical content under different ids={len(dup['identical_content_different_id'])} groups, "
          f"same code (comments/whitespace aside) under different ids={len(dup['same_code_different_id'])} groups")
    print(f"  conflicts (same id, different content): {len(catalog['conflicts'])}")
    for conflict in catalog["conflicts"]:
        print(f"    {conflict['id']}: kept {conflict['kept']}, dropped {conflict['dropped']}")
    if previous is not None:
        print("  since previous catalog: " + ", ".join(f"{k}={len(v)}" for k, v in changes.items()))
    if args.changes_out:
        ids = sorted(set(changes["added"]) | set(changes["content_changed"]))
        args.changes_out.parent.mkdir(parents=True, exist_ok=True)
        args.changes_out.write_text("".join(f"{i}\n" for i in ids), encoding="utf-8")
        print(f"  {len(ids)} added/changed ids -> {rel(args.changes_out)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
