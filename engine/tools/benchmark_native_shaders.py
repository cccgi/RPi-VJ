#!/usr/bin/env python3
"""Benchmark each native shader at 1080p and 720p on an idle Pi.

Run on the Pi only after the review renderer has finished its initial compile:
  sudo systemctl stop rpi-vj-native rpi-vj-controller
  python3 benchmark_native_shaders.py --i-understand-idle
  sudo systemctl start rpi-vj-native rpi-vj-controller
"""
import argparse, hashlib, json, os, re, select, subprocess, sys, time
from pathlib import Path

ROOT = Path("/srv/rpi-vj-native")
BIN = Path("/opt/rpi-vj-native/rpi-vj-native")
OUT = Path("/var/lib/rpi-vj-native/shader-bench.json")
FRAME = re.compile(r"renderer frames=(\d+) late=(\d+)")

def active(name):
    return subprocess.run(["systemctl", "is-active", "--quiet", name]).returncode == 0

def one(shader, size, seconds, workdir, entry=None):
    manifest = workdir / ".bench-show.json"
    shader_entry = dict(entry or {"label": shader.stem, "path": str(shader), "blend": "Additive", "opacity": 1.0})
    shader_entry["path"] = str(shader)
    shader_entry["render_height"] = size[1]
    for channel in shader_entry.get("channels", []):
        channel["path"] = str((workdir / channel["path"]).resolve()) if not Path(channel["path"]).is_absolute() else channel["path"]
    manifest.write_text(json.dumps({"version": 2, "bpm": 128, "audio_device": None,
        "clips": [], "pages": [], "shaders": [shader_entry]}))
    env = {**os.environ, "SDL_VIDEODRIVER": "kmsdrm", "SDL_AUDIODRIVER": "dummy",
           "RPI_VJ_RENDER_WIDTH": str(size[0]), "RPI_VJ_RENDER_HEIGHT": str(size[1])}
    proc = subprocess.Popen([str(BIN), str(manifest), "--allow-software"], stderr=subprocess.PIPE,
                            stdout=subprocess.DEVNULL, text=True, env=env)
    samples, errors, start = [], [], time.monotonic()
    try:
        while time.monotonic() - start < seconds:
            # Some valid shaders do not produce a periodic renderer status line.
            # Never let an empty stderr pipe stall the entire library benchmark.
            readable, _, _ = select.select([proc.stderr], [], [], 0.2)
            if readable:
                line = proc.stderr.readline()
                if any(term in line.lower() for term in ("error", "disabled", "missing texture", "invalid channel")):
                    errors.append(line.strip()[:300])
                hit = FRAME.search(line)
                if hit: samples.append((time.monotonic(), int(hit.group(1)), int(hit.group(2))))
            if proc.poll() is not None: break
    finally:
        proc.terminate()
        try: proc.wait(timeout=5)
        except subprocess.TimeoutExpired: proc.kill()
    if errors:
        return {"status": "shader_error", "diagnostic": errors[-3:],
                "samples": len(samples), "suitable_60fps": False}
    if len(samples) < 2:
        return {"status": "no_frame_sample", "exit_code": proc.returncode,
                "diagnostic": errors[-3:]}
    first, last = samples[0], samples[-1]
    elapsed = last[0] - first[0]
    fps = (last[1] - first[1]) / elapsed if elapsed else 0
    return {"status": "ok", "fps": round(fps, 2), "late_frames": last[2] - first[2],
            "samples": len(samples), "suitable_60fps": fps >= 59.0 and last[2] == first[2]}

