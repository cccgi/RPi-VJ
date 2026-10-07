#!/usr/bin/env python3
"""Static cost features of a GLSL fragment shader body.

These are heuristics on comment-stripped source, not a GLSL parser. They are
meant to rank and explain, never to decide qualification: the Pi benchmark
does that. Known blind spots are listed in `extract_features.__doc__`.
"""
from __future__ import annotations

import math
import re

from common import normalize_newlines, strip_comments

NUM = r"(?:\d+\.?\d*(?:[eE][+-]?\d+)?|\.\d+(?:[eE][+-]?\d+)?)"
_NUM_FULL = re.compile(rf"^\(?\s*([+-]?{NUM})[uUfF]?\s*\)?$")
_DEFINE_RE = re.compile(r"^[ \t]*#[ \t]*define[ \t]+(\w+)(\([^)\n]*\))?[ \t]*(.*?)[ \t]*$", re.M)
_CONST_RE = re.compile(rf"\bconst\s+(?:lowp\s+|mediump\s+|highp\s+)?(?:int|float|uint)\s+(\w+)\s*=\s*([+-]?{NUM})[uUfF]?\s*[;,]")
_PREPROC_RE = re.compile(r"^[ \t]*#[^\n]*(?:\\\n[^\n]*)*", re.M)
_IDENT_CALL_RE = re.compile(r"\b([A-Za-z_]\w*)\s*\(")
_KEYWORDS = {"if", "for", "while", "do", "switch", "return", "else"}

AA_NAME_RE = re.compile(
    r"^(?:AA|SSAA|MSAA|FSAA|AA_SAMPLES|AA_LEVEL|AASAMPLES|\w+_AA|ANTI_?ALIAS\w*|[Aa]nti[Aa]lias\w*"
    r"|SUPER_?SAMPL\w*|[Ss]uper[Ss]ampl\w*|[Ss]ub[Ss]amples|SUBSAMPLES)$")
QUALITY_NAME_RE = re.compile(r"QUALITY|^HQ$|^LQ$|HIGH_?Q|LOW_?Q|^FAST\w*|CHEAP|DETAIL|^SHADOWS?$|REFLECTIONS?$|^FXAA$", re.I)
STEP_NAME_RE = re.compile(r"STEPS?$|^STEPS|ITER(?:ATION)?S?$|^ITER|OCTAVES?$|SAMPLES$", re.I)
RAYMARCH_FN_RE = re.compile(r"march|trace|raycast|castray|intersect|shootray", re.I)
SDF_FN_RE = re.compile(r"^(?:map|scene|sdf|df|de|dist|world|sd[A-Z_]\w*|sdf\w+|\w*[Ss]dfs?)$")
NORMAL_FN_RE = re.compile(r"normal", re.I)
NOISE_FN_RE = re.compile(r"fbm|noise|turb|octave|voronoi|worley|perlin|simplex", re.I)
FBM_FN_RE = re.compile(r"fbm|turb|octave", re.I)


def parse_number(text):
    if text is None:
        return None
    match = _NUM_FULL.match(text.strip())
    if not match:
        return None
    try:
        return float(match.group(1))
    except ValueError:
        return None


def collect_constants(code: str) -> dict:
    """Object-like numeric `#define`s and `const int/float` initialisers.
    A name defined more than once (e.g. in `#if` branches) keeps its largest value."""
    constants: dict = {}
    for match in _DEFINE_RE.finditer(code):
        if match.group(2):
            continue
        value = parse_number(match.group(3))
        if value is not None:
            constants[match.group(1)] = max(value, constants.get(match.group(1), value))
    for match in _CONST_RE.finditer(code):
        value = float(match.group(2))
        constants[match.group(1)] = max(value, constants.get(match.group(1), value))
    return constants


def collect_defines(code: str) -> list:
    return [(m.group(1), bool(m.group(2)), m.group(3).strip()) for m in _DEFINE_RE.finditer(code)]


def _match_close(code: str, start: int, open_ch: str, close_ch: str) -> int:
    """Index of the bracket closing the one at `start`; -1 when unbalanced."""
    depth = 0
    for index in range(start, len(code)):
        ch = code[index]
        if ch == open_ch:
            depth += 1
        elif ch == close_ch:
            depth -= 1
            if depth == 0:
                return index
    return -1


