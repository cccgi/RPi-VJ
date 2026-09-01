#!/usr/bin/env bash
# Cut every clip at its detected seamless loop point.
#
#   python3 gig/tools/find-loops.py        # writes media/loops.json  (do this first)
#   ./gig/tools/loop-cut.sh                # cut confident clips, keep source height
#   ./gig/tools/loop-cut.sh --height 1080  # cut AND scale to 1080p
#   ./gig/tools/loop-cut.sh --all          # also cut the "uncertain" ones
#
# Cuts are FRAME-EXACT (-frames:v N), which requires a re-encode -- a stream
# copy can only cut on a keyframe, and being a few frames out is precisely the
# error that makes a wrap visible. Originals are moved to media/h264-full/,
# nothing is deleted, filenames are unchanged so show.json still matches.
set -euo pipefail
cd "$(dirname "$0")/.."

HEIGHT=0
ONLY_CONFIDENT=1
CRF=20
PRESET=veryfast
while [ $# -gt 0 ]; do
  case "$1" in
    --height) HEIGHT="$2"; shift 2 ;;
    --all)    ONLY_CONFIDENT=0; shift ;;
    --crf)    CRF="$2"; shift 2 ;;
    --preset) PRESET="$2"; shift 2 ;;
    *) echo "unknown option: $1"; exit 1 ;;
  esac
done

SRC=media/h264
KEEP=media/h264-full
LOOPS=media/loops.json

[ -f "$LOOPS" ] || { echo "!! $LOOPS not found - run find-loops.py first"; exit 1; }
command -v ffmpeg >/dev/null || { echo "!! ffmpeg not found"; exit 1; }
mkdir -p "$KEEP"

echo "== cutting at detected loop points"
[ "$HEIGHT" != "0" ] && echo "== scaling to ${HEIGHT}p (CRF $CRF, preset $PRESET)"
echo

python3 - "$LOOPS" "$ONLY_CONFIDENT" <<'PY' > /tmp/loopcut.list
import json, sys
data = json.load(open(sys.argv[1]))
only_conf = sys.argv[2] == "1"
for name, v in sorted(data.items()):
    lp = v.get("loop")
    if not lp:
        continue
    if only_conf and not lp.get("confident"):
        continue
    print("%s\t%d\t%s\t%s" % (name, lp["frames"], lp["seconds"], lp.get("confident")))
PY

count=$(wc -l < /tmp/loopcut.list)
[ "$count" -gt 0 ] || { echo "!! nothing to cut (no confident loops; try --all)"; exit 1; }
echo "== $count clip(s) to cut"
echo

done_n=0
while IFS=$'\t' read -r name frames secs conf; do
  f="$SRC/$name"
  [ -f "$f" ] || { echo "-- $name missing, skipping"; continue; }
  [ -f "$KEEP/$name" ] && { echo "-- $name already cut, skipping"; continue; }

  vf="null"
  [ "$HEIGHT" != "0" ] && vf="scale=-2:$HEIGHT:flags=lanczos"

  tmp="$SRC/.cut_$name"
  if ffmpeg -y -v error -i "$f" -frames:v "$frames" \
      -vf "$vf" -c:v libx264 -preset "$PRESET" -crf "$CRF" \
      -pix_fmt yuv420p -g 60 -an -movflags +faststart "$tmp"; then
    mv "$f" "$KEEP/$name"
    mv "$tmp" "$f"
    printf "== %-40s %6ss loop  %5sMB -> %sMB  %s\n" \
      "$(echo "$name" | cut -c1-40)" "$secs" \
      "$(( $(stat -c%s "$KEEP/$name") / 1048576 ))" \
      "$(( $(stat -c%s "$f") / 1048576 ))" \
      "$([ "$conf" = "True" ] && echo seamless || echo UNCERTAIN)"
    done_n=$((done_n+1))
  else
    rm -f "$tmp"; echo "!! FAILED: $name (left untouched)"
  fi
done < /tmp/loopcut.list

echo
echo "== cut $done_n clips"
echo "== library now: $(du -sh "$SRC" | cut -f1)   originals: $(du -sh "$KEEP" | cut -f1)"
echo "== restart the kiosk, then check each clip wraps without a jump."
echo "== to undo:  mv $KEEP/*.mp4 $SRC/"
