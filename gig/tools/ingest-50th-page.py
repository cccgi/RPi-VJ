#!/usr/bin/env python3
"""Replace the 50TH bank with only the current root-level birthday videos."""
import json
from pathlib import Path

GIG = Path(__file__).resolve().parents[1]
MEDIA = GIG / "media" / "50"
SHOW = GIG / "show.json"
KEYS = list(range(24))
def pad(key, src, big, label):
    path = GIG / src
    if not path.is_file():
        raise SystemExit("!! missing 50TH media: %s" % path)
    return {"key": key, "type": "clip", "src": src.replace("\\", "/"),
            "bigText": str(big), "label": label,
            "_sizeMB": path.stat().st_size // 1048576}


def main():
    videos = sorted((p for p in MEDIA.iterdir() if p.is_file() and p.suffix.lower() == ".mp4"),
                    key=lambda p: p.name.casefold())
    if len(videos) > len(KEYS):
        raise SystemExit("!! 50TH page exceeds 24 media buttons")
    pads = []
    for key, video in enumerate(videos):
        pads.append(pad(key, "media/50/" + video.name, key + 1, "BDAY VIDEO %d" % (key + 1)))

    show = json.loads(SHOW.read_text(encoding="utf-8"))
    matches = [i for i, bank in enumerate(show["banks"]) if bank.get("name") == "50TH"]
    if len(matches) != 1:
        raise SystemExit("!! expected exactly one 50TH bank, found %d" % len(matches))
    show["banks"][matches[0]] = {"name": "50TH", "pads": pads}
    SHOW.write_text(json.dumps(show, indent=1) + "\n", encoding="utf-8")
    print("== replaced page %d with %d current birthday videos" %
          (matches[0] + 1, len(pads)))


if __name__ == "__main__":
    main()
