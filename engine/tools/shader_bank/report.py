#!/usr/bin/env python3
"""Turn bench results (plan section 3.2) into documentation and a show manifest.

    python engine/tools/shader_bank/report.py target/shader-bank/results/results.jsonl [more.jsonl ...]

Reads the results files in the given order (later lines win per
(id, variant, tier)), joins the catalog, applies the R3 budgets, assigns each
shader its best variant and render tier and writes:

    docs/native/SHADER_BENCH_RESULTS.md       the results document
    target/shader-bank/qualified.json         machine-readable outcome
    target/shader-bank/show/                  show.json (+ shader files) with render_height per shader
    target/shader-bank/followup-*.txt         id lists for the next bench pass (make_manifest.py --ids-file)

With --synthetic every output gets a `.synthetic` name instead, so a dry run
can never overwrite or be mistaken for the real documents.
"""
from __future__ import annotations

import argparse
import collections
import statistics
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import (ASSET_MANIFEST_PATH, ASSETS_DIR, BANK_OUT, CATALOG_BINDING_KINDS, CLASS_MULTIPASS,  # noqa: E402
                    CLASS_ORIGINAL, CLASS_SELF, CLASS_TEXTURE, CLASSES, ROOT, VARIANTS_DIR, binding_files,
                    channel_bindings, iter_results, load_asset_manifest, load_catalog, manifest_channels,
                    merge_results, read_json, rel, write_bytes_if_changed, write_json)

# --- R3 budgets: shader pass p95 GPU ms at a tier -------------------------
SOLID_P95_MS = 10.0
OK_P95_MS = 13.0
MARGINAL_P95_MS = 16.0
MIN_FRAMES = 3                       # section 3.3: fewer frames is not a measurement
# --- pixel statistics: "renders but suspect" ------------------------------
BLACK_MEAN_LUMA_MAX = 0.02           # mean luma (0..1) at or below this is near-black
FLAT_SPATIAL_STD_MAX = 0.01          # spatial std at or below this is a flat field
STATIC_TEMPORAL_DIFF_MAX = 0.001     # mean abs temporal difference at or below this is a still image
# --- assignment -----------------------------------------------------------
ASSIGNABLE_TIERS = (1080, 720, 540, 360)     # plan R1, best first
MAIN_TIERS = (720, 1080)                     # always shown as columns
VARIANT_PREFERENCE = ("base", "tex", "lite", "pi")  # least modified first; ties go to the earlier one
GOOD_VERDICTS = ("SOLID", "OK")
# --- show manifest (engine/src/show.rs limits, build_shader_review_show.py layout)
SHOW_MAX_SHADERS = 1024
SHOW_MAX_PAGES = 128
KEYS_PER_PAGE = 24
SHOW_MARKER = ".shader-bank-show"
SHOW_ASSET_DIR = "assets"            # channel textures, relative to show.json

OUTCOMES = ("qualified", "suspect", "too_heavy", "failed", "unverified_texture", "excluded_multipass", "not_measured")
OUTCOME_TITLES = {
    "qualified": "qualified", "suspect": "renders but suspect", "too_heavy": "too heavy at measured tiers",
    "failed": "does not work", "unverified_texture": "needs texture (not measured as designed)",
    "excluded_multipass": "multipass (excluded)", "not_measured": "not measured",
}


def tier_verdict(data):
    """Verdict for one tier measurement; None when the tier is absent."""
    if data is None:
        return None
    if "skipped" in data:
        return "SKIPPED"
    p95, frames = data.get("p95_ms"), data.get("frames", MIN_FRAMES)
    if not isinstance(p95, (int, float)) or isinstance(p95, bool) or not isinstance(frames, (int, float)) or frames < MIN_FRAMES:
        return "NO_DATA"
    if p95 <= SOLID_P95_MS:
        return "SOLID"
    if p95 <= OK_P95_MS:
        return "OK"
    if p95 <= MARGINAL_P95_MS:
        return "MARGINAL"
    return "FAIL"


def pixel_flags(pixels) -> list:
    """Why a rendered frame is suspect. Missing statistics are themselves a flag:
    without them we cannot tell a working shader from a black frame (audit F3)."""
    if not isinstance(pixels, dict):
        return ["no_pixel_stats"]
    flags = []
    luma, spatial, temporal = pixels.get("mean_luma"), pixels.get("spatial_std"), pixels.get("temporal_diff")
    if not all(isinstance(v, (int, float)) for v in (luma, spatial, temporal)):
        return ["no_pixel_stats"]
    if luma <= BLACK_MEAN_LUMA_MAX:
        flags.append("black")
    if spatial <= FLAT_SPATIAL_STD_MAX:
        flags.append("flat")
    if temporal <= STATIC_TEMPORAL_DIFF_MAX:
        flags.append("static")
    return flags


