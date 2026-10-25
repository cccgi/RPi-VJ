#!/usr/bin/env python3
"""One entry point for Shadertoy import, native conversion, Pi qualification and append-only deployment.

Run with no arguments for prompts, or use --profile / --url / --file. A failed
phase leaves its batch under target/shader-pipeline so --resume NAME can retry.
The Pi show is read again just before deployment and checked by hash before
an atomic replacement. Existing shader entries and pages are never removed.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLS = ROOT / "engine" / "tools"
TARGET = ROOT / "target" / "shader-pipeline"
OFFLINE = ROOT / "target" / "shadertoy-offline"
DOWNLOADS = ROOT / "target" / "shadertoy-downloads"
ASSET_STORE = ROOT / "target" / "shader-bank" / "assets"
SHOW_REMOTE = "/srv/rpi-vj-native/show.json"
ID_URL = re.compile(r"shadertoy\.com/view/([A-Za-z0-9]{6})(?![A-Za-z0-9])", re.I)
ID_BARE = re.compile(r"^[A-Za-z0-9]{6}$")
NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")
FILE_TYPES = {"texture": "2d", "cubemap": "cube", "volume": "volume"}


def run(argv: list[str], *, check: bool = True) -> subprocess.CompletedProcess:
    print("+", " ".join(map(str, argv[:3])), "...", flush=True)
    result = subprocess.run([str(x) for x in argv], check=False)
    if check and result.returncode:
        raise RuntimeError(f"{argv[0]} failed with exit code {result.returncode}")
    return result


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8-sig"))


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def unique_ids(values) -> list[str]:
    found = []
    for raw in values:
        for line in str(raw).splitlines():
            text = line.strip()
            if not text or text.startswith("#"):
                continue
            match = ID_URL.search(text)
            sid = match.group(1) if match else text if ID_BARE.fullmatch(text) else None
            if sid is None:
                raise ValueError(f"Invalid Shadertoy ID or URL: {text[:100]}")
            if sid not in found:
                found.append(sid)
    return found


def file_items(path: Path):
    content = path.read_text(encoding="utf-8-sig")
    try:
        parsed = json.loads(content)
        if isinstance(parsed, list):
            return parsed
    except json.JSONDecodeError:
        pass
    return content.splitlines()


def source_spec(args):
    if args.resume:
        if not NAME.fullmatch(args.resume):
            raise ValueError("Invalid batch name for --resume")
        state = read_json(TARGET / args.resume / "input.json")
        return state["mode"], state["value"], args.resume
    choices = [bool(args.profile), bool(args.url), bool(args.file)]
    if sum(choices) > 1:
        raise ValueError("Choose one input: --profile, --url, or --file")
    if args.profile:
        mode, value = "profile", args.profile
    elif args.file:
        mode, value = "file", str(Path(args.file).resolve())
    elif args.url:
        mode, value = "urls", args.url
    else:
        print("Input: [1] Shadertoy user profile  [2] paste URLs/IDs  [3] text file")
        choice = input("Choose 1, 2, or 3: ").strip()
        if choice == "1":
            mode, value = "profile", input("Shadertoy username: ").strip()
        elif choice == "2":
            print("Paste one URL or ID per line. Press Enter on an empty line to finish.")
            lines = []
            while True:
                line = input("> ").strip()
                if not line:
                    break
                lines.append(line)
            mode, value = "urls", lines
        elif choice == "3":
            mode, value = "file", str(Path(input("Text file path: ").strip().strip('"')).resolve())
        else:
            raise ValueError("Input selection must be 1, 2, or 3")
    if mode == "profile":
        profile_url = re.fullmatch(r"https?://(?:www\.)?shadertoy\.com/user/([A-Za-z0-9_-]+)/?",
                                   value, re.I)
        if profile_url:
            value = profile_url.group(1)
        if not re.fullmatch(r"[A-Za-z0-9_-]+", value):
            raise ValueError("Invalid Shadertoy username or profile URL")
    if mode == "file" and not Path(value).is_file():
        raise ValueError(f"File not found: {value}")
    if mode in {"urls", "file"}:
        items = value if mode == "urls" else file_items(Path(value))
        if not unique_ids(items):
            raise ValueError("No shader IDs found")
    default = value if mode == "profile" else Path(value).stem if mode == "file" else "manual-" + hashlib.sha256(
        "\n".join(unique_ids(value)).encode()).hexdigest()[:8]
    name = args.batch or re.sub(r"[^A-Za-z0-9._-]", "-", default)[:64]
    if not NAME.fullmatch(name):
        raise ValueError(f"Invalid batch name: {name}")
    return mode, value, name


def imported_ids(mode, value, batch: Path, name: str, args) -> list[str]:
    ids_path = batch / "ids.json"
    if mode == "profile":
        state_path = ROOT / "target" / "shadertoy-users" / value.lower() / "ids.json"
        need_fetch = args.refresh_ids or not state_path.is_file()
        if not need_fetch:
            ids = unique_ids(read_json(state_path))
            need_fetch = any(not (DOWNLOADS / f"{sid}.glsl").is_file() or
                             not (OFFLINE / sid / "shader.json").is_file() for sid in ids)
        if need_fetch:
            cmd = [sys.executable, str(TOOLS / "fetch_shadertoy_user.py"), value]
            if args.refresh_ids:
                cmd.append("--refresh-ids")
            run(cmd)
        if not state_path.is_file():
            raise RuntimeError("Profile discovery did not save an ID list")
        ids = unique_ids(read_json(state_path))
    else:
        ids = unique_ids(value if mode == "urls" else file_items(Path(value)))
        missing = [sid for sid in ids if not (DOWNLOADS / f"{sid}.glsl").is_file() or
                   not (OFFLINE / sid / "shader.json").is_file()]
        if missing:
            fetch_list = batch / "fetch-ids.txt"
            fetch_list.write_text("\n".join(missing) + "\n", encoding="utf-8")
            run([sys.executable, str(TOOLS / "fetch_shadertoy_user.py"), "--ids-file", str(fetch_list),
                 "--name", name])
    write_json(ids_path, ids)
    missing = [sid for sid in ids if not (DOWNLOADS / f"{sid}.glsl").is_file() or
               not (OFFLINE / sid / "shader.json").is_file()]
    if missing:
        write_json(batch / "download-pending.json", missing)
        raise RuntimeError(f"{len(missing)} source downloads remain; rerun with --resume {name}")
    return ids


def old_ids(show: dict) -> set[str]:
    result = set()
    for shader in show.get("shaders", []):
        stem = Path(shader.get("path", "")).stem
        if ID_BARE.fullmatch(stem):
            result.add(stem)
    return result


def used_channels(code: str) -> set[int]:
    code = re.sub(r"/\*.*?\*/|//[^\n]*", "", code, flags=re.S)
    return {int(x) for x in re.findall(r"\biChannel([0-3])\b", code)}


def image_and_common(document):
    passes = document.get("renderpass") or []
    image = next((p for p in passes if p.get("type") == "image"), None)
    if image is None:
        raise ValueError("no Image pass")
    code = "\n".join(p.get("code", "") for p in passes
                     if p.get("type") == "common" or p.get("name") == "Common") + "\n" + image.get("code", "")
    return image, code


def channels_for(sid: str, document: dict, assets_dir: Path, shared: dict, copied: set[str]):
    image, code = image_and_common(document)
    used = used_channels(code)
    by_channel = {row.get("channel"): row for row in image.get("inputs", [])}
    if not used:
        return []
    sys.path.insert(0, str(TOOLS / "shader_bank"))
    from common import binding_files, channel_bindings  # noqa: PLC0415
    shared_bindings = {row["channel"]: row for row in channel_bindings(shared, sid)[0]}
    result = []
    for number in sorted(used):
        row = by_channel.get(number)
        if row is None:
            raise ValueError(f"iChannel{number} has no declared input")
        input_type = row.get("type") or row.get("ctype")
        if input_type not in FILE_TYPES:
            raise ValueError(f"iChannel{number} requires unsupported {input_type or 'unknown'} input")
        binding = shared_bindings.get(number)
        if binding:
            files = binding_files(binding)
            source = ASSET_STORE
            kind = binding["kind"]
            filter_name, wrap = binding["filter"], binding["wrap"]
            vflip = binding["vflip"]
        else:
            filename = Path(row.get("filepath") or "").name
            if not filename:
                raise ValueError(f"iChannel{number} has no asset filename")
            kind = FILE_TYPES[input_type]
            binding = {"file": filename, "kind": kind}
            files = binding_files(binding)
            source = OFFLINE / sid
            sampler = row.get("sampler") or {}
            filter_name = sampler.get("filter", "mipmap")
            wrap = sampler.get("wrap", "repeat")
            vflip = str(sampler.get("vflip", "true")).lower() == "true"
        for filename in files:
            origin = source / filename
            if not origin.is_file() and source != ASSET_STORE:
                # The shared bank may already hold the five extra cubemap
                # faces even when the original per-shader archive has only
                # face zero. They are content-addressed by this filename.
                origin = ASSET_STORE / filename
            if not origin.is_file() or origin.stat().st_size == 0:
                raise ValueError(f"iChannel{number} asset missing: {filename}")
            dest = assets_dir / filename
            if filename not in copied:
                dest.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(origin, dest)
                copied.add(filename)
        result.append({"channel": number, "path": f"assets/{binding['file']}", "kind": kind,
                       "filter": filter_name, "wrap": wrap, "vflip": vflip})
    return result


def build_candidates(ids: list[str], batch: Path, existing: set[str]):
    sys.path.insert(0, str(TOOLS))
    from make_native_shadertoy import convert  # noqa: PLC0415
    package = batch / "package"
    native, fallback, assets = package / "native", package / "fallback", package / "assets"
    for directory in (native, fallback, assets):
        directory.mkdir(parents=True, exist_ok=True)
    shared_path = ASSET_STORE / "manifest.json"
    shared = read_json(shared_path) if shared_path.is_file() else {}
    entries, rejected, copied = [], {}, set()
    for sid in ids:
        if sid in existing:
            continue
        try:
            document = read_json(OFFLINE / sid / "shader.json")
            channels = channels_for(sid, document, assets, shared, copied)
            primary, label = convert(sid, document=document, wrapper="react")
            alternate, _ = convert(sid, document=document, wrapper="legacy")
            (native / f"{sid}.fs").write_text(primary, encoding="utf-8")
            (fallback / f"{sid}.fs").write_text(alternate, encoding="utf-8")
            entry = {"label": f"{sid} {label}", "path": f"native/{sid}.fs",
                     "blend": "Additive", "opacity": 0.9}
            if channels:
                entry["channels"] = channels
            entries.append(entry)
        except (OSError, ValueError, KeyError, IndexError) as error:
            rejected[sid] = str(error)
    write_json(package / "candidate-show.json", {
        "version": 2, "bpm": 128, "audio_device": None, "clips": [], "pages": [], "shaders": entries,
    })
    write_json(package / "ids.json", [Path(row["path"]).stem for row in entries])
    write_json(batch / "excluded.json", rejected)
    print(f"CANDIDATES {len(entries)}; already live {len(set(ids) & existing)}; unsupported {len(rejected)}", flush=True)
    return package, entries


def fingerprint(package: Path) -> str:
    digest = hashlib.sha256()
    helpers = {"report.json", "run-benchmark.sh", "benchmark_native_shaders.py",
               "pi_bench.py", "manifest.json"}
    for path in sorted(p for p in package.rglob("*") if p.is_file() and p.name not in helpers):
        digest.update(path.relative_to(package).as_posix().encode())
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()


def native_manifest(package: Path, remote: str) -> dict:
    candidates = read_json(package / "candidate-show.json")["shaders"]
    entries = []
    for candidate in candidates:
        sid = Path(candidate["path"]).stem
        for variant, folder in (("react", "native"), ("legacy", "fallback")):
            channels = copy.deepcopy(candidate.get("channels", []))
            for channel in channels:
                channel["path"] = f"{remote}/{channel['path']}"
            entries.append({"id": sid, "variant": variant,
                            "path": f"{remote}/{folder}/{sid}.fs", "channels": channels})
    return {"version": 1, "out": f"{remote}/results.jsonl",
            "thumb_dir": f"{remote}/thumbs", "tiers": [[1280, 720], [1920, 1080]],
            "entries": entries}


def normalize_native_results(package: Path, results_path: Path) -> dict:
    candidates = read_json(package / "candidate-show.json")["shaders"]
    measured = {}
    for line in results_path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("event") == "result":
            measured[(event.get("id"), event.get("variant"))] = event
    expected = {(Path(row["path"]).stem, variant)
                for row in candidates for variant in ("react", "legacy")}
    if set(measured) != expected:
        missing = expected - set(measured)
        raise RuntimeError(f"Pi benchmark incomplete: {len(missing)} variants unmeasured; resume this batch")

    def tier(event: dict, height: int) -> dict:
        beat = event.get("tiers", {}).get(str(height), {})
        loud = event.get("tiers_max", {}).get(str(height), {})
        pixels = event.get("pixels") or {}
        good = (event.get("status") == "ok" and
                all(isinstance(t.get("p95_ms"), (int, float)) and
                    t.get("frames", 0) >= 3 and t["p95_ms"] <= 13.0
                    for t in (beat, loud)) and
                isinstance(pixels.get("mean_luma"), (int, float)) and
                pixels["mean_luma"] > 0.02 and
                isinstance(pixels.get("spatial_std"), (int, float)) and
                pixels["spatial_std"] > 0.01)
        return {"status": event.get("status"), "p95_ms": beat.get("p95_ms"),
                "p95_ms_max": loud.get("p95_ms"), "suitable_60fps": good,
                "error": event.get("error")}

    rows = []
    for candidate in candidates:
        sid = Path(candidate["path"]).stem
        row = {"shader": sid}
        for variant, folder in (("react", "native"), ("legacy", "fallback")):
            event = measured[(sid, variant)]
            target = row if variant == "react" else row.setdefault("fallback", {})
            target["source_sha256"] = hashlib.sha256(
                (package / folder / f"{sid}.fs").read_bytes()).hexdigest()
            for height in (720, 1080):
                target[f"{height}p"] = tier(event, height)
        rows.append(row)
    return {"method": "native-rust-single-session", "results": rows}


def benchmark(package: Path, batch: Path, name: str, args):
    report = batch / "pi-benchmark.json"
    stamp = batch / "benchmark-fingerprint.txt"
    # Helper files are generated below and are not shader inputs. Excluding
    # them keeps a resumed batch from needlessly repeating a long Pi run.
    current = fingerprint(package)
    if report.is_file() and stamp.is_file() and stamp.read_text().strip() == current and \
            read_json(report).get("method") == "native-rust-single-session":
        print("BENCHMARK_REUSED: inputs match the recorded Pi run", flush=True)
        return read_json(report)
    archive = batch / f"rpi-vj-{name}.tar"
    # A changed shader or asset must never reuse results from an older run.
    remote = f"/srv/rpi-vj-batch-bench/{name}-{current[:12]}"
    write_json(package / "manifest.json", native_manifest(package, remote))
    shutil.copy2(TOOLS / "pi_bench.py", package / "pi_bench.py")
    with tarfile.open(archive, "w") as tar:
        for path in package.rglob("*"):
            if path.is_file():
                tar.add(path, arcname=path.relative_to(package).as_posix())
    host = f"{args.user}@{args.host}"
    key = ["-i", str(Path(args.identity).resolve())] if args.identity else []
    run(["scp", *key, str(archive), f"{host}:/tmp/rpi-vj-{name}.tar"])
    remote_command = (f"sudo sh -c 'mkdir -p {remote} && "
                      f"tar -xf /tmp/rpi-vj-{name}.tar -C {remote} && "
                      f"python3 {remote}/pi_bench.py {remote}/manifest.json "
                      f"--bin /opt/rpi-vj-native/rpi-vj-native "
                      f"--stop-services --restart-services --engine-log {remote}/engine.log'")
    outcome = run(["ssh", *key, "-tt", host, remote_command], check=False)
    native_results = batch / "pi-native-results.jsonl"
    run(["scp", *key, f"{host}:{remote}/results.jsonl", str(native_results)])
    if outcome.returncode:
        raise RuntimeError("Pi native benchmark stopped; results were saved and a rerun will resume")
    healthy = run(["ssh", *key, host,
                   "systemctl is-active --quiet rpi-vj-native && "
                   "systemctl is-active --quiet rpi-vj-controller"], check=False)
    if healthy.returncode:
        raise RuntimeError("Pi live services are not both active after benchmarking; refusing deployment")
    result = normalize_native_results(package, native_results)
    write_json(report, result)
    stamp.write_text(current + "\n", encoding="utf-8")
    return result


def chosen_tier(record: dict):
    for height in (1080, 720):
        for variant in (record, record.get("fallback") or {}):
            if variant.get(f"{height}p", {}).get("suitable_60fps") is True:
                return height, variant is not record
    return None


def merge_show(current: dict, package: Path, report: dict, name: str, batch: Path):
    merged = copy.deepcopy(current)
    existing = old_ids(current)
    candidates = read_json(package / "candidate-show.json")["shaders"]
    by_id = {Path(row["path"]).stem: row for row in candidates}
    staged = batch / "deploy"
    shaders_out = staged / "batches" / name
    assets_out = shaders_out / "assets"
    shaders_out.mkdir(parents=True, exist_ok=True)
    assets_out.mkdir(parents=True, exist_ok=True)
    sys.path.insert(0, str(TOOLS / "shader_bank"))
    from common import binding_files  # noqa: PLC0415
    selected, rejected = [], {}
    for record in report.get("results", []):
        sid = record["shader"]
        if sid in existing or sid not in by_id:
            continue
        tier = chosen_tier(record)
        if tier is None:
            rejected[sid] = {"1080p": record.get("1080p"), "720p": record.get("720p"),
                             "fallback": record.get("fallback")}
            continue
        height, use_fallback = tier
        shader_file = package / ("fallback" if use_fallback else "native") / f"{sid}.fs"
        expected = (record.get("fallback") if use_fallback else record).get("source_sha256")
        if not shader_file.is_file() or hashlib.sha256(shader_file.read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"Benchmark source hash differs from packaged shader: {sid}")
        shutil.copy2(shader_file, shaders_out / shader_file.name)
        source_row = by_id[sid]
        live_row = {"label": source_row["label"], "path": f"batches/{name}/{sid}.fs",
                    "blend": "Additive", "opacity": 0.9, "render_height": height}
        if source_row.get("channels"):
            channels = copy.deepcopy(source_row["channels"])
            for channel in channels:
                asset_name = Path(channel["path"]).name
                for filename in binding_files({"file": asset_name, "kind": channel["kind"]}):
                    source_asset = package / "assets" / filename
                    if not source_asset.is_file():
                        raise RuntimeError(f"Missing qualified asset: {filename}")
                    shutil.copy2(source_asset, assets_out / filename)
                channel["path"] = f"batches/{name}/assets/{asset_name}"
            live_row["channels"] = channels
        index = len(merged["shaders"])
        merged["shaders"].append(live_row)
        selected.append({"id": sid, "index": index, "height": height, "fallback": use_fallback})
    for start in range(0, len(selected), 24):
        bindings = [{"key": key, "label": f"#{row['index'] + 1:03d}",
                     "command": {"type": "TriggerShader", "shader": row["index"]},
                     "color": [25, 75, 120]}
                    for key, row in enumerate(selected[start:start + 24])]
        merged["pages"].append({"name": f"SHADERS {len(merged['pages']) + 1}", "bindings": bindings})
    if merged["pages"][:len(current["pages"])] != current["pages"] or merged["shaders"][:len(current["shaders"])] != current["shaders"]:
        raise RuntimeError("Append invariant failed: an existing page or shader changed")
    write_json(staged / "show.json", merged)
    write_json(batch / "qualified.json", selected)
    write_json(batch / "benchmark-rejected.json", rejected)
    print(f"APPEND_READY {len(selected)} shaders, {len(current['pages'])} -> {len(merged['pages'])} pages", flush=True)
    return staged, selected


def fetch_live_show(batch: Path, args) -> dict:
    snapshot = batch / "live-show.json"
    host = f"{args.user}@{args.host}"
    key = ["-i", str(Path(args.identity).resolve())] if args.identity else []
    run(["scp", *key, f"{host}:{SHOW_REMOTE}", str(snapshot)])
    live = read_json(snapshot)
    if not isinstance(live.get("pages"), list) or not isinstance(live.get("shaders"), list):
        raise RuntimeError("Pi show has no pages/shaders arrays")
    print(f"PI_LIVE {len(live['pages'])} pages, {len(live['shaders'])} shaders", flush=True)
    return live


def deploy(staged: Path, batch: Path, name: str, before: dict, args) -> None:
    expected = hashlib.sha256((batch / "live-show.json").read_bytes()).hexdigest()
    after = read_json(staged / "show.json")
    if len(after["pages"]) <= len(before["pages"]):
        raise RuntimeError("No new pages to deploy")
    archive = batch / f"rpi-vj-append-{name}.tar"
    with tarfile.open(archive, "w") as tar:
        for path in staged.rglob("*"):
            if path.is_file():
                tar.add(path, arcname=path.relative_to(staged).as_posix())
    # Shell arguments here contain only a validated batch slug and a SHA-256.
    script = batch / "install-append.sh"
    script.write_text(
        "#!/bin/sh\nset -eu\n"
        f"ROOT=/srv/rpi-vj-native\nARCHIVE=/tmp/rpi-vj-append-{name}.tar\n"
        f"EXPECTED={expected}\n"
        'ACTUAL=$(sha256sum "$ROOT/show.json" | cut -d " " -f 1)\n'
        '[ "$ACTUAL" = "$EXPECTED" ] || { echo "LIVE_SHOW_CHANGED; refusing to overwrite"; exit 4; }\n'
        'TMP=$(mktemp -d "$ROOT/.append-XXXXXX")\n'
        'trap \'rm -rf "$TMP"\' EXIT\n'
        'tar -xf "$ARCHIVE" -C "$TMP"\n'
        'python3 - "$ROOT/show.json" "$TMP/show.json" <<\'PY\'\n'
        'import json,sys\n'
        'a=json.load(open(sys.argv[1])); b=json.load(open(sys.argv[2]))\n'
        'assert b["pages"][:len(a["pages"])] == a["pages"]\n'
        'assert b["shaders"][:len(a["shaders"])] == a["shaders"]\n'
        'assert len(b["pages"]) > len(a["pages"])\n'
        'print("VALIDATED",len(a["pages"]),"->",len(b["pages"]),flush=True)\n'
        'PY\n'
        f'mkdir -p "$ROOT/batches/{name}"\n'
        f'cp -a "$TMP/batches/{name}/." "$ROOT/batches/{name}/"\n'
        'BACKUP="$ROOT/show.json.before-append-$(date +%Y%m%d-%H%M%S)"\n'
        'cp "$ROOT/show.json" "$BACKUP"\n'
        'cp "$TMP/show.json" "$ROOT/show.json.next"\n'
        'mv "$ROOT/show.json.next" "$ROOT/show.json"\n'
        'if ! systemctl restart rpi-vj-native rpi-vj-controller || '
        '! systemctl is-active --quiet rpi-vj-native || '
        '! systemctl is-active --quiet rpi-vj-controller; then\n'
        '  cp "$BACKUP" "$ROOT/show.json"\n'
        '  systemctl restart rpi-vj-native rpi-vj-controller || true\n'
        '  echo "ROLLED_BACK: service failed"\n'
        '  exit 5\n'
        'fi\n'
        'echo "APPEND_DEPLOYED backup=$BACKUP"\n', encoding="utf-8", newline="\n")
    host = f"{args.user}@{args.host}"
    key = ["-i", str(Path(args.identity).resolve())] if args.identity else []
    run(["scp", *key, str(archive), str(script),
         f"{host}:/tmp/"])
    remote_script = f"/tmp/{script.name}"
    run(["ssh", *key, "-tt", host, f"sudo sh {remote_script}"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    src = parser.add_mutually_exclusive_group()
    src.add_argument("--profile")
    src.add_argument("--file", type=Path)
    src.add_argument("--url", action="append", help="Shadertoy URL or bare ID; repeatable")
    src.add_argument("--resume", metavar="BATCH")
    parser.add_argument("--batch", help="batch name; inferred from profile/file when omitted")
    parser.add_argument("--host", default="rpi-vj3.local")
    parser.add_argument("--user", default="rpi")
    parser.add_argument("--identity", help="SSH identity file")
    parser.add_argument("--seconds", type=int, default=8,
                        help="deprecated compatibility option; the Rust GPU benchmark uses fixed measurement windows")
    parser.add_argument("--refresh-ids", action="store_true")
    parser.add_argument("--no-deploy", action="store_true", help="build and benchmark, but leave the Pi live show unchanged")
    args = parser.parse_args()
    try:
        mode, value, name = source_spec(args)
        batch = TARGET / name
        batch.mkdir(parents=True, exist_ok=True)
        write_json(batch / "input.json", {"mode": mode, "value": value, "name": name})
        print(f"BATCH {name}", flush=True)
        ids = imported_ids(mode, value, batch, name, args)
        live = fetch_live_show(batch, args)
        package, entries = build_candidates(ids, batch, old_ids(live))
        missing_assets = [sid for sid, reason in read_json(batch / "excluded.json").items()
                          if "asset missing" in reason]
        if missing_assets:
            retry_file = batch / "retry-assets.txt"
            retry_file.write_text("\n".join(missing_assets) + "\n", encoding="utf-8")
            print(f"ASSET_RECOVERY {len(missing_assets)} incomplete shaders", flush=True)
            run([sys.executable, str(TOOLS / "fetch_shadertoy_user.py"),
                 "--ids-file", str(retry_file), "--name", name + "-assets"], check=False)
            package, entries = build_candidates(ids, batch, old_ids(live))
        if not entries:
            print("NO_NEW_SUPPORTED_SHADERS; live show unchanged", flush=True)
            return 0
        report = benchmark(package, batch, name, args)
        # Refresh after a long benchmark so an intervening page edit is kept.
        live = fetch_live_show(batch, args)
        staged, selected = merge_show(live, package, report, name, batch)
        if not selected:
            print("NO_PI_QUALIFIED_SHADERS; live show unchanged", flush=True)
            return 0
        if args.no_deploy:
            print(f"READY_FOR_DEPLOY {staged}", flush=True)
            return 0
        deploy(staged, batch, name, live, args)
        verified = fetch_live_show(batch, args)
        expected = len(read_json(staged / "show.json")["pages"])
        if len(verified["pages"]) != expected:
            raise RuntimeError(f"Post-deploy verification failed: expected {expected} pages")
        print(f"COMPLETE {name}: {len(selected)} shaders appended; {expected} pages live", flush=True)
        return 0
    except (OSError, ValueError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"PIPELINE_STOPPED: {error}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
