#!/usr/bin/env python3
"""Build a disposable Stream Deck shader-review show from the first library."""
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "target" / "native-13-shaders"
OUT = Path(os.environ.get("REVIEW_OUT", str(ROOT / "target" / "shader-review")))
SOURCE_SHOW = SRC / "show-fixed-first-latest.json"
PI_BENCH_REPORT = os.environ.get("PI_BENCH_REPORT")
PI_BENCH_RESOLUTION = os.environ.get("PI_BENCH_RESOLUTION", "both")

def group(path: Path) -> int:
    text = path.read_text(errors="ignore").lower()
    if any(x in text for x in ("mandelbrot", "fractal", "julia", "raymarch", "distance field")):
        return 0
    if any(x in text for x in ("noise", "plasma", "cloud", "fire", "water", "liquid", "smoke")):
        return 1
    if any(x in text for x in ("kaleid", "symmetr", "tunnel", "vortex", "spiral", "warp")):
        return 2
    return 3

def main():
    OUT.mkdir(parents=True, exist_ok=True)
    shaders = sorted(SRC.glob("*.fs"), key=lambda p: p.stat().st_mtime, reverse=True)
    if PI_BENCH_REPORT:
        report = json.loads(Path(PI_BENCH_REPORT).read_text(encoding="utf-8"))
        def qualifies(item):
            at_1080 = item.get("1080p", {}).get("suitable_60fps") is True
            at_720 = item.get("720p", {}).get("suitable_60fps") is True
            return (at_1080 and at_720 if PI_BENCH_RESOLUTION == "both"
                    else at_720 if PI_BENCH_RESOLUTION == "720"
                    else at_1080)
        qualified = {item["shader"] for item in report["results"] if qualifies(item)}
        shaders = [p for p in shaders if p.stem in qualified]
    source_index = {item["path"].rsplit(".", 1)[0]: index + 1
                    for index, item in enumerate(json.loads(SOURCE_SHOW.read_text(encoding="utf-8"))["shaders"])}
    groups = [[] for _ in range(4)]
    for p in shaders:
        groups[group(p)].append(p)
    ordered = [p for g in groups for p in g]
    pages = []
    for start in range(0, len(ordered), 24):
        bindings = []
        for key, shader in enumerate(ordered[start:start + 24]):
            label = f"#{source_index[shader.stem]:03d}"
            bindings.append({"key": key, "label": label, "command": {"type": "TriggerShader", "shader": ordered.index(shader)}, "color": [25, 75, 120]})
        bindings += [
            {"key": 24, "label": "PREV PAGE", "command": {"type": "Page", "delta": -1}, "color": [25, 90, 115]},
            {"key": 25, "label": "NEXT PAGE", "command": {"type": "Page", "delta": 1}, "color": [25, 90, 115]},
            {"key": 26, "label": "CRATE 1", "command": {"type": "CrateShader", "crate_id": 0}, "color": [40, 120, 70]},
            {"key": 27, "label": "CRATE 2", "command": {"type": "CrateShader", "crate_id": 1}, "color": [55, 100, 150]},
            {"key": 28, "label": "CRATE 3", "command": {"type": "CrateShader", "crate_id": 2}, "color": [140, 90, 35]},
            {"key": 29, "label": "CRATE 4", "command": {"type": "CrateShader", "crate_id": 3}, "color": [130, 50, 100]},
            {"key": 30, "label": "DELETE", "command": {"type": "DeleteShader"}, "color": [150, 25, 25]},
            {"key": 31, "label": "BLACK / PAGE", "command": {"type": "Blackout", "enabled": True}, "color": [70, 15, 15]},
        ]
        pages.append({"name": f"SHADERS {start // 24 + 1:02d} / {((len(ordered)-1)//24)+1:02d}", "bindings": bindings})
    manifest = {"version": 2, "bpm": 128, "audio_device": None,
                "clips": [{"label": "NO VIDEO", "path": "no-video.mp4", "thumbnail": None, "quad_mirror": False}],
                "shaders": [{"label": p.stem, "path": p.name, "blend": "Additive", "opacity": 0.9} for p in ordered], "pages": pages}
    (OUT / "show.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    for p in ordered:
        target = OUT / p.name
        target.write_bytes(p.read_bytes())
    (OUT / "groups.json").write_text(json.dumps({f"crate_{i+1}": [{"index": source_index[p.stem], "shader": p.stem} for p in g] for i, g in enumerate(groups)}, indent=2), encoding="utf-8")
    print(f"SHADER_REVIEW {len(ordered)} shaders, {len(pages)} pages")

if __name__ == "__main__": main()