def assess_variant(record: dict) -> dict:
    status = record.get("status", "ok")
    verdicts = {int(h): tier_verdict(d) for h, d in record.get("tiers", {}).items() if str(h).isdigit()}
    assigned = None
    if status == "ok":
        good = [h for h, v in verdicts.items() if v in GOOD_VERDICTS]
        assigned = max(good) if good else None
    return {"variant": record["variant"], "status": status, "error": record.get("error"),
            "channels": {str(c): k for c, k in (record.get("channels") or {}).items()},
            "tiers": {int(h): d for h, d in record.get("tiers", {}).items() if str(h).isdigit()},
            "verdicts": verdicts, "assigned_tier": assigned,
            "pixel_flags": pixel_flags(record.get("pixels")) if status == "ok" else [],
            "compile_ms": record.get("compile_ms"), "throttled": record.get("throttled"), "temp_c": record.get("temp_c")}


def _preference(variant: str) -> int:
    return VARIANT_PREFERENCE.index(variant) if variant in VARIANT_PREFERENCE else len(VARIANT_PREFERENCE)


def unbound_channels(catalog_record: dict, assessed_variant) -> list:
    """Channels the shader reads that were not bound as the catalog says they
    should be when this variant was measured, as "chN=kind" strings. Empty
    means the shader was measured as designed. A channel whose input is not a
    file (buffer, keyboard, audio, video, undeclared) can never be bound."""
    bound = (assessed_variant or {}).get("channels") or {}
    kinds = (catalog_record.get("channels") or {}).get("kinds") or {}
    return [f"ch{channel}={kind}" for channel, kind in sorted(kinds.items())
            if CATALOG_BINDING_KINDS.get(kind) is None or bound.get(str(channel)) != CATALOG_BINDING_KINDS[kind]]


def assess_shader(catalog_record: dict, variant_records: dict) -> dict:
    """Outcome, best variant and render tier for one shader.

    Best variant: among variants that run and reach a SOLID/OK tier, clean
    pixel statistics first, then the highest tier, then the least modified
    variant. `multipass` is never qualified whatever it measures, because the
    renderer draws only the Image pass. `needs_texture` is assessed like a
    self-contained shader over the variants that were measured with every
    channel the shader reads bound (the result line's `channels`); without
    such a measurement it stays unverified."""
    assessed = {v: assess_variant(r) for v, r in variant_records.items()}
    ordered = sorted(assessed.values(), key=lambda a: _preference(a["variant"]))
    klass = catalog_record["class"]
    everything = ordered
    if klass == CLASS_TEXTURE:
        # only measurements of the shader as designed count
        ordered = [a for a in ordered if not unbound_channels(catalog_record, a)]
    candidates = [a for a in ordered if a["assigned_tier"]]
    chosen = max(candidates, key=lambda a: (not a["pixel_flags"], a["assigned_tier"], -_preference(a["variant"]))) if candidates else None
    shown = chosen or next((a for a in ordered if a["status"] == "ok"), None) or (ordered[0] if ordered else None)
    reason = ""
    if klass == CLASS_MULTIPASS:
        outcome = "excluded_multipass"
        reason = "Image pass reads a buffer pass; the renderer draws only the Image pass"
    elif klass == CLASS_TEXTURE and not ordered:
        outcome = "unverified_texture"
        chosen = None
        shown = (next((a for a in everything if a["assigned_tier"]), None)
                 or next((a for a in everything if a["status"] == "ok"), None) or (everything[0] if everything else None))
        missing = unbound_channels(catalog_record, shown)
        unbindable = [m for m in missing if CATALOG_BINDING_KINDS.get(m.split("=", 1)[1]) is None]
        if unbindable:
            reason = "reads an input the renderer cannot supply (" + ", ".join(unbindable) + ")"
        elif everything:
            reason = "measured without its channel textures (" + ", ".join(missing) + ")"
        else:
            reason = "not measured (reads " + ", ".join(missing) + ")"
    elif not ordered:
        outcome = "not_measured"
    elif chosen and not chosen["pixel_flags"]:
        outcome = "qualified"
    elif chosen:
        outcome = "suspect"
        reason = "pixel statistics: " + ", ".join(chosen["pixel_flags"])
    elif all(a["status"] != "ok" for a in ordered):
        outcome = "failed"
        first = ordered[0]
        reason = first["status"] + (": " + str(first["error"]) if first["error"] else "")
    elif any(v in ("MARGINAL", "FAIL") for a in ordered for v in a["verdicts"].values()):
        outcome = "too_heavy"
        measured = sorted({h for a in ordered for h, v in a["verdicts"].items() if v in ("MARGINAL", "FAIL")})
        untried = [h for h in ASSIGNABLE_TIERS if h < min(measured) and all(h not in a["verdicts"] for a in ordered)]
        reason = "over budget at " + "/".join(f"{h}p" for h in measured)
        if untried:
            reason += "; not yet measured at " + "/".join(f"{h}p" for h in untried)
    else:
        outcome = "failed"
        reason = "no usable tier measurement"
    return {"id": catalog_record["id"], "class": klass, "outcome": outcome, "reason": reason,
            "variant": shown["variant"] if shown else None,
            "render_height": (chosen or (shown if outcome == "unverified_texture" else None) or {}).get("assigned_tier"),
            "chosen": shown, "variants": assessed}


