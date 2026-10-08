#!/usr/bin/env python3
"""Emit shader variants -> target/shader-bank/variants/<variant>/<id>.fs.

    base  the file unchanged (byte copy)
    lite  only transforms that keep the look apart from anti-aliasing
          (plan WP-B item 2). A shader with no applicable transform gets no
          `lite` file.
    tex   the file as it has to be once channel textures are bound: cubemap
          and volume lookups get their 3-component coordinates back (the
          converter had cut them to `.xy` so they compiled against
          `sampler2D`). Only shaders that carry such a rewrite get a `tex`
          file; it needs its channel bindings to compile. The sampler type in
          the header is left alone: the renderer sets it from the binding.
    react the converter's legacy audio wrapper replaced by the reactive one
          (make_native_shadertoy.py: geometry on the flow clocks and smoothed
          bands, per-band hits, low brightness floor), on top of the `tex`
          rewrite where that applies. Every shader that ends with the legacy
          wrapper gets a file, except one whose own code reads the audio
          uniforms. transforms.json lists, under `react`, the shaders given the
          whole-pixel mode, those whose gl_FragCoord reads were redirected, and
          those not wrapped, each with the reason.
    react_lite  `lite` and `react` together, where a `lite` transform applies.

Each variant directory gets a transforms.json recording exactly which rules
fired per shader and what they changed.

    python engine/tools/shader_bank/adapt.py                 # all variants
    python engine/tools/shader_bank/adapt.py --variant lite

Adding a rule (e.g. for a later `pi` variant from
docs/native/SHADER_ADAPTATION_RULES.md):
  1. write `def rule_xxx(text, code) -> (new_text, edits)` where `code` is the
     comment-blanked text with identical offsets and `edits` is a list of
     {"line", "before", "after"};
  2. register it in RULES with an id, a description, and `doc_rule` set to the
     rule number in SHADER_ADAPTATION_RULES.md;
  3. list its id in VARIANT_RULES for every variant that should apply it.
Rules run in the listed order; each sees the output of the previous one.
"""
from __future__ import annotations

import argparse
import collections
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Optional

