#!/usr/bin/env bash
# Six short placeholder clips so the clip pads are testable before the real
# library exists (G-16). Run anywhere ffmpeg exists; outputs to gig/media/.
set -euo pipefail
OUT="$(cd "$(dirname "$0")/.." && pwd)/media"; mkdir -p "$OUT"
command -v ffmpeg >/dev/null || { echo "!! ffmpeg not installed"; exit 1; }
for i in 1 2 3 4 5 6; do
  f="$OUT/clip_0$i.mp4"
  [ -f "$f" ] && { echo "skip $f"; continue; }
  ffmpeg -hide_banner -loglevel error -y \
    -f lavfi -i "testsrc2=size=1280x720:rate=30:duration=8,hue=h=$((i*57))" \
    -c:v libx264 -preset veryfast -crf 23 -g 15 -keyint_min 15 -sc_threshold 0 \
    -pix_fmt yuv420p -movflags +faststart -an "$f"
  echo "made $f"
done
ls -la "$OUT"