def assess_all(catalog: dict, merged: dict) -> tuple[list, list]:
    """Assess every catalog shader. Returns (assessments, result keys with no catalog entry)."""
    by_id = collections.defaultdict(dict)
    for (shader_id, variant), record in merged.items():
        by_id[shader_id][variant] = record
    known = {r["id"] for r in catalog["shaders"]}
    assessments = []
    for record in catalog["shaders"]:
        item = assess_shader(record, by_id.get(record["id"], {}))
        item["catalog"] = record
        assessments.append(item)
    orphans = sorted(f"{i}/{v}" for (i, v) in merged if i not in known)
    return assessments, orphans


def followups(assessments, variant_ids: dict) -> dict:
    """Id lists for the adaptation pass (plan WP-E)."""
    lower, lite = [], []
    for item in assessments:
        # A texture shader is only ever too_heavy from a bound measurement, and
        # make_manifest.py binds its lower-tier pass too.
        if item["class"] not in (CLASS_SELF, CLASS_ORIGINAL, CLASS_TEXTURE):
            continue
        measured_ok = [a for a in item["variants"].values() if a["status"] == "ok"]
        if item["outcome"] == "too_heavy" and "not yet measured" in item["reason"]:
            lower.append(item["id"])
        for variant, ids in variant_ids.items():
            if variant == "base" or item["id"] not in ids or variant in item["variants"]:
                continue
            if measured_ok and (item["render_height"] or 0) < max(ASSIGNABLE_TIERS):
                lite.append(item["id"])
                break
    return {"measure_lower_tiers": lower, "bench_other_variants": lite}


# --- markdown -------------------------------------------------------------

def _cell(value) -> str:
    return str(value).replace("|", "\\|").replace("\r", " ").replace("\n", " ").strip()


def _ms(value) -> str:
    return f"{value:.1f}" if isinstance(value, (int, float)) and not isinstance(value, bool) else "-"


def _table(headers, rows) -> list:
    lines = ["| " + " | ".join(headers) + " |", "|" + "|".join("---" for _ in headers) + "|"]
    lines += ["| " + " | ".join(_cell(c) for c in row) + " |" for row in rows]
    return lines


def _tier_cells(assessed, height):
    if not assessed:
        return "-", "-"
    data, verdict = assessed["tiers"].get(height), assessed["verdicts"].get(height)
    if data is None:
        return "-", "-"
    if verdict == "SKIPPED":
        return "-", f"skipped ({data.get('skipped')})"
    if verdict == "NO_DATA":
        return "-", "no data"
    return f"{_ms(data.get('p50_ms'))} / {_ms(data.get('p95_ms'))}", verdict


def _review(record) -> str:
    return f"#{record['review_number']:03d}" if record.get("review_number") else "-"


SHADER_HEADERS = ["id", "#", "name", "class", "variant", "720p p50/p95 ms", "720p", "1080p p50/p95 ms", "1080p",
                  "other tiers", "tier", "compile ms", "pixels", "note"]


def shader_row(item) -> list:
    record, chosen = item["catalog"], item["chosen"]
    cells = [f"`{record['id']}`", _review(record), record["name"][:48], item["class"], item["variant"] or "-"]
    for height in MAIN_TIERS:
        cells += list(_tier_cells(chosen, height))
    other = []
    if chosen:
        for height in sorted(chosen["tiers"], reverse=True):
            if height not in MAIN_TIERS:
                timing, verdict = _tier_cells(chosen, height)
                other.append(f"{height}p {timing} {verdict}")
    cells.append("; ".join(other) or "-")
    cells.append(f"{item['render_height']}p" if item["render_height"] else "-")
    cells.append(_ms(chosen["compile_ms"]) if chosen else "-")
    flags = list(chosen["pixel_flags"]) if chosen else []
    cells.append(", ".join(flags) if flags else ("ok" if chosen and chosen["status"] == "ok" else "-"))
    note = item["reason"]
    if chosen and chosen.get("throttled") not in (None, "0x0", "0x0\n"):
        note = (note + "; " if note else "") + f"measured while throttled={chosen['throttled']}"
    cells.append(note[:200] or "-")
    return cells


def _sort_key(item):
    chosen = item["chosen"] or {}
    tier = item["render_height"] or 0
    p95 = (chosen.get("tiers", {}).get(tier or 720) or {}).get("p95_ms")
    return (-tier, p95 if isinstance(p95, (int, float)) else 1e9, item["id"].lower())


def _stats(values):
    values = [v for v in values if isinstance(v, (int, float)) and not isinstance(v, bool)]
    if not values:
        return "-", "-", "-"
    return _ms(min(values)), _ms(statistics.median(values)), _ms(max(values))


