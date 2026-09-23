#!/usr/bin/env python3
"""Build/refresh the isolated Stream Deck music page from media/music/."""
import argparse
import json
import os
import re
from pathlib import Path

EXTENSIONS = {".mp3", ".flac", ".wav", ".m4a", ".aac", ".ogg", ".opus"}
# Buttons 14-16 are appliance controls and 17-24 are direct page jumps.
KEYS = list(range(13))


def label_for(path):
    # Preserve the filename wording/punctuation; the icon renderer wraps it.
    return " ".join(path.stem.replace("_", " ").split()) or "Song"


def song_sort_key(path):
    """Pin explicitly numbered 1.-5. tracks first, then sort the remainder."""
    match = re.match(r"^([1-5])\.\s", path.name)
    if match:
        return (0, int(match.group(1)), path.name.casefold())
    return (1, 0, path.name.casefold())


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default="media/music", help="music folder relative to gig/")
    ap.add_argument("--bank", default="MUSIC")
    args = ap.parse_args()

    gig = Path(__file__).resolve().parents[1]
    library = (gig / args.dir).resolve()
    library.mkdir(parents=True, exist_ok=True)
    songs = sorted((p for p in library.iterdir() if p.is_file() and p.suffix.lower() in EXTENSIONS),
                   key=song_sort_key)
    if len(songs) > len(KEYS):
        raise SystemExit("!! %d songs found; one music page supports 13 songs because buttons 14-32 are controls" % len(songs))

    show_path = gig / "show.json"
    show = json.loads(show_path.read_text(encoding="utf-8"))
    old = [i for i, b in enumerate(show["banks"]) if b.get("type") == "music" or b.get("name") == args.bank]
    insert_at = old[0] if old else len(show["banks"])
    show["banks"] = [b for b in show["banks"] if not (b.get("type") == "music" or b.get("name") == args.bank)]
    pads = []
    for key, song in zip(KEYS, songs):
        pads.append({
            "key": key,
            "type": "audio",
            "src": args.dir.rstrip("/") + "/" + song.name,
            "label": label_for(song),
            "_sizeMB": song.stat().st_size // 1048576,
        })
    show["banks"].insert(min(insert_at, len(show["banks"])), {
        "name": args.bank, "type": "music", "pads": pads,
    })
    show_path.write_text(json.dumps(show, indent=1) + "\n", encoding="utf-8")
    print("== music page %d: %d songs on keys %s" %
          (min(insert_at, len(show["banks"]) - 1) + 1, len(pads),
           ",".join(str(p["key"]) for p in pads) or "(empty)"))


if __name__ == "__main__":
    main()
