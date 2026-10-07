#!/usr/bin/env python3
"""Migrate the nine visual banks to native manifests; no browser code is executed."""
import argparse
import json
import os
from pathlib import Path
import shutil

BLENDS = dict(normal='Alpha', add='Additive', screen='Screen', multiply='Multiply',
              overlay='Overlay', softlight='SoftLight', lumakey='LumaKey', difference='Difference')
NAMES = ['1080p', 'Blend modes', 'System', 'Video FX', 'Effect FX', 'Quad Mirror', '50th', '720p', 'Neon']


def command(kind, **values):
    return dict(type=kind, **values)


def convert(data, source_root, destination, prepared_root=None):
    """Keep absent media visible/disabled; never claim that a source was transcoded."""
    show = dict(version=2, bpm=data.get('bpmDefault', 128), audio_device=None, clips=[], shaders=[], pages=[])
    clips, shaders = {}, {}
    def relative(path):
        return Path(os.path.relpath(path, destination.parent)).as_posix()
    for bank_index, bank in enumerate(data['banks'][:9]):
        bindings = []
        for pad in bank['pads']:
            key = pad['key']
            if key in (24, 25, 30, 31):
                # System-row navigation always overrides data keys, as in the original runtime.
                continue
            action = None
            label = pad.get('label', '')
            if pad['type'] == 'clip':
                identity = (pad['src'], pad.get('layout'))
                if identity not in clips:
                    clips[identity] = len(show['clips'])
                    source = source_root / pad['src']
                    # Optional prepared root preserves original paths relative to the legacy root.
                    path = prepared_root / pad['src'] if prepared_root else source
                    thumb = source_root / pad['thumb'] if pad.get('thumb') else None
                    show['clips'].append(dict(label=label, path=relative(path),
                        thumbnail=relative(thumb) if thumb else None,
                        quad_mirror=pad.get('layout') == 'mirror_hv_tl'))
                action = command('TriggerClip', clip=clips[identity])
            elif pad['type'] == 'shader':
                if pad['src'] not in shaders:
                    shaders[pad['src']] = len(show['shaders'])
                    source = source_root / pad['src']
                    asset = destination.parent / 'shaders' / source.name
                    asset.parent.mkdir(parents=True, exist_ok=True)
                    if asset.exists() and asset.read_bytes() != source.read_bytes():
                        raise ValueError(f'refusing to overwrite different shader: {asset}')
                    if not asset.exists():
                        shutil.copyfile(source, asset)
                    show['shaders'].append(dict(label=label, path=relative(asset),
                        blend=BLENDS[pad.get('blend', 'normal')], opacity=pad.get('opacity', 1.0)))
                action = command('TriggerShader', shader=shaders[pad['src']])
            elif pad['type'] in ('videofx', 'shaderfx'):
                action = command('SetVideoEffect' if pad['type'] == 'videofx' else 'SetShaderEffect', effect=pad['fx'])
            elif pad['type'] == 'blend':
                action = command('SetBlend', mode=BLENDS[pad['mode']], opacity=1.0)
            elif pad['type'] == 'system':
                label = '1080p FIXED'  # The native appliance has a fixed deployment output mode.
            else:
                raise ValueError(f'unsupported visual pad type: {pad["type"]}')
            color = pad.get('color', '#23415f')
            rgb = [int(color[i:i+2], 16) for i in (1, 3, 5)]
            # Keep unique clip numbers when short labels repeat.
            if pad.get('bigText') and pad['type'] == 'clip':
                label = f'{pad["bigText"]} {label}'
            bindings.append(dict(key=key, label=label, command=action, color=rgb))
        extra = []
        if bank_index == 0:
            extra = [('SPEED -', command('AdjustSpeed', delta=1/1.01-1)), ('SPEED +', command('AdjustSpeed', delta=.01)),
                     ('CLR CLIP', command('ClearClips')), ('CLR FX', command('ClearShader'))]
        elif bank_index == 1:
            extra = [('BPM -', command('AdjustBpm', delta=-1)), ('BPM +', command('AdjustBpm', delta=1)),
                     ('TAP', command('TapTempo', time_ms=0)), ('HUD', command('ToggleHud'))]
        elif bank_index == 2:
            extra = [(f'OPACITY {int(v*100)}', command('SetOpacity', opacity=v)) for v in (.25, .5, .75, 1.)]
        elif bank_index in (3, 4):
            extra = [(f'FX AMT {int(v*100)}', command('SetEffectAmount', layer='Video' if bank_index == 3 else 'Shader', amount=v)) for v in (.25, .5, .75, 1.)]
        for key, (label, action) in enumerate(extra, 26):
            bindings = [b for b in bindings if b['key'] != key]
            bindings.append(dict(key=key, label=label, command=action, color=[55, 65, 85]))
        show['pages'].append(dict(name=NAMES[bank_index], bindings=sorted(bindings, key=lambda b:b['key'])))
    return show


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--prepared-root', type=Path, help='HEVC assets with the original relative directory layout')
    args = parser.parse_args()
    if args.output.exists():
        parser.error('output exists; choose a new manifest path')
    source, output = args.source.resolve(), args.output.resolve()
    data = json.loads(source.read_text(encoding='utf-8-sig'))
    output.parent.mkdir(parents=True, exist_ok=True)
    show = convert(data, source.parent, output, args.prepared_root.resolve() if args.prepared_root else None)
    output.write_text(json.dumps(show, indent=2) + '\n', encoding='utf-8')
    print(f'Migrated {len(show["pages"])} visual pages, {len(show["clips"])} clips, {len(show["shaders"])} shaders. Media was not converted.')
    if len(data['banks']) > 9:
        print('Excluded auxiliary banks: ' + ', '.join(b['name'] for b in data['banks'][9:]))


if __name__ == '__main__':
    main()
