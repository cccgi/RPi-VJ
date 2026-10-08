"""Reviewed one-off iteration settings for the October 7 batch.

Keep these separate from generic loop rules: updating a shared iteration
setting also updates palette normalization and interior/escape tests. Each
replacement must occur exactly once or the rule refuses the changed source.
"""
from common import split_shadertoy

TUNING = {
    'llX3Dn': [('const int steps = 50;', 'const int steps = 37;')],
    'mdjSDW': [('#define CONVERGE_ITER 30', '#define CONVERGE_ITER 14'), ('#define CONVERGE_ITER_F 30.', '#define CONVERGE_ITER_F 14.')],
    '4fcXz2': [('#define ITERATIONS 18', '#define ITERATIONS 12')],
    'l3GGRd': [('float maxIter = 100.0;', 'float maxIter = 64.0;')],
    '4d2yDG': [('int maxIterations = 50;', 'int maxIterations = 28;')],
    'MdSyW3': [('int maxIterations = 50;', 'int maxIterations = 28;')],
    '3lSGzD': [('int max_iter = 50;', 'int max_iter = 28;')],
    'fsXGzB': [('float iterations = 50.0;', 'float iterations = 28.0;')],
    '7s23zD': [('float iterations = 50.0;', 'float iterations = 28.0;')],
    'wX3fD8': [('int maxIterations = 100;', 'int maxIterations = 50;')],
    't3jGRy': [('#define maxIter 100', '#define maxIter 64')],
    'tcyGRc': [('fbm(noiseInput + vec2(time * 0.7, 0.0), 3)', 'fbm(noiseInput + vec2(time * 0.7, 0.0), 2)'),
               ('fbm(noiseInput + vec2(0.0, time * 0.7), 3)', 'fbm(noiseInput + vec2(0.0, time * 0.7), 2)')],
}


def rule_pi_tuned(text, code, record=None):
    edits = TUNING.get((record or {}).get('id'), [])
    start, end = split_shadertoy(text)
    body = text[start:end]
    if any(body.count(before) != 1 for before, after in edits):
        raise ValueError('Reviewed tuning no longer matches source for '+record['id'])
    log = []
    for before, after in edits:
        offset = body.index(before)
        if code[start+offset:start+offset+len(before)] != before:
            raise ValueError('Reviewed tuning points into a comment')
        log.append({'line': text.count('\n', 0, start+offset)+1,
                    'before': before, 'after': after})
    for before, after in edits:
        body = body.replace(before, after, 1)
    return text[:start]+body+text[end:], log