def render_markdown(assessments, context: dict) -> str:
    by_outcome = collections.defaultdict(list)
    for item in assessments:
        by_outcome[item["outcome"]].append(item)
    for items in by_outcome.values():
        items.sort(key=_sort_key)
    lines = ["# Shader bench results", ""]
    if context.get("synthetic"):
        lines += ["> **SYNTHETIC DATA.** These numbers were generated on a workstation by `synth_results.py` to exercise the",
                  "> report. They are not measurements from the Pi and say nothing about any shader.", ""]
    lines += ["Generated by `engine/tools/shader_bank/report.py`; do not edit by hand, re-run the tool instead.", "",
              "Results files, in merge order (later lines win per id, variant and tier):", ""]
    lines += [f"- `{name}`" for name in context.get("results_files", [])] or ["- none"]
    lines += ["", f"Catalog: {context.get('catalog_total', len(assessments))} shaders. "
              f"Result keys merged: {context.get('result_keys', 0)}.", ""]
    for warning in context.get("warnings", []):
        lines.append(f"- warning: {_cell(warning)}")
    if context.get("warnings"):
        lines.append("")

    lines += ["## How to read this", "",
              f"Budget for the shader pass, p95 GPU ms at a tier (plan R3): `SOLID` <= {SOLID_P95_MS:g}, `OK` <= {OK_P95_MS:g}, "
              f"`MARGINAL` <= {MARGINAL_P95_MS:g}, otherwise `FAIL`. A shader's render tier is the highest resolution at which "
              "it is `SOLID` or `OK`; the compositor upscales lower tiers to the 1080p output.", "",
              f"Pixel checks on the 96x54 readback: `black` = mean luma <= {BLACK_MEAN_LUMA_MAX:g}, `flat` = spatial std <= "
              f"{FLAT_SPATIAL_STD_MAX:g}, `static` = temporal difference <= {STATIC_TEMPORAL_DIFF_MAX:g}. Any flag (or missing "
              "statistics) moves a shader from qualified to *renders but suspect*.", "",
              "`multipass` shaders are never qualified, whatever they measure: the renderer draws only the Image pass. A "
              "`needs_texture` shader is assessed like any other once it was measured with every channel it reads bound to its "
              "texture, cubemap or volume (the result line then carries `channels`); measured unbound, or reading an input "
              "the renderer cannot supply (keyboard, audio, video, buffer), it stays unverified.", "",
              "`#` is the Stream Deck review number (position in `show-fixed-first-latest.json`).", ""]

    lines += ["## Summary", "", "Outcome by class:", ""]
    classes = [c for c in CLASSES if any(i["class"] == c for i in assessments)]
    rows = []
    for outcome in OUTCOMES:
        counts = [sum(1 for i in by_outcome[outcome] if i["class"] == c) for c in classes]
        rows.append([OUTCOME_TITLES[outcome]] + counts + [sum(counts)])
    rows.append(["total"] + [sum(1 for i in assessments if i["class"] == c) for c in classes] + [len(assessments)])
    lines += _table(["outcome"] + classes + ["all"], rows)

    lines += ["", "Verdict per tier (chosen variant of every measured shader):", ""]
    heights = sorted({h for i in assessments if i["chosen"] for h in i["chosen"]["verdicts"]}, reverse=True)
    verdict_names = ["SOLID", "OK", "MARGINAL", "FAIL", "SKIPPED", "NO_DATA"]
    rows = []
    for klass in classes:
        for height in heights:
            counter = collections.Counter(i["chosen"]["verdicts"].get(height) for i in assessments
                                          if i["class"] == klass and i["chosen"] and i["chosen"]["status"] == "ok")
            if sum(v for k, v in counter.items() if k):
                rows.append([klass, f"{height}p"] + [counter.get(name, 0) for name in verdict_names])
    lines += _table(["class", "tier"] + verdict_names, rows) if rows else ["No tier measurements."]

    lines += ["", "Assigned render tier of qualified shaders:", ""]
    tier_counter = collections.Counter(i["render_height"] for i in by_outcome["qualified"])
    variant_counter = collections.Counter(i["variant"] for i in by_outcome["qualified"])
    lines += _table(["tier", "shaders"], [[f"{h}p", tier_counter[h]] for h in sorted(tier_counter, reverse=True)] or [["-", 0]])
    lines += ["", "Variant used by qualified shaders: " + (", ".join(f"{k}={v}" for k, v in sorted(variant_counter.items())) or "none") + "."]
    statuses = collections.Counter(a["status"] for i in assessments for a in i["variants"].values())
    lines += ["", "Result status over all measured (id, variant) keys: " + (", ".join(f"{k}={v}" for k, v in sorted(statuses.items())) or "none") + "."]
    temps = [a["temp_c"] for i in assessments for a in i["variants"].values() if isinstance(a.get("temp_c"), (int, float))]
    throttled = sum(1 for i in assessments for a in i["variants"].values() if a.get("throttled") not in (None, "0x0"))
    if temps:
        lines += ["", f"SoC temperature across entries: {min(temps):.1f} to {max(temps):.1f} C; entries measured with a non-zero throttle flag: {throttled}."]

    # reference envelope
    originals = [i for i in assessments if i["class"] == CLASS_ORIGINAL]
    lines += ["", "## Reference envelope: the original 18", "",
              "The owner's hand-built, Pi-proven shaders. Their cost is the yardstick for everything else.", ""]
    measured = [i for i in originals if i["chosen"] and i["chosen"]["status"] == "ok"]
    if measured:
        rows = []
        for height in MAIN_TIERS:
            p50 = _stats((i["chosen"]["tiers"].get(height) or {}).get("p50_ms") for i in measured)
            p95 = _stats((i["chosen"]["tiers"].get(height) or {}).get("p95_ms") for i in measured)
            rows.append([f"{height}p", *p50, *p95])
        lines += _table(["tier", "p50 min", "p50 median", "p50 max", "p95 min", "p95 median", "p95 max"], rows)
        compile_stats = _stats(i["chosen"]["compile_ms"] for i in measured)
        lines += ["", f"Compile time ms (min / median / max): {' / '.join(compile_stats)}. Measured: {len(measured)} of {len(originals)}.", ""]
    else:
        lines += [f"No measurements for the {len(originals)} originals in these results.", ""]
    lines += _table(SHADER_HEADERS, [shader_row(i) for i in sorted(originals, key=lambda i: i["id"])])

    def section(title, intro, items, headers=SHADER_HEADERS, row=shader_row):
        out = ["", f"## {title} ({len(items)})", ""] + intro + [""]
        out += _table(headers, [row(i) for i in items]) if items else ["None."]
        return out

    lines += section("Qualified", ["Shaders that run within budget with healthy pixel statistics: originals, self-contained "
                                   "shaders, and texture shaders measured with their channel textures bound. `tier` is the "
                                   "`render_height` written to the show manifest."], by_outcome["qualified"])
    lines += section("Renders but suspect", ["Within budget, but the readback looks near-black, flat or static (or has no "
                                             "statistics). Check the thumbnail before using; not in the show manifest."],
                     by_outcome["suspect"])
    lines += section("Too heavy at the measured tiers", ["Runs, but no measured tier is `SOLID` or `OK`. Candidates for a lower "
                                                         "tier or an adapted variant (see `followup-lower-tiers` list)."],
                     by_outcome["too_heavy"])
    lines += section("Does not work", ["Error status in the latest result line; `note` carries the reason."], by_outcome["failed"])

    def texture_row(item):
        record = item["catalog"]
        kinds = record["channels"]["kinds"]
        row = shader_row(item)
        return row[:3] + [", ".join(f"ch{c}={k}" for c, k in sorted(kinds.items())) or "-"] + row[4:13]

    texture_headers = SHADER_HEADERS[:3] + ["channels read"] + SHADER_HEADERS[4:13]
    lines += section("Needs texture: benchmarked, not qualified",
                     ["The Image pass samples `iChannelN` and was not measured as designed: either the result was taken "
                      "without the channel textures bound, or a channel is an input the renderer cannot supply (keyboard, "
                      "audio, video, buffer). Timings are indicative only and the picture is unverified; `note` says which."],
                     by_outcome["unverified_texture"], texture_headers, texture_row)

    def multipass_row(item):
        record = item["catalog"]
        passes = ", ".join(f"{k}={v}" for k, v in record.get("passes", {}).items()) or "-"
        kinds = ", ".join(f"ch{c}={k}" for c, k in sorted(record["channels"]["kinds"].items())) or "-"
        chosen = item["chosen"]
        timing = "-"
        if chosen and chosen["status"] == "ok":
            timing = "; ".join(f"{h}p {_tier_cells(chosen, h)[0]}" for h in sorted(chosen["tiers"]))
        return [f"`{record['id']}`", _review(record), record["name"][:48], passes, kinds, timing]

    lines += section("Multipass: excluded",
                     ["The Image pass reads one or more buffer passes. Only the Image pass exists in the generated file, so "
                      "the output is not the shader; excluded until a buffer graph exists (plan phase 3)."],
                     sorted(by_outcome["excluded_multipass"], key=lambda i: i["id"].lower()),
                     ["id", "#", "name", "passes", "Image pass reads", "Image-only timing (not meaningful)"], multipass_row)
    lines += section("Not measured", ["Self-contained shaders with no result line in these files."],
                     sorted(by_outcome["not_measured"], key=lambda i: i["id"].lower()),
                     ["id", "#", "name", "class"],
                     lambda i: [f"`{i['id']}`", _review(i["catalog"]), i["catalog"]["name"][:48], i["class"]])

    compared = [i for i in assessments if len(i["variants"]) > 1]
    rows = []
    for item in sorted(compared, key=lambda i: i["id"].lower()):
        for variant in sorted(item["variants"], key=_preference):
            assessed = item["variants"][variant]
            rows.append([f"`{item['id']}`", variant, assessed["status"]]
                        + [f"{_tier_cells(assessed, h)[0]} {_tier_cells(assessed, h)[1]}" for h in MAIN_TIERS]
                        + [f"{assessed['assigned_tier']}p" if assessed["assigned_tier"] else "-",
                           "chosen" if item["variant"] == variant and item["render_height"] else ""])
    lines += ["", f"## Variant comparison ({len(compared)})", "", "Shaders measured in more than one variant.", ""]
    lines += _table(["id", "variant", "status", "720p p50/p95 verdict", "1080p p50/p95 verdict", "tier", ""], rows) if rows else ["None."]
    if context.get("orphans"):
        lines += ["", "## Result keys without a catalog entry", "", ", ".join(f"`{o}`" for o in context["orphans"][:200])]
    return "\n".join(lines) + "\n"


