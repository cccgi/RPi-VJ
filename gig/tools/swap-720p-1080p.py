#!/usr/bin/env python3
"""2026-08-27. One-off, re-runnable migration:

  - Pages 1-3 (CLIPS/MIX/SYS) currently hold 28 h264 (720p) clip pads
    (16 + 8 + 4). Replace them with the 25 media/1080 (1080p-converted)
    files, distributed 14 / 7 / 4 (same ~2:1:0.5 ratio as before, scaled to
    25). The 2 slots this drops from page 1 and the 1 from page 2 are
    removed outright, not left as dead pads.
  - Page 7 ("1080 TEST" -> renamed "720p") currently holds 24 media/1080
    clip pads. Replace with all 28 h264 (720p) files: 24 at keys 0-23 (its
    existing footprint) plus 4 more at keys 26-29, which index.html's
    SYS_PAGES now leaves empty for this page specifically (see the "page 7"
    comment in index.html).
  - Page 6 ("4K TEST" -> renamed "4K") is untouched -- not part of this ask.

Run from gig/:  python3 tools/swap-720p-1080p.py
Writes show.json in place; a timestamped .bak is made first.
"""
import json, os, sys, time, glob

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHOW = os.path.join(ROOT, "show.json")

def label_from_filename(path):
    base = os.path.splitext(os.path.basename(path))[0]
    low = base.lower()
    if "abstract" in low:
        core = "Abstract"
    elif "laser" in low:
        core = "Laser"
    elif "vj_loop_edm" in low:
        core = "EDM Loop"
    else:
        core = base[:14]
    return core

def build_pool(dirname, ext_dir_rel):
    files = sorted(glob.glob(os.path.join(ROOT, "media", dirname, "*.mp4")))
    pool = []
    for f in files:
        rel = "media/%s/%s" % (dirname, os.path.basename(f))
        thumb_name = os.path.splitext(os.path.basename(f))[0] + ".jpg"
        thumb_rel = "media/thumbs/%s" % thumb_name
        thumb = thumb_rel if os.path.exists(os.path.join(ROOT, "media", "thumbs", thumb_name)) else None
        pool.append({"src": rel, "label": label_from_filename(f), "thumb": thumb})
    return pool

def make_clip_pad(key, item, num):
    pad = {"key": key, "type": "clip", "src": item["src"], "bigText": str(num), "label": item["label"]}
    if item["thumb"]:
        pad["thumb"] = item["thumb"]
    return pad

def main():
    pool_1080 = build_pool("1080", "1080")
    pool_720 = build_pool("h264", "h264")
    print("found %d 1080p files, %d 720p(h264) files" % (len(pool_1080), len(pool_720)))
    if len(pool_1080) < 25:
        print("!! expected >=25 1080p files, found %d -- aborting, check media/1080/" % len(pool_1080))
        sys.exit(1)
    if len(pool_720) < 28:
        print("!! expected >=28 720p files, found %d -- aborting, check media/h264/" % len(pool_720))
        sys.exit(1)
    pool_1080 = pool_1080[:25]
    pool_720 = pool_720[:28]

    d = json.load(open(SHOW, encoding="utf-8"))
    banks = {b["name"]: b for b in d["banks"]}
    for needed in ("CLIPS", "MIX", "SYS", "1080 TEST"):
        if needed not in banks:
            print("!! bank %r not found -- show.json structure changed, aborting" % needed)
            sys.exit(1)

    split = {"CLIPS": 14, "MIX": 7, "SYS": 4}
    i1080 = 0
    for bank_name, count in split.items():
        bank = banks[bank_name]
        pads = bank["pads"]
        clip_keys_in_order = sorted(p["key"] for p in pads if p["type"] == "clip")
        keep_keys = clip_keys_in_order[:count]
        drop_keys = set(clip_keys_in_order[count:])
        new_pads = []
        cursor = 0
        for p in pads:
            if p["type"] != "clip":
                new_pads.append(p)
                continue
            if p["key"] in drop_keys:
                continue  # removed outright, per the migration note above
            item = pool_1080[i1080]
            i1080 += 1
            new_pads.append(make_clip_pad(p["key"], item, cursor + 1))
            cursor += 1
        bank["pads"] = new_pads
        print("%-6s -> %d 1080p clip pads (was %d 720p)" % (bank_name, count, len(clip_keys_in_order)))
    assert i1080 == 25, "did not consume all 25 1080p files (%d used)" % i1080

    page7 = banks["1080 TEST"]
    page7["name"] = "720p"
    keys_for_28 = list(range(0, 24)) + [26, 27, 28, 29]
    assert len(keys_for_28) == 28
    new_pads = []
    for idx, key in enumerate(keys_for_28):
        new_pads.append(make_clip_pad(key, pool_720[idx], idx + 1))
    page7["pads"] = new_pads
    print("720p (was '1080 TEST') -> 28 h264/720p clip pads across keys 0-23,26-29")

    banks["4K TEST"]["name"] = "4K"
    print("4K TEST renamed to '4K' (content untouched)")

    stamp = time.strftime("%Y%m%d-%H%M%S")
    bak = os.path.join(ROOT, "show.json.bak-%s" % stamp)
    os.replace(SHOW, bak) if False else None
    import shutil
    shutil.copy2(SHOW, bak)
    print("backup written: %s" % bak)

    json.dump(d, open(SHOW, "w", encoding="utf-8"), indent=1)
    print("show.json rewritten.")

if __name__ == "__main__":
    main()
