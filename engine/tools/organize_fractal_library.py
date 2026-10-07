#!/usr/bin/env python3
"""Build the dedicated fractal library from crawled and existing shaders."""
import json
import re
import shutil
from pathlib import Path

ROOT = Path("E:/__Projects/RPi-VJ")
OFFLINE = ROOT / "target/shadertoy-offline"
DOWNLOADS = ROOT / "target/shadertoy-downloads"
LIB = ROOT / "target/shadertoy-fractals"
SOURCES = LIB / "sources"
TERMS = re.compile(r"\b(fractal|mandelbrot|mandelbulb|julia|ifs|iterated function system|kleinian|orbit trap|apollonian)\b", re.I)

def main():
    SOURCES.mkdir(parents=True, exist_ok=True)
    matches = []
    for source in sorted(DOWNLOADS.glob("*.glsl")):
        sid = source.stem
        meta_path = OFFLINE / sid / "shader.json"
        meta = {}
        if meta_path.exists():
            try: meta = json.loads(meta_path.read_text(encoding="utf-8"))
            except Exception: pass
        info = meta.get("info", {}) if isinstance(meta, dict) else {}
        haystack = " ".join([
            sid, str(info.get("name", "")), str(info.get("description", "")),
            " ".join(map(str, info.get("tags", []) or [])),
            source.read_text(encoding="utf-8", errors="ignore")[:120000],
        ])
        if not TERMS.search(haystack):
            continue
        target = SOURCES / source.name
        shutil.copy2(source, target)
        matches.append({"id": sid, "name": info.get("name", sid), "source": str(target.relative_to(LIB)).replace("\\", "/"), "canonical": str(source.relative_to(ROOT)).replace("\\", "/")})
    (LIB / "index.json").write_text(json.dumps({"query": "fractals", "count": len(matches), "shaders": matches}, indent=2), encoding="utf-8")
    print(f"FRACTAL_LIBRARY {len(matches)}")

if __name__ == "__main__": main()