# --- qualified.json and show manifest ---------------------------------------

def qualified_document(assessments, context: dict, follow: dict) -> dict:
    def entry(item):
        chosen, record = item["chosen"], item["catalog"]
        return {"id": item["id"], "review_number": record.get("review_number"), "name": record["name"], "class": item["class"],
                "variant": item["variant"], "render_height": item["render_height"], "channels": chosen.get("channels") or {},
                "p95_ms": {str(h): d.get("p95_ms") for h, d in sorted(chosen["tiers"].items()) if "p95_ms" in d},
                "verdicts": {str(h): v for h, v in sorted(chosen["verdicts"].items())},
                "compile_ms": chosen["compile_ms"], "pixel_flags": chosen["pixel_flags"]}

    groups = collections.defaultdict(list)
    for item in sorted(assessments, key=lambda i: i["id"].lower()):
        groups[item["outcome"]].append(item)
    return {
        "version": 1, "synthetic": bool(context.get("synthetic")), "results_files": context.get("results_files", []),
        "budgets_p95_ms": {"SOLID": SOLID_P95_MS, "OK": OK_P95_MS, "MARGINAL": MARGINAL_P95_MS},
        "pixel_thresholds": {"black_mean_luma_max": BLACK_MEAN_LUMA_MAX, "flat_spatial_std_max": FLAT_SPATIAL_STD_MAX,
                             "static_temporal_diff_max": STATIC_TEMPORAL_DIFF_MAX},
        "summary": {outcome: len(groups[outcome]) for outcome in OUTCOMES},
        "qualified": [entry(i) for i in groups["qualified"]],
        "suspect": [entry(i) for i in groups["suspect"]],
        "too_heavy": [{"id": i["id"], "reason": i["reason"]} for i in groups["too_heavy"]],
        "failed": [{"id": i["id"], "reason": i["reason"]} for i in groups["failed"]],
        "unverified_texture": [{"id": i["id"], "reason": i["reason"], "measured_tier": i["render_height"]} for i in groups["unverified_texture"]],
        "excluded_multipass": [i["id"] for i in groups["excluded_multipass"]],
        "not_measured": [i["id"] for i in groups["not_measured"]],
        "followup": follow,
    }