def _statement_end(code: str, pos: int) -> int:
    """End offset (exclusive) of the statement starting at `pos`."""
    n = len(code)
    while pos < n and code[pos].isspace():
        pos += 1
    if pos >= n:
        return n
    if code[pos] == "{":
        end = _match_close(code, pos, "{", "}")
        return n if end < 0 else end + 1
    keyword = re.match(r"(for|while|if|switch)\b\s*\(", code[pos:])
    if keyword:
        close = _match_close(code, pos + keyword.end() - 1, "(", ")")
        if close < 0:
            return n
        end = _statement_end(code, close + 1)
        if keyword.group(1) == "if":
            tail = re.match(r"\s*else\b", code[end:])
            if tail:
                end = _statement_end(code, end + tail.end())
        return end
    if re.match(r"do\b", code[pos:]):
        end = _statement_end(code, pos + 2)
        tail = re.match(r"\s*while\s*\(", code[end:])
        if tail:
            close = _match_close(code, end + tail.end() - 1, "(", ")")
            semi = code.find(";", close) if close >= 0 else -1
            return n if semi < 0 else semi + 1
        return end
    depth = 0
    for index in range(pos, n):
        ch = code[index]
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            if depth == 0:
                return index
            depth -= 1
        elif ch == ";" and depth == 0:
            return index + 1
    return n


def _split_top(text: str, sep: str) -> list:
    parts, depth, last = [], 0, 0
    for index, ch in enumerate(text):
        if ch in "([":
            depth += 1
        elif ch in ")]":
            depth -= 1
        elif ch == sep and depth == 0:
            parts.append(text[last:index])
            last = index + 1
    parts.append(text[last:])
    return parts


def _resolve(token, constants):
    if token is None:
        return None
    token = token.strip()
    negative = token.startswith("-")
    if negative:
        token = token[1:].strip()
    wrapped = re.match(r"^(?:float|int|uint)\s*\(\s*(\w+|" + NUM + r")\s*\)$", token)
    if wrapped:
        token = wrapped.group(1)
    value = parse_number(token)
    if value is None and re.match(r"^[A-Za-z_]\w*$", token):
        value = constants.get(token)
    if value is None:
        return None
    return -value if negative else value


_OPERAND = rf"(?:(?:float|int|uint)\s*\(\s*\w+\s*\)|{NUM}[uUfF]?|[A-Za-z_]\w*)"
_COND_LT = re.compile(rf"(?:\+\+|--)?\s*\b([A-Za-z_]\w*)\s*(?:\+\+|--)?\s*\)?\s*(<=?)\s*(-?\s*{_OPERAND})")
_COND_GT = re.compile(rf"(-?\s*{_OPERAND})\s*(>=?)\s*\(?\s*(?:\+\+|--)?\s*([A-Za-z_]\w*)")
_COND_DESC = re.compile(rf"\b([A-Za-z_]\w*)\s*(?:--)?\s*(>=?)\s*(-?\s*{_OPERAND})")
_STEP_RE = re.compile(rf"\b([A-Za-z_]\w*)\s*([+-])=\s*({NUM}|[A-Za-z_]\w*)")