sys.path.insert(0, str(Path(__file__).resolve().parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from common import (ROOT, SHADERTOY_HEAD_END, SHADERTOY_WRAPPER_RE, VARIANTS_DIR, decode, load_catalog, rel,  # noqa: E402
                    sha256_bytes, strip_comments, write_bytes_if_changed, write_json)
from glsl_features import AA_NAME_RE  # noqa: E402
from make_native_shadertoy import (COORD_REGEX_RESTORES, COORD_REWRITES, REACT_MARKER, REACT_PRELUDE,  # noqa: E402
                                   WRAPPER as LEGACY_WRAPPER, react_analysis, react_wrapper)

SPATIAL_KINDS = ("cubemap", "volume")   # catalog channel kinds sampled with a 3-component coordinate
_CHANNEL_RE = re.compile(r"\biChannel([0-3])\b")

_LITERAL = r"\d+\.\d*|\.\d+|\d+"
_DEFINE_NUM_RE = re.compile(rf"^([ \t]*#[ \t]*define[ \t]+)(\w+)([ \t]+\(?[ \t]*)({_LITERAL})([ \t]*\)?[ \t]*\r?)$", re.M)
_CONST_NUM_RE = re.compile(rf"(\bconst\s+(?:int|float)\s+)(\w+)(\s*=\s*)({_LITERAL})(\s*;)")
_FOR_RE = re.compile(r"\bfor\s*\(")


def _line_of(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def _line_text(text: str, offset: int) -> str:
    start = text.rfind("\n", 0, offset) + 1
    end = text.find("\n", offset)
    return text[start:len(text) if end < 0 else end].rstrip("\r").strip()


def _apply(text: str, replacements) -> tuple[str, list]:
    """Apply [(start, end, new)] and describe each edit by line."""
    edits = []
    for start, end, new in sorted(replacements, reverse=True):
        before = _line_text(text, start)
        line = _line_of(text, start)
        text = text[:start] + new + text[end:]
        edits.append({"line": line, "before": before, "after": _line_text(text, start)})
    edits.reverse()
    return text, edits


def _one_like(literal: str) -> str:
    """`2` -> `1`, `2.` -> `1.`, `2.0` -> `1.0` (keeps int/float typing)."""
    if "." not in literal:
        return "1"
    return "1." + ("0" if literal.split(".", 1)[1] else "")


def _for_headers(code: str) -> list:
    headers = []
    for match in _FOR_RE.finditer(code):
        depth = 0
        for index in range(match.end() - 1, len(code)):
            if code[index] == "(":
                depth += 1
            elif code[index] == ")":
                depth -= 1
                if depth == 0:
                    headers.append(code[match.end():index])
                    break
    return headers


def _is_sample_count(name: str, code: str) -> bool:
    """Evidence that `name` is a supersample count rather than, say, a
    smoothing width: it bounds a `for` loop, gates `#if NAME > 1`, or bounds a
    sample loop's `break`."""
    n = re.escape(name)
    bound = re.compile(rf"<=?\s*(?:float|int)?\s*\(?\s*{n}\b|\b{n}\s*\)?\s*>=?")
    if any(bound.search(header) for header in _for_headers(code)):
        return True
    if re.search(rf"^[ \t]*#[ \t]*(?:el)?if\b[^\n]*\b{n}\s*>\s*[01]\b", code, re.M):
        return True
    return bool(re.search(rf"^[^\n#]*\b{n}\b[^\n]*\bbreak\b", code, re.M))


def rule_hw_performance_0(text: str, code: str):
    """`#define HW_PERFORMANCE 1` -> 0 when the shader body tests it. Shadertoy
    authors use HW_PERFORMANCE==0 to select their own mobile settings (almost
    always AA 1)."""
    defines = [m for m in re.finditer(r"^([ \t]*#[ \t]*define[ \t]+HW_PERFORMANCE[ \t]+)(\d+)", code, re.M)]
    if not defines:
        return text, []
    define_spans = [(m.start(), m.end()) for m in defines]
    references = [m for m in re.finditer(r"\bHW_PERFORMANCE\b", code)
                  if not any(s <= m.start() < e for s, e in define_spans)]
    if not references:
        return text, []
    return _apply(text, [(m.start(2), m.end(2), "0") for m in defines if m.group(2) != "0"])


def rule_aa_samples_1(text: str, code: str):
    """Supersample counts (`#define AA 2`, `const float antialias = 2.;`) -> 1.
    Only numeric literals > 1 on a recognised AA name with evidence that the
    name is used as a sample count."""
    replacements = []
    for regex in (_DEFINE_NUM_RE, _CONST_NUM_RE):
        for match in regex.finditer(code):
            name, literal = match.group(2), match.group(4)
            if not AA_NAME_RE.match(name) or float(literal) <= 1.0:
                continue
            if _is_sample_count(name, code):
                replacements.append((match.start(4), match.end(4), _one_like(literal)))
    return _apply(text, replacements) if replacements else (text, [])


def rule_aa_switch_off(text: str, code: str):
    """Boolean AA switch (`#define AA 1` tested only as `#if AA == 0` / `#if !AA`
    / `#elif AA == 1`) -> 0."""
    replacements = []
    for match in _DEFINE_NUM_RE.finditer(code):
        name, literal = match.group(2), match.group(4)
        if not AA_NAME_RE.match(name) or literal != "1":
            continue
        n = re.escape(name)
        uses = [m for m in re.finditer(rf"\b{n}\b", code) if m.start() != match.start(2)]
        lines = [_line_text(code, m.start()) for m in uses]
        if not lines or not all(re.match(r"#\s*(?:el)?if\b", line) for line in lines):
            continue
        if any(re.search(rf"defined\s*\(?\s*{n}\b", line) for line in lines):
            continue
        if any(re.search(rf"\b{n}\s*==\s*0\b|!\s*{n}\b", line) for line in lines):
            replacements.append((match.start(4), match.end(4), "0"))
    return _apply(text, replacements) if replacements else (text, [])


def rule_restore_3d_coords(text: str, code: str, record: Optional[dict] = None):
    """Undo the converter's `.xy` cuts on cubemap and volume lookups.

    Each known rewrite is reversed where its result still stands in the file
    and the channel it names is a cubemap or volume for this shader (rewrites
    that go through a macro and name no channel need the shader to have such a
    channel at all). A 2D channel is never touched."""
    kinds = ((record or {}).get("channels") or {}).get("kinds") or {}
    spatial = {str(channel) for channel, kind in kinds.items() if kind in SPATIAL_KINDS}
    if not spatial:
        return text, []

    def applies(snippet: str) -> bool:
        named = set(_CHANNEL_RE.findall(snippet))
        return named <= spatial if named else True

    replacements, taken = [], []

    def claim(start: int, end: int, new: str):
        if all(end <= s or start >= e for s, e in taken):
            taken.append((start, end))
            replacements.append((start, end, new))

    # Longest results first, so a rewrite is not mistaken for a shorter one it contains.
    for before, after in sorted(set(COORD_REWRITES), key=lambda pair: -len(pair[1])):
        if not applies(after):
            continue
        start = text.find(after)
        while start >= 0:
            claim(start, start + len(after), before)
            start = text.find(after, start + len(after))
    for pattern, replacement in COORD_REGEX_RESTORES:
        for match in re.finditer(pattern, text):
            if applies(match.group(0)):
                claim(match.start(), match.end(), match.expand(replacement))
    return _apply(text, replacements) if replacements else (text, [])


# The legacy wrapper exactly as the converter wrote it, in any newline style
# (the bank holds it with CRLF), up to the end of the file.
_LEGACY_WRAPPER_RE = re.compile(r"\r*\n".join(re.escape(line) for line in LEGACY_WRAPPER.split("\n")) + r"\s*\Z")


def _react_body(text: str, code: str, wrapper_start: int) -> tuple[int, str]:
    """(offset where the Shadertoy code starts, that code comment-blanked)."""
    start = text.find(SHADERTOY_HEAD_END)
    start = 0 if start < 0 else start + len(SHADERTOY_HEAD_END)
    return start, code[start:wrapper_start]


def react_skip_reason(text: str) -> Optional[str]:
    """Why `react_wrapper` leaves a file alone; None when it rewrites it."""
    if REACT_MARKER in text:
        return "already has the react wrapper"
    match = _LEGACY_WRAPPER_RE.search(text)
    if not match:
        if SHADERTOY_WRAPPER_RE.search(text):
            return "generated wrapper differs from the converter's (edited by hand?)"
        return "no generated wrapper: the shader has its own main()"
    _, body = _react_body(text, strip_comments(text), match.start())
    return react_analysis(body)["skip"]


def rule_react_wrapper(text: str, code: str):
    """Replace the converter's legacy audio wrapper with the reactive one.

    Fires only on a file that ends with exactly the legacy wrapper; a file that
    already has the reactive wrapper, has its own `main`, or reads the audio
    uniforms in its own code (wrapping it would double the reaction) is left
    alone. `react_analysis` picks the mode: `pixel` for shaders that need whole
    pixel coordinates, `normal` otherwise; a shader that reads gl_FragCoord
    itself also gets REACT_PRELUDE so that read is transformed too."""
    match = _LEGACY_WRAPPER_RE.search(text)
    if not match or REACT_MARKER in text:
        return text, []
    body_start, body = _react_body(text, code, match.start())
    plan = react_analysis(body)
    if plan["skip"]:
        return text, []
    newline = "\r\n" if "\r\n" in match.group(0) else "\n"
    wrapper = react_wrapper(plan["mode"], plan["direct_fragcoord"], plan["undef"]).replace("\n", newline)
    edits = [{"line": _line_of(text, match.start()) + 1, "before": "legacy audio wrapper main()",
              "after": f"react wrapper ({plan['mode']})", "mode": plan["mode"], "reasons": plan["reasons"],
              "direct_fragcoord": plan["direct_fragcoord"]}]
    new_text = text[:match.start()] + wrapper
    if plan["direct_fragcoord"]:
        prelude = newline + REACT_PRELUDE.replace("\n", newline).rstrip("\r\n")
        new_text = new_text[:body_start] + prelude + new_text[body_start:]
        edits.insert(0, {"line": _line_of(text, body_start) + 1, "before": "",
                         "after": "gl_FragCoord redirected to the transformed coordinate"})
    return new_text, edits


@dataclass(frozen=True)
class Rule:
    id: str
    description: str
    apply: Callable
    look: str                       # what the rule may change visually
    doc_rule: Optional[str] = None  # rule number in docs/native/SHADER_ADAPTATION_RULES.md, once it exists
    needs_record: bool = False      # apply(text, code, catalog_record) instead of apply(text, code)


RULES = {rule.id: rule for rule in (
    Rule("hw_performance_0", "#define HW_PERFORMANCE 1 -> 0 where the shader tests it", rule_hw_performance_0,
         "author's own low-performance settings (normally AA only; see transforms.json per shader)", "E10"),
    Rule("aa_samples_1", "supersample count define/const (AA, SSAA, ANTIALIAS*, *_AA, subSamples ...) -> 1", rule_aa_samples_1,
         "anti-aliasing only", "E10"),
    Rule("aa_switch_off", "boolean AA switch tested only by #if -> 0", rule_aa_switch_off, "anti-aliasing only", "E10"),
    Rule("restore_3d_coords", "cubemap/volume lookups get back the 3-component coordinate the converter cut to .xy",
         rule_restore_3d_coords, "the shader as designed, once its cubemap/volume channel is bound", None, True),
    Rule("react_wrapper", "legacy audio wrapper main() -> reactive wrapper (flow-clock geometry, low brightness floor)",
         rule_react_wrapper, "audio response: geometry moves on the flow clocks, dark at silence; the shader's own code "
         "is untouched", "section 5"),
)}

# Variant -> ordered rule ids. `base` is the unchanged file. A later `pi`
# variant is added here as  "pi": VARIANT_RULES["lite"] + (<new rule ids>,).
_LITE = ("hw_performance_0", "aa_samples_1", "aa_switch_off")
VARIANT_RULES = {
    "base": (),
    "lite": _LITE,
    "tex": ("restore_3d_coords",),
    # The reactive wrapper on the file as it has to be with textures bound, so one
    # variant serves self-contained and texture shaders alike.
    "react": ("restore_3d_coords", "react_wrapper"),
    "react_lite": _LITE + ("restore_3d_coords", "react_wrapper"),
}
# Variant -> groups of rule ids; a file is written only when at least one rule
# of every group fired. Variants not listed need any one of their rules.
VARIANT_NEEDS = {
    "react": (("react_wrapper",),),
    "react_lite": (("react_wrapper",), _LITE),
}


def transform(text: str, rule_ids, record: Optional[dict] = None) -> tuple[str, list]:
    """Apply rules in order. Returns (new_text, applied) where applied is a
    list of {"rule", "count", "edits"} for the rules that changed something.
    `record` is the shader's catalog record, for the rules that need it."""
    applied = []
    for rule_id in rule_ids:
        rule = RULES[rule_id]
        if rule.needs_record:
            new_text, edits = rule.apply(text, strip_comments(text), record)
        else:
            new_text, edits = rule.apply(text, strip_comments(text))
        if edits and new_text != text:
            applied.append({"rule": rule_id, "count": len(edits), "edits": edits})
            text = new_text
    return text, applied


def build_variant(variant: str, catalog: dict, out_root: Path = VARIANTS_DIR, root: Path = ROOT) -> dict:
    rule_ids = VARIANT_RULES[variant]
    out_dir = Path(out_root) / variant
    out_dir.mkdir(parents=True, exist_ok=True)
    shaders, written, rule_counts = {}, 0, collections.Counter()
    needs = VARIANT_NEEDS.get(variant, ())
    reactive = "react_wrapper" in rule_ids
    not_wrapped = {}
    for record in catalog["shaders"]:
        source = Path(root) / record["path"]
        data = source.read_bytes()
        if rule_ids:
            text, encoding = decode(data)
            new_text, applied = transform(text, rule_ids, record)
            fired = {item["rule"] for item in applied}
            if reactive and "react_wrapper" not in fired:
                not_wrapped[record["id"]] = {"class": record["class"], "reason": react_skip_reason(text)}
            if not applied or any(not fired & set(group) for group in needs):
                continue
            out_data = new_text.encode(encoding)
        else:
            applied, out_data = [], data
        written += write_bytes_if_changed(out_dir / f"{record['id']}.fs", out_data)
        for item in applied:
            rule_counts[item["rule"]] += 1
        shaders[record["id"]] = {"source": record["path"], "source_sha256": sha256_bytes(data),
                                 "sha256": sha256_bytes(out_data), "class": record["class"], "transforms": applied}
    removed = 0
    for stale in out_dir.glob("*.fs"):
        if stale.stem not in shaders:
            stale.unlink()
            removed += 1
    report = {
        "variant": variant,
        "rules": [{"id": r, "description": RULES[r].description, "look": RULES[r].look, "doc_rule": RULES[r].doc_rule}
                  for r in rule_ids],
        "summary": {"shaders": len(shaders), "catalog_shaders": len(catalog["shaders"]),
                    "by_class": dict(sorted(collections.Counter(s["class"] for s in shaders.values()).items())),
                    "rule_counts": {r: rule_counts.get(r, 0) for r in rule_ids}},
        "shaders": dict(sorted(shaders.items())),
    }
    if reactive:
        # How the reactive wrapper treated each shader, for review: which ones got
        # the whole-pixel mode or the gl_FragCoord redirect and why, and which
        # were left with what they had.
        modes, pixel, direct = collections.Counter(), {}, []
        for shader_id, entry in sorted(shaders.items()):
            edit = next(e for item in entry["transforms"] if item["rule"] == "react_wrapper"
                        for e in item["edits"] if "mode" in e)
            modes[edit["mode"]] += 1
            if edit["mode"] == "pixel":
                pixel[shader_id] = edit["reasons"]
            if edit["direct_fragcoord"]:
                direct.append(shader_id)
        report["react"] = {"modes": dict(sorted(modes.items())), "pixel_mode": pixel, "gl_fragcoord_redirected": direct,
                           "not_wrapped": dict(sorted(not_wrapped.items()))}
        report["summary"]["react_modes"] = report["react"]["modes"]
        report["summary"]["not_wrapped"] = len(not_wrapped)
    write_json(out_dir / "transforms.json", report)
    report["files_written"], report["files_removed"] = written, removed
    return report


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--variant", action="append", choices=sorted(VARIANT_RULES),
                        help="variant to build (repeatable; default: all)")
    parser.add_argument("--catalog", type=Path, default=None)
    parser.add_argument("--out", type=Path, default=VARIANTS_DIR)
    args = parser.parse_args(argv)
    catalog = load_catalog(args.catalog) if args.catalog else load_catalog()
    for variant in args.variant or list(VARIANT_RULES):
        report = build_variant(variant, catalog, args.out)
        summary = report["summary"]
        print(f"VARIANT {variant}: {summary['shaders']} of {summary['catalog_shaders']} shaders -> "
              f"{rel(args.out / variant)} ({report['files_written']} written, {report['files_removed']} stale removed)")
        if summary["rule_counts"]:
            print("  rules fired (shaders): " + ", ".join(f"{k}={v}" for k, v in summary["rule_counts"].items()))
            print("  by class: " + ", ".join(f"{k}={v}" for k, v in summary["by_class"].items()))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
