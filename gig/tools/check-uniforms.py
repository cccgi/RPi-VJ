#!/usr/bin/env python3
"""Fail if the bench harness and the engine disagree about which uniforms exist.

src/isf.js and probe/shader-bench.html each carry their own list of uniforms to
auto-declare. They drifted once: the engine gained u_flow*/u_locked/u_spec_flow
and the bench did not, so five shaders that compile fine in the engine reported
COMPILE FAIL in the bench and cost three debugging rounds. A bench that lies is
worse than no bench. Run this from the repo root before trusting a bench result.
"""
import re, sys, pathlib

root = pathlib.Path(__file__).resolve().parents[1]

def names(text, start_marker):
    seg = text[text.index(start_marker):]
    seg = seg[:seg.index('];') + 2]
    return set(re.findall(r"'([A-Za-z_][A-Za-z0-9_]*)'", seg))

isf = (root / 'src' / 'isf.js').read_text()
bench = (root / 'probe' / 'shader-bench.html').read_text()

engine = names(isf, 'const WANT_UNIFORMS = [')
harness = names(bench, "var WANT=[") | names(bench, "var WANT_ARR=[")

# The engine also lists legacy camelCase and stock-ISF aliases the bench does
# not need to feed; only the u_* convention is load-bearing here.
engine_u = {n for n in engine if n.startswith('u_')}
missing = sorted(engine_u - harness)

if missing:
    print('FAIL: probe/shader-bench.html does not declare these engine uniforms:')
    for n in missing:
        print('   ' + n)
    print('\nAdd them to WANT (or WANT_ARR if the shader indexes them) and')
    print('supply a value in the per-frame loop, or the bench will report')
    print('false COMPILE FAILs for shaders that work in the engine.')
    sys.exit(1)

print('OK: bench declares all %d engine u_* uniforms' % len(engine_u))
