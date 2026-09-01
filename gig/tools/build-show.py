#!/usr/bin/env python3
"""Rebuild show.json's CLIP pads from whatever is actually in gig/media/.

Run ON THE PI (it reads the real directory):
    cd ~/RPi-VJ && python3 gig/tools/build-show.py            # uses h264
    cd ~/RPi-VJ && python3 gig/tools/build-show.py --codec hevc

Shader and blend pads are preserved exactly; only clip pads are regenerated.
"""
import json, os, sys, argparse

HERE = os.path.dirname(os.path.abspath(__file__))
GIG  = os.path.dirname(HERE)

ap = argparse.ArgumentParser()
ap.add_argument('--codec', default='h264', choices=['h264', 'hevc'])
ap.add_argument('--show', default=os.path.join(GIG, 'show.json'))
a = ap.parse_args()

mdir = os.path.join(GIG, 'media', a.codec)
if not os.path.isdir(mdir):
    sys.exit("!! no such directory: %s" % mdir)

clips = sorted(f for f in os.listdir(mdir) if f.lower().endswith(('.mp4', '.mov', '.webm')))
if not clips:
    sys.exit("!! no clips found in %s" % mdir)
print("== %d clips in media/%s" % (len(clips), a.codec))

thumbs = os.path.join(GIG, 'media', 'thumbs')

def pad(key, idx, fname):
    stem = os.path.splitext(fname)[0]
    p = {
        "key": key,
        "type": "clip",
        "src": "media/%s/%s" % (a.codec, fname),
        # Big number for instant recognition; the name sits small above it.
        "bigText": str(idx + 1),
        "label": stem.replace('_', ' ')[:11],
    }
    t = os.path.join(thumbs, stem + '.jpg')
    if os.path.isfile(t):
        p["thumb"] = "media/thumbs/%s.jpg" % stem
    return p

show = json.load(open(a.show, encoding='utf-8'))
banks = show.get('banks', [])

# How many clip slots each page offers, matching the agreed layout.
LAYOUT = [16, 8, 8]

i = 0
for bi, bank in enumerate(banks):
    slots = LAYOUT[bi] if bi < len(LAYOUT) else 0
    keep = [p for p in bank.get('pads', []) if p.get('type') != 'clip']
    newpads = []
    for k in range(slots):
        if i >= len(clips):
            break
        newpads.append(pad(k, i, clips[i]))
        i += 1
    bank['pads'] = newpads + keep
    print("   page %d (%s): %d clips, %d other" % (bi + 1, bank.get('name', '?'),
                                                   len(newpads), len(keep)))

json.dump(show, open(a.show, 'w', encoding='utf-8'), indent=2)
leftover = len(clips) - i
print("== written. %d clips placed%s" % (i, (", %d did not fit" % leftover) if leftover > 0 else ""))