def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--i-understand-idle", action="store_true")
    ap.add_argument("--seconds", type=int, default=8)
    ap.add_argument("--shader-list", type=Path,
                    help="optional text or JSON list of shader IDs/stems to test; omitted tests every .fs file")
    ap.add_argument("--out", type=Path, default=OUT, help="report destination")
    ap.add_argument("--shader-dir", type=Path, default=ROOT,
                    help="directory holding candidate .fs files (default: live shader directory)")
    ap.add_argument("--candidate-show", type=Path,
                    help="show containing each candidate's real texture channel bindings")
    ap.add_argument("--fallback-dir", type=Path,
                    help="alternate converted shaders, tested when the primary yields no frame samples")
    ap.add_argument("--input-fingerprint", help="hash of shader and asset inputs for safe resume")
    args = ap.parse_args()
    if not args.i_understand_idle: raise SystemExit("refusing: pass --i-understand-idle after stopping native services")
    if active("rpi-vj-native") or active("rpi-vj-controller"): raise SystemExit("refusing: native services are active")
    if not BIN.is_file(): raise SystemExit(f"missing {BIN}")
    results = []
    previous = {}
    if args.out.is_file():
        try:
            old_report = json.loads(args.out.read_text())
            if args.input_fingerprint and old_report.get("input_fingerprint") == args.input_fingerprint \
                    and old_report.get("seconds_per_test") == args.seconds:
                previous = {row["shader"]: row for row in old_report.get("results", [])}
        except (OSError, ValueError, KeyError, TypeError):
            previous = {}
    shader_dir = args.shader_dir.resolve()
    if not shader_dir.is_dir(): raise SystemExit(f"missing shader directory {shader_dir}")
    shaders = sorted(shader_dir.glob("*.fs"))
    candidate_root = args.candidate_show.resolve().parent if args.candidate_show else shader_dir
    candidates = {}
    if args.candidate_show:
        candidate_show = json.loads(args.candidate_show.read_text(encoding="utf-8"))
        candidates = {Path(row["path"]).stem: row for row in candidate_show.get("shaders", [])}
    if args.shader_list:
        try:
            selected = json.loads(args.shader_list.read_text())
            if not isinstance(selected, list): selected = [selected]
        except json.JSONDecodeError:
            selected = args.shader_list.read_text().splitlines()
        wanted = {Path(str(value).strip()).stem for value in selected if str(value).strip()}
        shaders = [shader for shader in shaders if shader.stem in wanted]
        unknown = sorted(wanted - {shader.stem for shader in shaders})
        if unknown: print("NOT_STAGED=" + " ".join(unknown), file=sys.stderr)
    for number, shader in enumerate(shaders, 1):
        candidate = candidates.get(shader.stem)
        source_hash = hashlib.sha256(shader.read_bytes()).hexdigest()
        old = previous.get(shader.stem)
        if old and old.get("source_sha256") == source_hash and "720p" in old and "1080p" in old:
            entry = old
            entry["index"] = number
            print(f"REUSE {number}/{len(shaders)} {shader.stem}", flush=True)
        else:
            entry = {"index": number, "shader": shader.stem,
                     "source_sha256": source_hash,
                     "1080p": one(shader, (1920, 1080), args.seconds, candidate_root, candidate),
                     "720p": one(shader, (1280, 720), args.seconds, candidate_root, candidate)}
        if args.fallback_dir and entry["720p"]["status"] != "ok":
            fallback = args.fallback_dir / shader.name
            fallback_hash = hashlib.sha256(fallback.read_bytes()).hexdigest() if fallback.is_file() else None
            if fallback.is_file() and (not entry.get("fallback") or entry["fallback"].get("source_sha256") != fallback_hash):
                entry["fallback"] = {"source_sha256": hashlib.sha256(fallback.read_bytes()).hexdigest(),
                                     "1080p": one(fallback, (1920, 1080), args.seconds, candidate_root, candidate),
                                     "720p": one(fallback, (1280, 720), args.seconds, candidate_root, candidate)}
        results.append(entry); print(json.dumps(entry), flush=True)
        report = {"generated_at": time.time(), "seconds_per_test": args.seconds,
                  "input_fingerprint": args.input_fingerprint,
                  "shader_list": str(args.shader_list) if args.shader_list else None,
                  "shader_dir": str(shader_dir), "results": results}
        args.out.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.out.with_suffix(args.out.suffix + ".next")
        temporary.write_text(json.dumps(report, indent=2))
        temporary.replace(args.out)
    print(f"REPORT={args.out}")
if __name__ == "__main__": main()
