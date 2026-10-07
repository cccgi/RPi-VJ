#!/usr/bin/env python3
"""Generate SYNTHETIC bench results from the catalog, to exercise report.py.

    python engine/tools/shader_bank/synth_results.py
    python engine/tools/shader_bank/report.py --synthetic \
        target/shader-bank/synthetic/results.synthetic.jsonl \
        target/shader-bank/synthetic/results.synthetic-lowtiers.jsonl

The numbers are pseudo-random (seeded by shader id, loosely shaped by the
static cost features) and mean nothing about any real shader. Every line
carries `"synthetic": true` so report.py refuses to treat the file as real.
The files cover every status value of plan section 3.2, a skipped tier, a
re-measured key (later line wins), a torn last line and a second file with
540/360 tiers.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import (ALL_STATUSES, BANK_OUT, CLASS_MULTIPASS, CLASS_ORIGINAL, VARIANTS_DIR, load_catalog, read_json, rel)  # noqa: E402

OUT_DIR = BANK_OUT / "synthetic"
ERROR_TEXT = {
    "prepare_error": "unsupported ISF PASSES; single-pass generators only",
    "compile_error": "0:41(12): error: could not implicitly convert operands to arithmetic operator",
    "too_slow": "first 720p frame took 2140 ms (limit 1500 ms)",
    "gl_error": "GL_OUT_OF_MEMORY after draw",
    "crash": "bench exited with signal 11 while measuring this entry",
    "hang": "no output for 60 s; bench killed",
}


def _rng(*parts) -> random.Random:
    return random.Random(int.from_bytes(hashlib.sha256("/".join(map(str, parts)).encode()).digest()[:8], "big"))


def tier_stats(base_ms: float, rng: random.Random) -> dict:
    p50 = round(base_ms * rng.uniform(0.97, 1.03), 2)
    p95 = round(p50 * rng.uniform(1.04, 1.25), 2)
    frames = 90 if p50 < 27 else max(3, int(2500 / p50))
    return {"frames": frames, "mean_ms": round((p50 + p95) / 2.05, 2), "p50_ms": p50, "p95_ms": p95,
            "max_ms": round(p95 * rng.uniform(1.0, 1.4), 2), "first_ms": round(p95 * rng.uniform(1.5, 4.0), 2)}


def synth_entry(record: dict, variant: str, tiers, forced_status=None, lite_gain: float = 1.0) -> dict:
    rng = _rng(record["id"], variant)
    features = record["features"]
    if record["class"] == CLASS_ORIGINAL:
        cost_720 = rng.uniform(1.2, 4.2)
    else:
        weight = math.log10(max(10, features["est_iter_product"])) + 0.15 * features["texture_fetches"]
        cost_720 = min(400.0, 0.9 * math.exp(rng.gauss(0.55 * weight, 0.75)))
    cost_720 *= lite_gain
    status = forced_status
    if status is None:
        roll = _rng(record["id"], "status").random()
        status = "ok"
        if record["class"] != CLASS_ORIGINAL:
            for name, share in (("compile_error", 0.04), ("prepare_error", 0.008), ("too_slow", 0.015),
                                ("gl_error", 0.006), ("crash", 0.008), ("hang", 0.008)):
                if roll < share:
                    status = name
                    break
                roll -= share
    event = {"event": "result", "id": record["id"], "variant": variant, "status": status, "synthetic": True}
    if status in ("prepare_error", "crash", "hang"):
        event["error"] = ERROR_TEXT[status]
    elif status == "compile_error":
        event.update(compile_ms=round(rng.uniform(40, 300), 1), error=ERROR_TEXT[status])
    else:
        event["compile_ms"] = round(rng.uniform(60, 180) + record["size_bytes"] / 40.0 * rng.uniform(0.5, 1.5), 1)
        measured = {}
        for _, height in tiers:
            measured[str(height)] = tier_stats(cost_720 * (height / 720.0) ** 2, _rng(record["id"], variant, height))
        if status == "too_slow":
            first = str(tiers[0][1])
            measured = {first: {"frames": 0, "first_ms": 2140.0}}
            measured.update({str(h): {"skipped": "too_slow at first tier"} for _, h in tiers[1:]})
            event["error"] = ERROR_TEXT[status]
        elif status == "gl_error":
            measured = {str(tiers[0][1]): measured[str(tiers[0][1])]}
            measured.update({str(h): {"skipped": "gl_error"} for _, h in tiers[1:]})
            event["error"] = ERROR_TEXT[status]
        event["tiers"] = measured
        if status == "ok":
            look = _rng(record["id"], "pixels").random()
            pixels = {"mean_luma": round(rng.uniform(0.08, 0.55), 3), "spatial_std": round(rng.uniform(0.05, 0.32), 3),
                      "temporal_diff": round(rng.uniform(0.01, 0.2), 4)}
            if record["class"] != CLASS_ORIGINAL:
                if look < 0.03:
                    pixels = {"mean_luma": 0.004, "spatial_std": 0.003, "temporal_diff": 0.0}
                elif look < 0.05:
                    pixels.update(spatial_std=0.004)
                elif look < 0.07:
                    pixels.update(temporal_diff=0.0002)
            event["pixels"] = pixels
            event["thumb"] = f"{record['id']}.{variant}.ppm"
    if status not in ("crash", "hang"):
        event["temp_c"] = round(rng.uniform(52, 71), 1)
        event["throttled"] = "0x0"
    return event


def generate(catalog: dict, lite_ids=frozenset(), unmeasured_share: float = 0.04) -> tuple[list, list]:
    """Return (main events for 720/1080, low-tier events for 540/360)."""
    main_tiers, low_tiers = [[1280, 720], [1920, 1080]], [[960, 540], [640, 360]]
    records = [r for r in catalog["shaders"] if r["class"] != CLASS_MULTIPASS]
    main, low = [], []
    # make sure every status appears at least once, on fixed non-original shaders
    forced = {}
    pool = [r for r in records if r["class"] != CLASS_ORIGINAL]
    for index, status in enumerate(ALL_STATUSES):
        if index < len(pool):
            forced[pool[index * 7 % len(pool)]["id"]] = status
    for record in records:
        if record["class"] != CLASS_ORIGINAL and record["id"] not in forced and _rng(record["id"], "skip").random() < unmeasured_share:
            continue
        event = synth_entry(record, "base", main_tiers, forced.get(record["id"]))
        main.append({"event": "begin", "id": record["id"], "variant": "base", "synthetic": True})
        main.append(event)
        if record["id"] in lite_ids:
            main.append({"event": "begin", "id": record["id"], "variant": "lite", "synthetic": True})
            main.append(synth_entry(record, "lite", main_tiers, lite_gain=0.3))
        if event["status"] == "ok" and event["tiers"]["720"]["p95_ms"] > 13.0 and _rng(record["id"], "low").random() < 0.8:
            low.append(synth_entry(record, "base", low_tiers, "ok"))
    # a key measured twice: the supervisor recorded a hang, a later resume succeeded
    retried = next((e for e in main if e.get("status") == "ok" and e["event"] == "result"), None)
    if retried:
        position = main.index(retried)
        main.insert(position, {"event": "result", "id": retried["id"], "variant": retried["variant"], "status": "hang",
                               "error": ERROR_TEXT["hang"], "synthetic": True})
    return main, low


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--catalog", type=Path, default=None)
    parser.add_argument("--out-dir", type=Path, default=OUT_DIR)
    args = parser.parse_args(argv)
    catalog = load_catalog(args.catalog) if args.catalog else load_catalog()
    lite_path = VARIANTS_DIR / "lite" / "transforms.json"
    lite_ids = set(read_json(lite_path).get("shaders", {})) if lite_path.is_file() else set()
    main_events, low_events = generate(catalog, lite_ids)
    args.out_dir.mkdir(parents=True, exist_ok=True)
    main_path = args.out_dir / "results.synthetic.jsonl"
    low_path = args.out_dir / "results.synthetic-lowtiers.jsonl"
    with main_path.open("w", encoding="utf-8", newline="\n") as handle:
        for event in main_events:
            handle.write(json.dumps(event, separators=(",", ":")) + "\n")
        handle.write('{"event":"result","id":"torn-line","variant":"base","status":"o')  # crash mid-write
    with low_path.open("w", encoding="utf-8", newline="\n") as handle:
        for event in low_events:
            handle.write(json.dumps(event, separators=(",", ":")) + "\n")
    results = [e for e in main_events if e["event"] == "result"]
    statuses = {s: sum(1 for e in results if e["status"] == s) for s in ALL_STATUSES}
    print(f"SYNTHETIC {len(results)} result lines -> {rel(main_path)}; {len(low_events)} low-tier lines -> {rel(low_path)}")
    print("  statuses: " + ", ".join(f"{k}={v}" for k, v in statuses.items()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