def crate_group(text: str) -> int:
    """Same keyword crates as build_shader_review_show.py."""
    text = text.lower()
    if any(x in text for x in ("mandelbrot", "fractal", "julia", "raymarch", "distance field")):
        return 0
    if any(x in text for x in ("noise", "plasma", "cloud", "fire", "water", "liquid", "smoke")):
        return 1
    if any(x in text for x in ("kaleid", "symmetr", "tunnel", "vortex", "spiral", "warp")):
        return 2
    return 3


def build_show(entries) -> tuple[dict, dict, list]:
    """Show manifest in the existing format plus `render_height` per shader.

    `entries` are dicts with id, review_number, render_height and text (shader
    source, for the crate keywords), plus optional `channels`: the shader's
    channel bindings in manifest spelling, paths relative to show.json. Layout per page as in
    build_shader_review_show.py: keys 0-23 shader triggers, 24/25 page
    navigation, 26-29 crates, 30 delete, 31 blackout."""
    groups = [[] for _ in range(4)]
    for entry in entries:
        groups[crate_group(entry.get("text", ""))].append(entry)
    for group in groups:
        group.sort(key=lambda e: (e.get("review_number") is None, e.get("review_number") or 0, e["id"].lower(), e["id"]))
    ordered = [entry for group in groups for entry in group]
    limit = min(SHOW_MAX_SHADERS, SHOW_MAX_PAGES * KEYS_PER_PAGE)
    dropped = [entry["id"] for entry in ordered[limit:]]
    ordered = ordered[:limit]
    total_pages = max(1, (len(ordered) - 1) // KEYS_PER_PAGE + 1)
    pages = []
    for start in range(0, len(ordered), KEYS_PER_PAGE):
        bindings = []
        for key, entry in enumerate(ordered[start:start + KEYS_PER_PAGE]):
            label = f"#{entry['review_number']:03d}" if entry.get("review_number") else entry["id"][:10]
            bindings.append({"key": key, "label": label, "command": {"type": "TriggerShader", "shader": start + key},
                             "color": [25, 75, 120]})
        bindings += [
            {"key": 24, "label": "PREV PAGE", "command": {"type": "Page", "delta": -1}, "color": [25, 90, 115]},
            {"key": 25, "label": "NEXT PAGE", "command": {"type": "Page", "delta": 1}, "color": [25, 90, 115]},
            {"key": 31, "label": "BLACK / PAGE", "command": {"type": "Blackout", "enabled": True}, "color": [70, 15, 15]},
        ]
        pages.append({"name": f"SHADERS {start // KEYS_PER_PAGE + 1:02d} / {total_pages:02d}", "bindings": bindings})
    manifest = {"version": 2, "bpm": 128, "audio_device": None,
                "clips": [{"label": "NO VIDEO", "path": "no-video.mp4", "thumbnail": None, "quad_mirror": False}],
                "shaders": [dict({"label": entry["id"], "path": f"{entry['id']}.fs", "blend": "Additive", "opacity": 0.9,
                                  "render_height": int(entry["render_height"])},
                                 **({"channels": entry["channels"]} if entry.get("channels") else {})) for entry in ordered],
                "pages": pages}
    kept = {entry["id"] for entry in ordered}
    group_doc = {f"crate_{i + 1}": [{"index": e.get("review_number"), "shader": e["id"], "render_height": e["render_height"]}
                                    for e in group if e["id"] in kept] for i, group in enumerate(groups)}
    return manifest, group_doc, dropped


def write_show(assessments, show_dir: Path, variants_dir: Path = VARIANTS_DIR, asset_manifest=None,
               assets_dir: Path = ASSETS_DIR) -> dict:
    """Write the show directory. A qualified texture shader gets its channel
    bindings from `asset_manifest` and its texture files copied to
    <show_dir>/assets/; if a file it needs is not in `assets_dir` the shader is
    left out rather than shipped unbound."""
    show_dir = Path(show_dir)
    entries, sources, missing, missing_assets, asset_names = [], {}, [], [], set()
    for item in assessments:
        if item["outcome"] != "qualified":
            continue
        source = Path(variants_dir) / item["variant"] / f"{item['id']}.fs"
        if not source.is_file():
            missing.append(item["id"])
            continue
        channels = []
        if item["class"] == CLASS_TEXTURE:
            bindings, _ = channel_bindings(asset_manifest, item["id"])
            wanted_channels = {str(b["channel"]) for b in bindings}
            names = {name for binding in bindings for name in binding_files(binding)}
            measured = set((item["chosen"] or {}).get("channels") or {})
            if not measured <= wanted_channels or any(not (Path(assets_dir) / name).is_file() for name in names):
                missing_assets.append(item["id"])
                continue
            channels = manifest_channels([b for b in bindings if str(b["channel"]) in measured], SHOW_ASSET_DIR)
            asset_names |= {name for b in bindings if str(b["channel"]) in measured for name in binding_files(b)}
        data = source.read_bytes()
        sources[item["id"]] = data
        entries.append({"id": item["id"], "review_number": item["catalog"].get("review_number"),
                        "render_height": item["render_height"], "text": data.decode("utf-8", errors="ignore"),
                        "channels": channels})
    manifest, group_doc, dropped = build_show(entries)
    owned = (show_dir / SHOW_MARKER).is_file() or not show_dir.exists() or not any(show_dir.iterdir())
    show_dir.mkdir(parents=True, exist_ok=True)
    (show_dir / SHOW_MARKER).write_text("written by engine/tools/shader_bank/report.py; stale *.fs here are removed on each run\n",
                                        encoding="utf-8")
    wanted = {shader["path"] for shader in manifest["shaders"]}
    for shader in manifest["shaders"]:
        write_bytes_if_changed(show_dir / shader["path"], sources[shader["label"]])
    kept = {shader["label"] for shader in manifest["shaders"]}
    used_assets = sorted({name for entry in entries if entry["id"] in kept for channel in entry["channels"]
                          for name in binding_files({"file": channel["path"].rsplit("/", 1)[-1], "kind": channel["kind"]})})
    for name in used_assets:
        write_bytes_if_changed(show_dir / SHOW_ASSET_DIR / name, (Path(assets_dir) / name).read_bytes())
    removed = 0
    if owned:
        for stale in show_dir.glob("*.fs"):
            if stale.name not in wanted:
                stale.unlink()
                removed += 1
        if (show_dir / SHOW_ASSET_DIR).is_dir():
            for stale in (show_dir / SHOW_ASSET_DIR).iterdir():
                if stale.is_file() and stale.name not in used_assets:
                    stale.unlink()
                    removed += 1
    write_json(show_dir / "show.json", manifest)
    write_json(show_dir / "groups.json", group_doc)
    return {"shaders": len(manifest["shaders"]), "pages": len(manifest["pages"]), "dropped": dropped,
            "missing_variant_files": missing, "missing_assets": missing_assets, "assets": len(used_assets),
            "stale_removed": removed}


def available_variants(variants_dir: Path = VARIANTS_DIR) -> dict:
    found = {}
    if Path(variants_dir).is_dir():
        for path in sorted(Path(variants_dir).glob("*/transforms.json")):
            found[path.parent.name] = set(read_json(path).get("shaders", {}))
    return found


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("results", nargs="+", type=Path, help="results JSONL files, oldest first")
    parser.add_argument("--catalog", type=Path, default=None)
    parser.add_argument("--variants-dir", type=Path, default=VARIANTS_DIR)
    parser.add_argument("--synthetic", action="store_true", help="results are synthetic: write *.synthetic outputs with a banner")
    parser.add_argument("--doc", type=Path, default=None, help="markdown output")
    parser.add_argument("--qualified", type=Path, default=None, help="qualified.json output")
    parser.add_argument("--show-dir", type=Path, default=None, help="show manifest directory")
    parser.add_argument("--no-show", action="store_true")
    parser.add_argument("--asset-manifest", type=Path, default=ASSET_MANIFEST_PATH,
                        help="asset store manifest, for the channel bindings of qualified texture shaders")
    parser.add_argument("--assets-dir", type=Path, default=ASSETS_DIR)
    args = parser.parse_args(argv)

    events, warnings = iter_results(args.results)
    if any(event.get("synthetic") for event in events) and not args.synthetic:
        parser.error("these results are marked synthetic; pass --synthetic (outputs then go to *.synthetic paths)")
    suffix = ".synthetic" if args.synthetic else ""
    doc_path = args.doc or (BANK_OUT / "SHADER_BENCH_RESULTS.synthetic.md" if args.synthetic
                            else ROOT / "docs" / "native" / "SHADER_BENCH_RESULTS.md")
    qualified_path = args.qualified or BANK_OUT / f"qualified{suffix}.json"
    show_dir = args.show_dir or BANK_OUT / f"show{suffix}"

    catalog = load_catalog(args.catalog) if args.catalog else load_catalog()
    merged = merge_results(events)
    assessments, orphans = assess_all(catalog, merged)
    if orphans:
        warnings.append(f"{len(orphans)} result keys have no catalog entry (re-run catalog.py?)")
    context = {"synthetic": args.synthetic, "results_files": [rel(p) for p in args.results], "warnings": warnings,
               "catalog_total": len(catalog["shaders"]), "result_keys": len(merged), "orphans": orphans}
    follow = followups(assessments, available_variants(args.variants_dir))
    write_bytes_if_changed(doc_path, render_markdown(assessments, context).encode("utf-8"))
    document = qualified_document(assessments, context, follow)
    write_json(qualified_path, document)
    for key, name in (("measure_lower_tiers", "followup-lower-tiers"), ("bench_other_variants", "followup-variants")):
        write_bytes_if_changed(qualified_path.parent / f"{name}{suffix}.txt", "".join(f"{i}\n" for i in follow[key]).encode("utf-8"))

    print(f"REPORT {len(assessments)} catalog shaders, {len(merged)} result keys -> {rel(doc_path)}")
    print("  " + ", ".join(f"{OUTCOME_TITLES[o]}={document['summary'][o]}" for o in OUTCOMES))
    tiers = collections.Counter(i["render_height"] for i in assessments if i["outcome"] == "qualified")
    print("  qualified by render tier: " + (", ".join(f"{h}p={tiers[h]}" for h in sorted(tiers, reverse=True)) or "none"))
    print(f"  {rel(qualified_path)}; follow-up lists: lower tiers={len(follow['measure_lower_tiers'])}, "
          f"other variants={len(follow['bench_other_variants'])}")
    if not args.no_show:
        show = write_show(assessments, show_dir, args.variants_dir, load_asset_manifest(args.asset_manifest), args.assets_dir)
        print(f"  show manifest: {show['shaders']} shaders on {show['pages']} pages, {show['assets']} texture files "
              f"-> {rel(show_dir / 'show.json')}")
        if show["missing_assets"]:
            print(f"  WARNING: {len(show['missing_assets'])} qualified texture shaders were left out of the show because their "
                  f"bindings or texture files are not in the asset store: {', '.join(show['missing_assets'][:8])}")
        if show["dropped"]:
            print(f"  WARNING: {len(show['dropped'])} qualified shaders exceed the engine's {SHOW_MAX_SHADERS}-shader limit and were left out")
        if show["missing_variant_files"]:
            print(f"  WARNING: {len(show['missing_variant_files'])} qualified shaders have no local variant file (run adapt.py)")
    for warning in warnings:
        print(f"  warning: {warning}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
