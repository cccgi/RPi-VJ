#!/usr/bin/env bash
# Trim the clip library to short loops. Run ON THE PI (or anywhere with ffmpeg).
#   ./gig/tools/trim-clips.sh          60-second loops (default)
#   ./gig/tools/trim-clips.sh 45       45-second loops
#
# WHY THIS IS THE ROOT FIX, not a workaround
# ------------------------------------------
# The library is 28 clips of 5-10 minutes, 230-290MB each, 5.3GB total, on a
# Pi with 2GB of RAM. Pressing a clip asks Chromium to open a ~290MB media
# resource and start buffering it. That is the whole crash: idle -> press a
# clip -> renderer runs out of memory -> "Aw, Snap! Error code: 5".
#
# Nothing about a VJ loop needs to be 10 minutes long. These are seamless
# abstract loops; 60 seconds of one is indistinguishable from 600 seconds of
# it on a dancefloor, and the deck loops it anyway. Trimming takes the library
# from 5.3GB to roughly 500MB and each clip from ~290MB to ~30MB.
#
# This is a STREAM COPY (-c copy): no re-encoding, no quality loss, a second
# or two per file. Cutting on a keyframe means the exact length varies by a
# fraction of a second, which does not matter for a loop.
#
# Originals are moved to media/h264-full/ -- nothing is deleted.
set -euo pipefail
cd "$(dirname "$0")/.."

LEN="${1:-60}"
SRC=media/h264
KEEP=media/h264-full

command -v ffmpeg >/dev/null || { echo "!! ffmpeg not found"; exit 1; }
[ -d "$SRC" ] || { echo "!! $SRC not found"; exit 1; }
mkdir -p "$KEEP"

echo "== trimming to ${LEN}s loops (stream copy, no re-encode)"
echo "== originals preserved in $KEEP"
echo

n=0; kept=0
for f in "$SRC"/*.mp4; do
  b="$(basename "$f")"
  [ -f "$KEEP/$b" ] && { echo "-- $b already trimmed, skipping"; continue; }

  dur="$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$f" 2>/dev/null || echo 0)"
  if awk "BEGIN{exit !($dur <= $LEN + 2)}"; then
    echo "-- $b is already ${dur%.*}s, leaving alone"
    kept=$((kept+1)); continue
  fi

  tmp="$SRC/.trim_$b"
  if ffmpeg -y -v error -t "$LEN" -i "$f" -c copy -movflags +faststart "$tmp"; then
    mv "$f" "$KEEP/$b"
    mv "$tmp" "$f"
    printf "== %s  %ss -> %ss  %sMB -> %sMB\n" "$(echo "$b" | cut -c1-38)" \
      "${dur%.*}" "$LEN" \
      "$(( $(stat -c%s "$KEEP/$b") / 1048576 ))" \
      "$(( $(stat -c%s "$f") / 1048576 ))"
    n=$((n+1))
  else
    rm -f "$tmp"; echo "!! FAILED: $b (left untouched)"
  fi
done

echo
echo "== trimmed $n, left $kept already-short"
echo "== library now: $(du -sh "$SRC" | cut -f1)   originals: $(du -sh "$KEEP" | cut -f1)"
echo
echo "The show references media/h264/<name>.mp4 -- filenames are unchanged, so"
echo "show.json and the thumbnails still match. Restart the kiosk and test."
echo "To undo:  mv $KEEP/*.mp4 $SRC/"
