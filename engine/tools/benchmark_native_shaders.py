#!/usr/bin/env python3
"""Benchmark each native shader at 1080p and 720p on an idle Pi.

Run on the Pi only after the review renderer has finished its initial compile:
  sudo systemctl stop rpi-vj-native rpi-vj-controller
  python3 benchmark_native_shaders.py --i-understand-idle
  sudo systemctl start rpi-vj-native rpi-vj-controller
"""
import argparse, json, os, re, select, subprocess, sys, time
from pathlib import Path

ROOT = Path("/srv/rpi-vj-native")
BIN = Path("/opt/rpi-vj-native/rpi-vj-native")
OUT = Path("/var/lib/rpi-vj-native/shader-bench.json")
FRAME = re.compile(r"renderer frames=(\d+) late=(\d+)")

def active(name):
    return subprocess.run(["systemctl", "is-active", "--quiet", name]).returncode == 0

def one(shader, size, seconds):
    manifest = ROOT / ".bench-show.json"
    manifest.write_text(json.dumps({"version": 2, "bpm": 128, "audio_device": None,
        "clips": [], "pages": [], "shaders": [{"label": shader.stem, "path": str(shader), "blend": "Additive", "opacity": 1.0}]}))
    env = {**os.environ, "SDL_VIDEODRIVER": "kmsdrm", "SDL_AUDIODRIVER": "dummy",
           "RPI_VJ_RENDER_WIDTH": str(size[0]), "RPI_VJ_RENDER_HEIGHT": str(size[1])}
    proc = subprocess.Popen([str(BIN), str(manifest), "--allow-software"], stderr=subprocess.PIPE,
                            stdout=subprocess.DEVNULL, text=True, env=env)
    samples, start = [], time.monotonic()
    try:
        while time.monotonic() - start < seconds:
            # Some valid shaders do not produce a periodic renderer status line.
            # Never let an empty stderr pipe stall the entire library benchmark.
            readable, _, _ = select.select([proc.stderr], [], [], 0.2)
            if readable:
                line = proc.stderr.readline()
                hit = FRAME.search(line)
                if hit: samples.append((time.monotonic(), int(hit.group(1)), int(hit.group(2))))
            if proc.poll() is not None: break
    finally:
        proc.terminate()
        try: proc.wait(timeout=5)
        except subprocess.TimeoutExpired: proc.kill()
    if len(samples) < 2: return {"status": "no_frame_sample"}
    first, last = samples[0], samples[-1]
    elapsed = last[0] - first[0]
    fps = (last[1] - first[1]) / elapsed if elapsed else 0
    return {"status": "ok", "fps": round(fps, 2), "late_frames": last[2] - first[2],
            "samples": len(samples), "suitable_60fps": fps >= 59.0 and last[2] == first[2]}

def main():
    ap = argparse.ArgumentParser(); ap.add_argument("--i-understand-idle", action="store_true")
    ap.add_argument("--seconds", type=int, default=8); args = ap.parse_args()
    if not args.i_understand_idle: raise SystemExit("refusing: pass --i-understand-idle after stopping native services")
    if active("rpi-vj-native") or active("rpi-vj-controller"): raise SystemExit("refusing: native services are active")
    if not BIN.is_file(): raise SystemExit(f"missing {BIN}")
    results = []
    shaders = sorted(ROOT.glob("*.fs"))
    for number, shader in enumerate(shaders, 1):
        entry = {"index": number, "shader": shader.stem,
                 "1080p": one(shader, (1920, 1080), args.seconds),
                 "720p": one(shader, (1280, 720), args.seconds)}
        results.append(entry); print(json.dumps(entry), flush=True)
    report = {"generated_at": time.time(), "seconds_per_test": args.seconds, "results": results}
    OUT.write_text(json.dumps(report, indent=2)); print(f"REPORT={OUT}")
if __name__ == "__main__": main()