def loop_iterations(header: str, constants: dict):
    """Best-effort iteration count of a `for (init; cond; incr)` header.
    Returns None when the bound is not a literal or a resolvable constant."""
    parts = _split_top(header, ";")
    if len(parts) != 3:
        return None
    init, cond, incr = parts
    starts = {}
    for name, value in re.findall(rf"\b([A-Za-z_]\w*)\s*=\s*(-?\s*{_OPERAND})", init):
        starts[name] = _resolve(value, constants)
    step_src = incr + " ; " + cond
    steps = {name: (sign, _resolve(value, constants)) for name, sign, value in _STEP_RE.findall(step_src)}

    def factor_of(var):
        """Geometric steps, common in golfed fbm loops: `n += n`, `a += a+a`,
        `n += n*3.`, `i *= .2`, `i /= 2.`."""
        v = re.escape(var)
        if re.search(rf"\b{v}\s*\+=\s*{v}\s*\+\s*{v}\b(?!\s*[*/.\w])", step_src):
            return 3.0
        match = re.search(rf"\b{v}\s*\+=\s*{v}\s*\*\s*({NUM})|\b{v}\s*\+=\s*({NUM})\s*\*\s*{v}\b", step_src)
        if match:
            return 1.0 + float(match.group(1) or match.group(2))
        if re.search(rf"\b{v}\s*\+=\s*{v}\b(?!\s*[*/+.\w\[(])", step_src):
            return 2.0
        match = re.search(rf"\b{v}\s*([*/])=\s*({NUM})(?!\s*[*/\w(])", step_src)
        if match and float(match.group(2)) > 0:
            return float(match.group(2)) if match.group(1) == "*" else 1.0 / float(match.group(2))
        return None

    def count(var, bound, inclusive, descending=False):
        factor = factor_of(var)
        if factor is not None:
            start = starts.get(var)
            if not start or not bound or start <= 0 or bound <= 0 or factor == 1.0:
                return None
            if (factor > 1.0) == descending:
                return None
            total = math.ceil(math.log(bound / start) / math.log(factor) - 1e-9)
            return int(total) if 0 < total < 1000 else None
        start = starts.get(var)
        if start is None:
            start = 0.0 if not descending else None
        if bound is None or start is None:
            return None
        step = 1.0
        if var in steps:
            step = steps[var][1]
            if step is None or step <= 0:
                return None
        span = (start - bound) if descending else (bound - start)
        total = math.ceil(span / step - 1e-9) + (1 if inclusive else 0)
        return int(total) if 0 < total < 10_000_000 else None

    match = _COND_LT.search(cond)
    if match:
        # ascending: `i < N`, `i++ < N`, `++i <= N`
        if match.group(1) not in constants or match.group(1) in starts:
            return count(match.group(1), _resolve(match.group(3), constants), match.group(2) == "<=")
    match = _COND_GT.search(cond)
    if match and _resolve(match.group(1), constants) is not None and match.group(3) not in constants:
        # ascending written backwards: `80. > i++`
        return count(match.group(3), _resolve(match.group(1), constants), match.group(2) == ">=")
    match = _COND_DESC.search(cond)
    if match and match.group(1) in starts:
        # descending: `for (int i = N; i > 0; i--)`
        return count(match.group(1), _resolve(match.group(3), constants), match.group(2) == ">=", descending=True)
    return None


def find_loops(code: str, constants: dict) -> list:
    """All `for` / `while` / `do` loops with body extent and iteration estimate."""
    loops = []
    for match in re.finditer(r"\b(for|while)\s*\(", code):
        open_paren = match.end() - 1
        close = _match_close(code, open_paren, "(", ")")
        if close < 0:
            continue
        kind = match.group(1)
        if kind == "while":
            # `do { } while (x);` tail is not a loop of its own
            before = code[:match.start()].rstrip()
            after = code[close + 1:close + 40].lstrip()
            if before.endswith("}") and after.startswith(";") and _is_do_tail(code, match.start()):
                continue
        header = code[open_paren + 1:close]
        iterations = loop_iterations(header, constants) if kind == "for" else None
        loops.append({"kind": kind, "start": match.start(), "body_start": close + 1,
                      "end": _statement_end(code, close + 1), "iterations": iterations})
    for match in re.finditer(r"\bdo\b\s*\{", code):
        end = _statement_end(code, match.start())
        loops.append({"kind": "do", "start": match.start(), "body_start": match.end() - 1,
                      "end": end, "iterations": None})
    loops.sort(key=lambda item: item["start"])
    return loops


def _is_do_tail(code: str, while_pos: int) -> bool:
    close = code.rfind("}", 0, while_pos)
    depth = 0
    for index in range(close, -1, -1):
        ch = code[index]
        if ch == "}":
            depth += 1
        elif ch == "{":
            depth -= 1
            if depth == 0:
                return bool(re.search(r"\bdo\s*$", code[:index]))
    return False


def find_functions(code: str) -> list:
    """Top-level function definitions as {name, start, end} over the body braces."""
    functions, depth, index, n = [], 0, 0, len(code)
    last_boundary = 0
    while index < n:
        ch = code[index]
        if ch == "{":
            if depth == 0:
                head = code[last_boundary:index]
                match = re.search(r"([A-Za-z_]\w*)\s*\(([^()]*(?:\([^()]*\)[^()]*)*)\)\s*$", head)
                end = _match_close(code, index, "{", "}")
                if end < 0:
                    break
                if match and match.group(1) not in _KEYWORDS:
                    functions.append({"name": match.group(1), "start": index, "end": end + 1})
                index = end + 1
                last_boundary = index
                continue
            depth += 1
        elif ch == "}":
            depth = max(0, depth - 1)
        elif ch == ";" and depth == 0:
            last_boundary = index + 1
        index += 1
    return functions


