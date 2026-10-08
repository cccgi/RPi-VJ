#!/usr/bin/env python3
"""Report an adaptation campaign without promoting unreviewed timing passes.

python engine/tools/shader_bank/adaptation_report.py --help
The baseline catalog must be batch-filtered. Reviews use id/variant keys with
decision=accept|degraded and a note. A rescue requires final-run evidence,
both audio pixel checks, startup limits, and an explicit visual acceptance.
"""
import argparse
import collections
import json
import math
from pathlib import Path

from common import ROOT, write_json
import report


def results(path):
    return [r for line in Path(path).read_text(encoding='utf-8').splitlines()
            if line.strip() and (r := json.loads(line)).get('event') == 'result']


def worst_ms(record, tier=720):
    values = [record.get(k, {}).get(str(tier), {}).get('p95_ms') for k in ('tiers', 'tiers_max')]
    return max(values) if all(isinstance(v, (int, float)) and math.isfinite(v) for v in values) else None


def safety_flags(record):
    flags = []
    if record.get('status') in ('hang', 'gpu_reset', 'crash', 'too_slow'):
        flags.append(record['status'])
    if any(d.get('first_ms', 0) > 1500 for k in ('tiers', 'tiers_max') for d in record.get(k, {}).values()):
        flags.append('first frame >1500 ms')
    if record.get('compile_ms', 0) > 1500:
        flags.append('compile >1500 ms (conservative candidate limit)')
    return flags


