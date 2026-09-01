#!/usr/bin/env bash
# G-02 — produce the five test encodes for the codec probe (gig/probe/codec.html).
#
# Usage: make-test-clips.sh <input-clip> <output-dir>
#
# From one representative 1080p input clip, produces:
#   1080p60 HEVC, 1080p30 HEVC, 1080p60 H.264, 720p60 H.264, 720p30 H.264
# Closed GOP of 15 for clean loop points on all encodes.

set -euo pipefail

if [ $# -ne 2 ]; then
  echo "usage: $0 <input-clip> <output-dir>" >&2
  exit 1
fi

IN="$1"
OUT="$2"

if [ ! -f "$IN" ]; then
  echo "error: input clip not found: $IN" >&2
  exit 1
fi

mkdir -p "$OUT"

GOP_OPTS=(-g 15 -keyint_min 15 -sc_threshold 0)

echo "== 1080p60 HEVC =="
ffmpeg -y -i "$IN" \
  -c:v libx265 -preset medium -crf 20 -tag:v hvc1 "${GOP_OPTS[@]}" \
  -an "$OUT/1080p60_hevc.mp4"

echo "== 1080p30 HEVC =="
ffmpeg -y -i "$IN" \
  -c:v libx265 -preset medium -crf 20 -tag:v hvc1 "${GOP_OPTS[@]}" -r 30 \
  -an "$OUT/1080p30_hevc.mp4"

echo "== 1080p60 H.264 =="
ffmpeg -y -i "$IN" \
  -c:v libx264 -preset medium -crf 20 "${GOP_OPTS[@]}" \
  -an "$OUT/1080p60_h264.mp4"

echo "== 720p60 H.264 =="
ffmpeg -y -i "$IN" \
  -c:v libx264 -preset medium -crf 20 "${GOP_OPTS[@]}" -vf scale=1280:720 \
  -an "$OUT/720p60_h264.mp4"

echo "== 720p30 H.264 =="
ffmpeg -y -i "$IN" \
  -c:v libx264 -preset medium -crf 20 "${GOP_OPTS[@]}" -vf scale=1280:720 -r 30 \
  -an "$OUT/720p30_h264.mp4"

echo "done: $OUT"