def _iteration_product(functions, loops, code, entry_names):
    """Largest product of loop bounds along lexical nesting and the static call
    graph, starting from the entry function. Unresolved bounds count as 1."""
    by_name: dict = {}
    for function in functions:
        by_name.setdefault(function["name"], []).append(function)
    memo: dict = {}
    active: set = set()

    def region_cost(start, end, depth=0):
        children = [l for l in loops if start <= l["start"] and l["end"] <= end and l["start"] >= start]
        direct = []
        for loop in children:
            if not any(o is not loop and o["body_start"] <= loop["start"] and loop["end"] <= o["end"] for o in children):
                direct.append(loop)
        best = 1.0
        cursor = start
        segments = []
        for loop in direct:
            segments.append((cursor, loop["start"]))
            # the loop header itself can call functions once per iteration
            inner = region_cost(loop["start"] + 1, loop["end"], depth + 1) if depth < 12 else 1.0
            best = max(best, (loop["iterations"] or 1) * inner)
            cursor = loop["end"]
        segments.append((cursor, end))
        for seg_start, seg_end in segments:
            for match in _IDENT_CALL_RE.finditer(code, seg_start, seg_end):
                if match.group(1) in by_name:
                    best = max(best, function_cost(match.group(1)))
        return best

    def function_cost(name):
        if name in memo:
            return memo[name]
        if name in active:
            return 1.0
        active.add(name)
        cost = max(region_cost(f["start"] + 1, f["end"]) for f in by_name[name])
        active.discard(name)
        memo[name] = min(cost, 1e15)
        return memo[name]

    for name in entry_names:
        if name in by_name:
            return function_cost(name)
    return max([function_cost(name) for name in by_name] + [region_cost(0, len(code))])