def candidate_tier(record):
    if record.get('status') != 'ok' or safety_flags(record):
        return None
    if not isinstance(record.get('compile_ms'), (int, float)):
        return None
    if any(report.pixel_flags(record.get(k)) for k in ('pixels', 'pixels_max')):
        return None
    assessed = report.assess_variant(record, True)
    for tier in (1080, 720):
        if assessed['verdicts'].get(tier) in report.GOOD_VERDICTS:
            # Refuse incomplete startup evidence even if the p95 looks good.
            if all(isinstance(record.get(k, {}).get(str(tier), {}).get('first_ms'), (int, float))
                   for k in ('tiers', 'tiers_max')):
                return tier
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--catalog', type=Path, required=True)
    ap.add_argument('--baseline', type=Path, required=True)
    ap.add_argument('--experiments', type=Path, nargs='+', required=True)
    ap.add_argument('--final', type=Path, required=True)
    ap.add_argument('--reviews', type=Path, required=True)
    ap.add_argument('--out', type=Path, required=True)
    ap.add_argument('--candidate-catalog', type=Path, required=True)
    args = ap.parse_args()
    catalog = json.loads(args.catalog.read_text(encoding='utf-8'))
    baseline = {r['id']: r for r in results(args.baseline)}
    final = {(r['id'], r['variant']): r for r in results(args.final)}
    reviews = json.loads(args.reviews.read_text(encoding='utf-8'))
    measured = {}
    for path in args.experiments:
        for r in results(path):
            measured[(r['id'], r['variant'])] = r
    measured.update({k: r for k, r in final.items() if k[1] != 'base'})
    counts = collections.Counter()
    rows, accepted, details = [], [], []
    def fmt(value):
        return '-' if value is None else f'{value:.2f}'
    for shader in sorted(catalog['shaders'], key=lambda x: x['id'].lower()):
        sid = shader['id']
        tried = [r for (i, v), r in measured.items() if i == sid]
        good = [r for r in tried if candidate_tier(r)]
        reviewed = [r for r in good if reviews.get(sid+'/'+r['variant'], {}).get('decision') == 'accept'
                    and (sid, r['variant']) in final
                    and not report.unbound_channels(shader, report.assess_variant(r, True))]
        danger = [r for r in tried if safety_flags(r)]
        chosen = None
        if reviewed:
            chosen = max(reviewed, key=lambda r: candidate_tier(r))
            tier = candidate_tier(chosen)
            verdict = f'rescued at {tier}p'
            note = reviews[sid+'/'+chosen['variant']]['note']
            accepted.append(shader)
        elif good:
            chosen = min(good, key=lambda r: worst_ms(r))
            review = reviews.get(sid+'/'+chosen['variant'], {})
            verdict = 'passes but look degraded' if review.get('decision') == 'degraded' else 'not rescued'
            note = review.get('note', 'Timing passes; visual acceptance/final confirmation missing. Not in candidate.')
        elif danger:
            chosen = danger[0]
            verdict = 'dangerous'
            note = '; '.join(safety_flags(chosen))+'; candidate excluded, originals unchanged'
        elif tried:
            chosen = min(tried, key=lambda r: worst_ms(r) if worst_ms(r) is not None else 1e30)
            verdict = 'not rescued'
            flags = report.pixel_flags(chosen.get('pixels')) + report.pixel_flags(chosen.get('pixels_max'))
            note = chosen.get('error') or ('Pixel flags: '+', '.join(sorted(set(flags))) if flags else 'Measured adaptations remain above 13 ms at 720p')
        else:
            verdict = 'not rescued'
            counts['adaptation not measured'] += 1
            f = shader['features']
            note = ('Adaptation unmeasured: no eligible canonical loop; needs source-specific work'
                    if not (args.catalog.parent/'variants/pi75'/f'{sid}.fs').exists()
                    else 'Adaptation unmeasured: even the selected loop reduction projects above budget; this is an inference, not proof')
            if f.get('noise_functions') and f.get('raymarch'):
                note += '; nested field/noise work is a likely cost'
        counts[verdict] += 1
        if tried:
            counts['adaptation measured'] += 1
        key = sid+'/'+chosen['variant'] if chosen else None
        title = shader['name'].replace('|', '\\|').replace('\n', ' ')
        applied = ', '.join(sorted({r['variant'] for r in tried})) or 'none measured'
        after = fmt(worst_ms(chosen)) if chosen else '-'
        note = note.replace('|', '\\|').replace('\n', ' ')
        rows.append(f'| `{sid}` | {title} | {fmt(worst_ms(baseline[sid]))} | {applied} | {after} | {verdict} | {note} |')
        details.append({'id': sid, 'verdict': verdict, 'chosen': key, 'note': note,
                        'baseline_ms': worst_ms(baseline[sid]), 'after_ms': worst_ms(chosen) if chosen else None})
    nrescued = counts['rescued at 1080p']+counts['rescued at 720p']
    lines = ['# Shader adaptation results — 2026-10-08', '',
             f'{nrescued} of the 367 over-budget shaders are rescued: {counts["rescued at 1080p"]} at 1080p and {counts["rescued at 720p"]} at 720p. '
             f'{counts["passes but look degraded"]} pass timing but fail visual review. '
             f'{counts["dangerous"]} have unsafe adaptation results. The candidate has not been deployed.', '',
             f'Adaptations were measured for {counts["adaptation measured"]} unique shaders; '
             f'{counts["adaptation not measured"]} received static triage only. '
             '“Not rescued” does not mean impossible: the unmeasured remainder is explicitly identified in the table. '
             'The main remaining work is source-specific iteration handling, nested loops/noise and expensive distance fields.', '',
             '## Evidence and limits', '',
             '- All new numbers are from the actual Pi 5 using the installed benchmark engine, at 720p and 1080p, with beat128 and max audio. '
             'The table reports the worse 720p p95 of those two patterns.',
             '- Rescued variants passed again in one final session. Original and adapted thumbnails were reviewed, including both audio patterns. '
             'Three-time pixel statistics corroborate motion; this is not a live controller or long-soak qualification.',
             '- The candidate additionally rejects black/flat/static statistics under either pattern, startup frames over 1500 ms, '
             'and compiles over 1500 ms. The last limit is a conservative campaign choice; the runbook does not give a precise compile limit.',
             '- The ordinary report tool checks suspicious pixels only for beat128, and does not enforce these startup limits. '
             'This campaign screens records before calling report.py for the candidate.',
             '- Generic loop changes can break interior tests or accumulation normalization. The reviewed pi_tuned recipes fix shared settings instead. '
             'A timing pass alone is never a rescue.',
             '- More time should go first to near-miss shaders with variable loop bounds and the measured visually degraded cases, '
             'rather than treating blanket loop caps as safe optimizations.', '',
             'Final session: `'+str(args.final.relative_to(ROOT)).replace('\\','/')+'`.', '',
             'Experiments: '+', '.join('`'+str(p.relative_to(ROOT)).replace('\\','/')+'`' for p in args.experiments)+'.', '',
             'Variants and exact edits are under `target/shader-bank/variants/<variant>/transforms.json`. '
             'Review decisions are in `'+str(args.reviews.relative_to(ROOT)).replace('\\','/')+'`.', '',
             '## All 367 shaders', '',
             '| ID | Title | Original 720p ms | Applied/measured variants | After 720p ms | Verdict | Note |',
             '|---|---|---:|---|---:|---|---|', *rows, '']
    args.out.write_text('\n'.join(lines), encoding='utf-8')
    candidate = dict(catalog, shaders=accepted)
    write_json(args.candidate_catalog, candidate)
    write_json(args.candidate_catalog.with_name('adaptation-verdicts.json'), {'counts': dict(counts), 'shaders': details})
    print(json.dumps(dict(counts), indent=2))


if __name__ == '__main__':
    main()
