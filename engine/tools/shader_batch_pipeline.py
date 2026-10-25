#!/usr/bin/env python3
"""Prepare and qualify a newly imported Shadertoy batch for the native Pi bank.

This program is deliberately offline: downloading is handled by
fetch_shadertoy_user.py, while the Pi benchmark remains the authority for
performance.  A publish step can only select entries already measured in a
report.  It never starts a renderer, changes a Pi, or contacts Shadertoy.
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "engine" / "tools"
TARGET = ROOT / "target" / "shader-pipeline"


def read_ids(path: Path) -> list[str]:
    try:
        values = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(values, list): values = [values]
    except json.JSONDecodeError:
        values = path.read_text(encoding="utf-8", errors="replace").splitlines()
    result = []
    import re
    for value in values:
        for line in str(value).splitlines():
            item = line.strip()
            hit = re.search(r"shadertoy\.com/view/([A-Za-z0-9]{6})(?![A-Za-z0-9])", item, re.I)
            sid = hit.group(1) if hit else item if re.fullmatch(r"[A-Za-z0-9]{6}", item) else None
            if sid and sid not in result: result.append(sid)
    return result


def qualify(item: dict, resolution: str) -> bool:
    a, b = item.get("1080p", {}), item.get("720p", {})
    if resolution == "1080": return a.get("suitable_60fps") is True
    if resolution == "720": return b.get("suitable_60fps") is True
    return a.get("suitable_60fps") is True and b.get("suitable_60fps") is True


def prepare(args: argparse.Namespace) -> int:
    ids = read_ids(args.ids_file)
    if not ids: raise SystemExit("no valid six-character IDs in input")
    batch = TARGET / args.name
    batch.mkdir(parents=True, exist_ok=True)
    ids_path = batch / "ids.json"; ids_path.write_text(json.dumps(ids, indent=2), encoding="utf-8")
    native = batch / "native"
    command = [sys.executable, str(TOOLS / "make_native_shadertoy.py"), "--out", str(native),
               "--ids-file", str(ids_path), "--wrapper", "react"]
    done = subprocess.run(command, text=True)
    if done.returncode: return done.returncode
    show = json.loads((native / "show.json").read_text(encoding="utf-8"))
    converted = [Path(row["path"]).stem for row in show["shaders"]]
    state = {"version": 1, "name": args.name, "prepared_at": time.time(),
             "requested_ids": ids, "converted_ids": converted,
             "native_directory": str(native), "status": "awaiting_pi_benchmark"}
    (batch / "batch.json").write_text(json.dumps(state, indent=2), encoding="utf-8")
    print(f"BATCH_PREPARED {args.name}: {len(converted)}/{len(ids)} converted")
    print(f"NEXT: stage {native} on the Pi, then run benchmark_native_shaders.py --shader-list ids.json")
    return 0


def publish(args: argparse.Namespace) -> int:
    batch = TARGET / args.name
    state_path, native = batch / "batch.json", batch / "native"
    if not state_path.is_file() or not native.is_dir(): raise SystemExit(f"unknown or unprepared batch: {args.name}")
    report = json.loads(args.report.read_text(encoding="utf-8"))
    passed = [row["shader"] for row in report.get("results", []) if qualify(row, args.resolution)]
    available = {path.stem for path in native.glob("*.fs")}
    selected = [sid for sid in passed if sid in available]
    if not selected: raise SystemExit("report has no qualified shaders from this batch")
    selected_path = batch / f"qualified-{args.resolution}.json"
    selected_path.write_text(json.dumps(selected, indent=2), encoding="utf-8")
    review = batch / f"pi-review-{args.resolution}"
    env = {**os.environ, "REVIEW_SOURCE_DIR": str(native), "REVIEW_SOURCE_SHOW": str(native / "show.json"),
           "REVIEW_OUT": str(review), "PI_BENCH_REPORT": str(args.report),
           "PI_BENCH_RESOLUTION": {"1080": "1080", "720": "720", "both": "both"}[args.resolution]}
    done = subprocess.run([sys.executable, str(TOOLS / "build_shader_review_show.py")], env=env, text=True)
    if done.returncode: return done.returncode
    state = json.loads(state_path.read_text(encoding="utf-8"))
    state.update({"published_at": time.time(), "status": "qualified_for_pi_deploy",
                  "benchmark_report": str(args.report), "resolution": args.resolution,
                  "qualified_ids": selected, "pi_review_directory": str(review)})
    state_path.write_text(json.dumps(state, indent=2), encoding="utf-8")
    print(f"BATCH_QUALIFIED {args.name}: {len(selected)} shaders; review package={review}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="command", required=True)
    p = sub.add_parser("prepare", help="convert a downloaded list into an isolated native batch")
    p.add_argument("--name", required=True); p.add_argument("--ids-file", type=Path, required=True); p.set_defaults(func=prepare)
    p = sub.add_parser("publish", help="turn a completed Pi benchmark into Stream Deck review pages")
    p.add_argument("--name", required=True); p.add_argument("--report", type=Path, required=True)
    p.add_argument("--resolution", choices=("720", "1080", "both"), default="both"); p.set_defaults(func=publish)
    args = ap.parse_args(); return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