def extract_features(body: str) -> dict:
    """Static cost features of a shader body.

    Blind spots: loops hidden in function-like macros are counted but not
    nested; bounds held in uniforms, function parameters or expressions are
    'unresolved'; loops that exit early via `break` are counted at their full
    bound; dead code and `#if 0` blocks are counted as live.
    """
    code = strip_comments(normalize_newlines(body))
    constants = collect_constants(code)
    defines = collect_defines(code)
    # Preprocessor lines are analysed separately: a `for(` inside a macro has no
    # body we can follow.
    plain = _PREPROC_RE.sub(lambda m: re.sub(r"[^\n]", " ", m.group(0)), code)
    macro_text = "\n".join(m.group(0) for m in _PREPROC_RE.finditer(code))
    loops = find_loops(plain, constants)
    functions = find_functions(plain)
    names = [f["name"] for f in functions]

    bounds = [l["iterations"] for l in loops if l["iterations"]]
    for_loops = [l for l in loops if l["kind"] == "for"]
    depth_max, product_max = 0, 1
    for loop in loops:
        chain = [o for o in loops if o["start"] <= loop["start"] and loop["end"] <= o["end"]]
        depth_max = max(depth_max, len(chain))
        product = 1
        for item in chain:
            product *= item["iterations"] or 1
        product_max = max(product_max, product)

    def span_of(position):
        for function in functions:
            if function["start"] <= position < function["end"]:
                return function["name"]
        return None

    fbm_octaves = 0
    fbm_unrolled = 0
    for function in functions:
        if FBM_FN_RE.search(function["name"]):
            inner = [l["iterations"] for l in loops if function["start"] <= l["start"] < function["end"] and l["iterations"]]
            if inner:
                fbm_octaves = max(fbm_octaves, max(inner))
            else:
                text = plain[function["start"]:function["end"]]
                calls = sum(1 for m in _IDENT_CALL_RE.finditer(text)
                            if NOISE_FN_RE.search(m.group(1)) and not FBM_FN_RE.search(m.group(1)))
                fbm_unrolled = max(fbm_unrolled, calls)
    for name, value in constants.items():
        if re.search(r"OCTAVE", name, re.I) and 0 < value <= 64:
            fbm_octaves = max(fbm_octaves, int(value))

    aa_defines, quality_defines, step_defines = {}, {}, {}
    for name, is_function, value in defines:
        if is_function:
            continue
        if AA_NAME_RE.match(name):
            aa_defines.setdefault(name, []).append(value)
        elif QUALITY_NAME_RE.search(name):
            quality_defines.setdefault(name, []).append(value)
        elif STEP_NAME_RE.search(name) and parse_number(value) is not None:
            step_defines.setdefault(name, []).append(value)
    for match in re.finditer(rf"\bconst\s+(?:int|float)\s+(\w+)\s*=\s*({NUM})", plain):
        if AA_NAME_RE.match(match.group(1)):
            aa_defines.setdefault(match.group(1), []).append(match.group(2))
    commented = sorted({m.group(1) for m in re.finditer(r"^[ \t]*//+[ \t]*#[ \t]*define[ \t]+(\w+)", normalize_newlines(body), re.M)
                        if AA_NAME_RE.match(m.group(1)) or QUALITY_NAME_RE.search(m.group(1))})

    markers = []
    march_fns = sorted({n for n in names if RAYMARCH_FN_RE.search(n)})
    sdf_fns = sorted({n for n in names if SDF_FN_RE.match(n)})
    normal_fns = sorted({n for n in names if NORMAL_FN_RE.search(n)})
    if march_fns:
        markers.append("march_fn:" + ",".join(march_fns[:4]))
    if sdf_fns:
        markers.append("sdf_fn:" + ",".join(sdf_fns[:4]))
    if normal_fns:
        markers.append("normal_fn:" + ",".join(normal_fns[:3]))
    march_defines = sorted(n for n in step_defines if re.search(r"STEP|MARCH", n, re.I))
    if march_defines:
        markers.append("step_define:" + ",".join(march_defines[:4]))
    sdf_set = set(sdf_fns)
    loop_calls_sdf = any(m.group(1) in sdf_set for l in loops
                         for m in _IDENT_CALL_RE.finditer(plain, l["start"], l["end"]))
    if loop_calls_sdf:
        markers.append("loop_calls_sdf")
    # Golfed one-loop marchers: `for(float i,z,d; i++<80.; ...) { ... z += d ... }`
    golf = any((l["iterations"] or 0) >= 20 and re.search(r"\b\w+\s*\+=\s*\w+\s*[=;,)]|\b\w+\s*\+=\s*\w+\s*=", plain[l["start"]:l["end"]])
               and re.search(r"\b(?:normalize|length)\s*\(", plain[l["start"]:l["end"]]) for l in loops)
    if golf and not markers:
        markers.append("accumulating_loop")
    raymarch = bool(march_fns or loop_calls_sdf or march_defines or (golf and not sdf_fns))

    fetch_re = re.compile(r"\b(?:texture|textureLod|texelFetch|texture2D|textureGrad|texture2DLod|textureCube)\s*\(")
    fetches = [m.start() for m in fetch_re.finditer(plain)]
    fetches_in_loops = sum(1 for p in fetches if any(l["start"] <= p < l["end"] for l in loops))
    est = _iteration_product(functions, loops, plain, ("mainImage", "main"))

    return {
        "body_bytes": len(body.encode("utf-8", errors="replace")),
        "lines": code.count("\n") + 1,
        "functions": len(functions),
        "loops": len(loops),
        "for_loops": len(for_loops),
        "while_loops": len(loops) - len(for_loops),
        "loops_in_macros": len(re.findall(r"\b(?:for|while)\s*\(", macro_text)),
        "loops_unresolved": sum(1 for l in loops if not l["iterations"]),
        "loop_bounds": sorted(bounds, reverse=True)[:12],
        "loop_bound_max": max(bounds) if bounds else 0,
        "loop_nest_depth_max": depth_max,
        "loop_nest_product_max": int(product_max),
        "est_iter_product": int(min(est, 1e15)),
        "raymarch": raymarch,
        "raymarch_markers": markers,
        "noise_functions": sorted({n for n in names if NOISE_FN_RE.search(n)})[:8],
        "fbm_octaves_hint": int(max(fbm_octaves, fbm_unrolled)),
        "aa_defines": aa_defines,
        "quality_defines": quality_defines,
        "step_defines": step_defines,
        "commented_quality_defines": commented,
        "uses_hw_performance": bool(re.search(r"\bHW_PERFORMANCE\b", code)),
        "derivatives": len(re.findall(r"\b(?:dFdx|dFdy|fwidth)\s*\(", plain)),
        "texture_fetches": len(fetches),
        "texture_fetches_in_loops": fetches_in_loops,
    }
