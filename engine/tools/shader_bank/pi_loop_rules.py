"""Experimental single-loop budget reductions; every output needs Pi/visual review.

The largest recognized loop is a cost hypothesis, not a proven hot spot.
Only a canonical ascending for loop with an explicit zero/one initializer,
unit increment and literal/constant upper bound is eligible. Change the bound
in the condition only: shared constants, colour normalization, audio and the
generated wrapper are untouched. Use the existing GLSL analyser for extents.
"""
import math
import collections
import re

from common import split_shadertoy
from glsl_features import collect_constants, find_loops, find_functions, FBM_FN_RE, NUM, collect_defines, _CONST_RE


def reduce_dominant_loop(text, code, fraction, cap=None):
    start, end = split_shadertoy(text)
    body = code[start:end]
    constants = collect_constants(body)
    definitions = collections.Counter(name for name, _, _ in collect_defines(body))
    definitions.update(m[1] for m in _CONST_RE.finditer(body))
    choices = []
    for loop in find_loops(body, constants):
        if loop['kind'] != 'for' or not loop['iterations'] or loop['iterations'] < 8:
            continue
        header = body[loop['start']:loop['body_start']]
        # Deliberately reject golfed headers, compound conditions, non-unit
        # increments and inferred initializers rather than guessing semantics.
        match = re.fullmatch(
            r'for\s*\(\s*(int|float)\s+(\w+)\s*=\s*(0\.?0*|1\.?0*)\s*;\s*'
            r'\2\s*<\s*(' + NUM + r'|[A-Za-z_]\w*)\s*;\s*'
            r'(?:\2\s*\+\+|\+\+\s*\2|\2\s*\+=\s*1(?:\.0*)?)\s*\)\s*', header)
        if not match:
            continue
        if definitions[match[4]] > 1:
            continue  # scoped/conditional definitions are ambiguous to this analyser
        bound = constants.get(match[4])
        if bound is None:
            try:
                bound = float(match[4])
            except ValueError:
                continue
        if not math.isfinite(bound) or bound != int(bound):
            continue
        initial = int(float(match[3]))
        count = int(bound) - initial
        if count < 8:
            continue
        new_count = max(4, int(count * fraction))
        if cap is not None:
            new_count = min(new_count, cap)
        if new_count >= count:
            continue
        replacement = str(initial + new_count) + ('.0' if match[1] == 'float' else '')
        offset = start + loop['start'] + match.start(4)
        choices.append((count, offset, len(match[4]), replacement, new_count))
    if not choices:
        return text, []
    count, offset, length, replacement, new_count = max(choices, key=lambda item: item[0])
    old = text[offset:offset + length]
    changed = text[:offset] + replacement + text[offset + length:]
    return changed, [{'line': text.count('\n', 0, offset) + 1,
                      'before': old, 'after': replacement,
                      'iterations_before': count, 'iterations_after': new_count,
                      'note': 'One loop only; experimental look change, requires visual review'}]


def rule_pi75(text, code):
    return reduce_dominant_loop(text, code, 0.75)


def rule_pi50(text, code):
    return reduce_dominant_loop(text, code, 0.5)


def rule_pi28(text, code):
    return reduce_dominant_loop(text, code, 1.0, cap=28)


def rule_pi_fbm2(text, code):
    start, end = split_shadertoy(text)
    body = code[start:end]
    edits = []
    for fn in find_functions(body):
        if not FBM_FN_RE.search(fn['name']):
            continue
        for loop in find_loops(body[fn['start']:fn['end']], {}):
            offset = fn['start'] + loop['start']
            header = body[offset:fn['start']+loop['body_start']]
            match = re.fullmatch(r'for\s*\(\s*int\s+(\w+)\s*=\s*0\s*;\s*\1\s*<\s*([4-9])\s*;\s*(?:\1\s*\+\+|\+\+\s*\1)\s*\)\s*', header)
            if match:
                at = start+offset+match.start(2)
                edits.append((at, match[2]))
    changed = text
    for at, before in sorted(edits, reverse=True):
        changed = changed[:at]+'2'+changed[at+len(before):]
    return changed, [{'line': text.count('\n', 0, at)+1, 'before': before, 'after': '2',
                      'note': 'FBM octaves reduced; visual review required'} for at, before in edits]
