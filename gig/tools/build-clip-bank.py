#!/usr/bin/env python3
"""Add Stream Deck bank(s) of clips from a media folder.

Splits across as many pages as the folder needs (24 clips per page, the top
three rows), so a 40-clip drop becomes "NEON 1" and "NEON 2" automatically.
Re-running replaces those banks rather than duplicating them, so it is safe to
run again after adding more clips.

A bench measures a shader on a blank canvas. This measures the thing that
actually happens on the night: a 4K clip decoding in software while the
compositor runs, on a 2GB Pi. Nothing else tells you whether 4K is viable.

    python3 gig/tools/build-4k-test-bank.py                 # add/refresh the bank
    python3 gig/tools/build-4k-test-bank.py --remove        # take it away again

Keys 0-23 (top three rows) become clips. The bottom row is left empty so the
system row and page navigation still behave.
"""
import argparse, json, os, sys

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default="media/4k", help="clip folder, relative to gig/")
    ap.add_argument("--bank", default="4K TEST", help="bank (page) name")
    ap.add_argument("--label", default=None, help="key label prefix, default derived from --bank")
    ap.add_argument("--count", type=int, default=28,
                    help="clips per page (28 = top three rows 0-23 + bottom row slots 26-29)")
    ap.add_argument("--quad", action="store_true",
                    help="mark all clips in this bank with layout: mirror_hv_tl for full-screen quad-mirror playback")
    ap.add_argument("--page", type=int, default=0,
                    help="1-based page position; 0 appends (default)")
    ap.add_argument("--move-only", action="store_true",
                    help="move an existing single bank to --page without rebuilding its pads")
    ap.add_argument("--remove", action="store_true")
    a = ap.parse_args()
    BANK = a.bank
    prefix = a.label or ("".join(c for c in BANK.split()[0] if c.isalnum()) or "C")

    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    show_path = os.path.join(root, "show.json")
    show = json.load(open(show_path))

    # Drop any existing bank for this set, including multi-page variants
    # ("NEON", "NEON 1", "NEON 2"), so re-running replaces instead of stacking.
    def owned(n):
        return n == BANK or (n.startswith(BANK + " ") and n[len(BANK) + 1:].isdigit())
    old_banks = show["banks"]
    owned_indices = [i for i, b in enumerate(old_banks) if owned(b.get("name", ""))]
    if a.move_only:
        if a.page <= 0:
            print("!! --move-only requires a positive --page"); return 1
        exact = [i for i, b in enumerate(old_banks) if b.get("name", "") == BANK]
        if len(exact) != 1:
            print(f"!! --move-only expected exactly one '{BANK}' bank, found {len(exact)}"); return 1
        bank = show["banks"].pop(exact[0])
        insert_at = min(len(show["banks"]), a.page - 1)
        show["banks"].insert(insert_at, bank)
        json.dump(show, open(show_path, "w"), indent=1)
        print(f"== moved existing bank '{BANK}' to page {insert_at + 1}; pads preserved")
        return 0
    show["banks"] = [b for b in old_banks if not owned(b.get("name", ""))]
    if a.remove:
        json.dump(show, open(show_path, "w"), indent=1)
        print(f"== removed the '{BANK}' bank; {len(show['banks'])} banks remain")
        return 0

    src = os.path.join(root, a.dir)
    if not os.path.isdir(src):
        print(f"!! {src} not found. Encode the clips there first, e.g.:")
        print('   python gig\\tools\\loop-cut.py --src "E:\\__VJ\\_4K VJ Backgrounds" \\')
        print(f"          --dst gig\\{a.dir.replace('/', chr(92))} --loops loops.json --min-length 15")
        return 1

    clips = sorted(f for f in os.listdir(src) if f.lower().endswith(".mp4"))
    if not clips:
        print(f"!! no .mp4 files in {src}"); return 1

    # Slots available per page: 0-23 (rows 1-3), then 26, 27, 28, 29 (row 4)
    # Keys 24 & 25 are page nav (< and >); keys 30 & 31 are RESET and BLACK.
    KEY_SLOTS = list(range(24)) + [26, 27, 28, 29]
    per_page = min(len(KEY_SLOTS), a.count)

    # Optional loops.json lookup for layout metadata (e.g. mirror_hv_tl)
    loops_meta = {}
    loops_file = f"{BANK.lower().split()[0]}-loops.json"
    for candidate in [os.path.join(root, loops_file), os.path.join(root, "media", "loops.json")]:
        if os.path.isfile(candidate):
            try:
                loops_meta = json.load(open(candidate))
                break
            except Exception:
                pass

    pages = [clips[i:i + per_page] for i in range(0, len(clips), per_page)]
    made = []
    for pi, chunk in enumerate(pages):
        pads = []
        for i, name in enumerate(chunk):
            mb = os.path.getsize(os.path.join(src, name)) // 1048576
            n = pi * per_page + i + 1
            slot_key = KEY_SLOTS[i]
            pad_def = {
                "key": slot_key,
                "type": "clip",
                "src": f"{a.dir}/{name}",
                "bigText": str(n),
                # Short label: on a 96px key the number is what you read anyway.
                "label": f"{prefix}{n}",
                "_sizeMB": mb,
            }
            # Check if source video had detected quad symmetry layout or user passed --quad
            if a.quad:
                pad_def["layout"] = "mirror_hv_tl"
            else:
                meta = loops_meta.get(name) or loops_meta.get(name.replace("_", " "))
                if meta and isinstance(meta, dict) and meta.get("layout") and meta.get("layout") != "direct":
                    pad_def["layout"] = meta["layout"]
            pads.append(pad_def)
        bank_name = BANK if len(pages) == 1 else f"{BANK} {pi + 1}"
        new_bank = {"name": bank_name, "pads": pads}
        if a.page > 0:
            insert_at = min(len(show["banks"]), (a.page - 1) + pi)
            show["banks"].insert(insert_at, new_bank)
        elif owned_indices:
            insert_at = min(len(show["banks"]), owned_indices[0] + pi)
            show["banks"].insert(insert_at, new_bank)
        else:
            show["banks"].append(new_bank)
        made.append((bank_name, len(pads)))

    json.dump(show, open(show_path, "w"), indent=1)

    total = sum(os.path.getsize(os.path.join(src, c)) for c in clips) // 1048576
    names = [b["name"] for b in show["banks"]]
    for bank_name, cnt in made:
        used_keys = [p["key"] for b in show["banks"] if b["name"] == bank_name for p in b["pads"]]
        print(f"== bank '{bank_name}' = page {names.index(bank_name) + 1}, "
              f"{cnt} clips on keys {','.join(str(k) for k in used_keys)}")
    print(f"== {len(clips)} clips, {total} MB total, "
          f"{total // max(1, len(clips))} MB average")
    print()
    print("WHAT TO WATCH ON THE TV")
    print("  The HUD fps figure, and the log:")
    print("    ssh rpi@rpi-vj.local 'grep \"engine fps\" ~/vj-results/show.log | tail -20'")
    print("  60 fps holding with a 4K clip playing  -> 4K is viable")
    print("  30-50 fps, or fps sagging after a while -> 1080p is the right call")
    print("  a crash on pressing a clip              -> 4K files are too large for 2GB")
    print()
    print("Press several clips in succession, and leave one running for a few")
    print("minutes: the decode cost shows immediately, the memory cost does not.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
