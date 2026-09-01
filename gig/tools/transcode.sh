#!/usr/bin/env bash
# G-16 — Transcode Clip Library
#
# 2026-08-26: The shipped library is 28 clips at 1280x720 H.264 30fps and is CORRECT
# as built — re-encoding it to 1080p60 would make Pi playback harder, not easier
# (the Pi has no H.264 hardware decode, so every pixel is software-decoded).
# Default resolution and frame rate are set to 1280x720 @ 30fps.
#
# Generates both H.264 and HEVC ladders (due to unverified Pi Chromium HEVC support) + manifest.json.

set -euo pipefail

INPUT_DIR="${1:-}"
OUTPUT_DIR="${2:-}"
CODEC_CHOICE="${3:-both}" # "h264", "hevc", or "both"

if [ -z "$INPUT_DIR" ] || [ -z "$OUTPUT_DIR" ]; then
  echo "Usage: $0 <input-dir> <output-dir> [h264|hevc|both]"
  echo "Example: $0 ./raw_clips ./gig/media both"
  exit 1
fi

if [ ! -d "$INPUT_DIR" ]; then
  echo "Error: Input directory '$INPUT_DIR' does not exist." >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
if [ "$CODEC_CHOICE" = "both" ] || [ "$CODEC_CHOICE" = "h264" ]; then
  mkdir -p "$OUTPUT_DIR/h264"
fi
if [ "$CODEC_CHOICE" = "both" ] || [ "$CODEC_CHOICE" = "hevc" ]; then
  mkdir -p "$OUTPUT_DIR/hevc"
fi

echo "========================================================"
echo "G-16 Transcode Pipeline Starting"
echo "Input : $INPUT_DIR"
echo "Output: $OUTPUT_DIR"
echo "Codec : $CODEC_CHOICE"
echo "========================================================"

TARGET_WIDTH=1280
TARGET_HEIGHT=720
TARGET_FPS=30
GOP_SIZE=15

# Common ffmpeg flags strictly required by G-16 spec:
# -g 15 -keyint_min 15 -sc_threshold 0 for closed-GOP loops
# -an to strip audio
# -movflags +faststart for instant header parsing in Chromium
# -pix_fmt yuv420p for universal GPU texture upload compatibility
COMMON_FLAGS=(
  -g "$GOP_SIZE"
  -keyint_min "$GOP_SIZE"
  -sc_threshold 0
  -an
  -movflags +faststart
  -pix_fmt yuv420p
)

VF_FILTER="scale=${TARGET_WIDTH}:${TARGET_HEIGHT}:force_original_aspect_ratio=decrease,pad=${TARGET_WIDTH}:${TARGET_HEIGHT}:(ow-iw)/2:(oh-ih)/2:black,fps=${TARGET_FPS}"

transcode_clip() {
  local in_file="$1"
  local out_file="$2"
  local codec="$3"

  local out_dir
  out_dir="$(dirname "$out_file")"
  mkdir -p "$out_dir"

  # Skip if output exists and is newer than input
  if [ -f "$out_file" ] && [ "$out_file" -nt "$in_file" ]; then
    echo "  [SKIP] Newer output exists: $(basename "$out_file")"
    return 0
  fi

  echo "  [TRANSCODE] -> $(basename "$out_file") ($codec)"

  local encoder_flags=()
  if [ "$codec" = "h264" ]; then
    encoder_flags=(
      -c:v libx264
      -preset medium
      -crf 20
      -profile:v high
      -level 4.2
    )
  elif [ "$codec" = "hevc" ]; then
    encoder_flags=(
      -c:v libx265
      -preset medium
      -crf 22
      -tag:v hvc1
    )
  else
    echo "Error: Unknown codec $codec" >&2
    return 1
  fi

  ffmpeg -y -v warning -i "$in_file" \
    -vf "$VF_FILTER" \
    "${COMMON_FLAGS[@]}" \
    "${encoder_flags[@]}" \
    "$out_file"
}

# Find video files
FILES=()
while IFS= read -r -d '' file; do
  FILES+=("$file")
done < <(find "$INPUT_DIR" -type f \( -iname "*.mp4" -o -iname "*.mov" -o -iname "*.mkv" -o -iname "*.webm" -o -iname "*.avi" -o -iname "*.m4v" \) -print0)

TOTAL_FILES=${#FILES[@]}
echo "Found $TOTAL_FILES input video clip(s)."

if [ "$TOTAL_FILES" -eq 0 ]; then
  echo "No video files found in $INPUT_DIR."
fi

INDEX=0
for in_clip in "${FILES[@]}"; do
  INDEX=$((INDEX + 1))
  REL_PATH="${in_clip#"$INPUT_DIR"/}"
  BASE_NAME="$(basename "$in_clip")"
  STEM="${BASE_NAME%.*}"
  REL_DIR="$(dirname "$REL_PATH")"

  echo "[$INDEX/$TOTAL_FILES] Processing $REL_PATH"

  if [ "$CODEC_CHOICE" = "both" ] || [ "$CODEC_CHOICE" = "h264" ]; then
    OUT_H264="$OUTPUT_DIR/h264/${REL_DIR#/}/${STEM}.mp4"
    transcode_clip "$in_clip" "$OUT_H264" "h264"
  fi

  if [ "$CODEC_CHOICE" = "both" ] || [ "$CODEC_CHOICE" = "hevc" ]; then
    OUT_HEVC="$OUTPUT_DIR/hevc/${REL_DIR#/}/${STEM}.mp4"
    transcode_clip "$in_clip" "$OUT_HEVC" "hevc"
  fi
done

echo ""
echo "Generating manifest.json..."

# Generate manifest.json
python3 - << 'EOF' "$OUTPUT_DIR"
import json
import os
import subprocess
import sys

out_dir = sys.argv[1]
manifest_file = os.path.join(out_dir, "manifest.json")
manifest = {
    "version": "1.0",
    "target_resolution": "1280x720",
    "target_fps": 30,
    "gop_size": 15,
    "clips": []
}

for root, _, files in os.walk(out_dir):
    for f in sorted(files):
        if not f.lower().endswith(('.mp4', '.mov', '.webm')):
            continue
        full_path = os.path.join(root, f)
        rel_path = os.path.relpath(full_path, out_dir).replace('\\', '/')
        size_bytes = os.path.getsize(full_path)
        
        duration = None
        width = 1280
        height = 720
        fps = 30.0
        codec_name = "unknown"
        
        try:
            cmd = [
                "ffprobe", "-v", "error",
                "-show_entries", "stream=width,height,r_frame_rate,codec_name:format=duration",
                "-of", "json", full_path
            ]
            res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
            if res.returncode == 0:
                data = json.loads(res.stdout)
                streams = data.get("streams", [])
                if streams:
                    s = streams[0]
                    width = int(s.get("width", 1280))
                    height = int(s.get("height", 720))
                    codec_name = s.get("codec_name", "unknown")
                    r_fps = s.get("r_frame_rate", "30/1")
                    if "/" in r_fps:
                        num, den = map(float, r_fps.split("/"))
                        fps = num / den if den != 0 else 30.0
                    else:
                        fps = float(r_fps)
                fmt = data.get("format", {})
                duration = float(fmt.get("duration", 0.0))
        except Exception:
            pass

        manifest["clips"].append({
            "rel_path": rel_path,
            "filename": f,
            "codec": codec_name if codec_name != "unknown" else ("hevc" if "hevc" in rel_path else "h264"),
            "resolution": f"{width}x{height}",
            "width": width,
            "height": height,
            "fps": round(fps, 2),
            "duration": round(duration, 3) if duration is not None else None,
            "size_bytes": size_bytes,
            "size_mb": round(size_bytes / (1024 * 1024), 2)
        })

with open(manifest_file, "w", encoding="utf-8") as fp:
    json.dump(manifest, fp, indent=2)

print(f"Manifest written to {manifest_file} with {len(manifest['clips'])} clip entries.")
EOF

echo "========================================================"
echo "Transcode Complete"
echo "Total Library Size: $(du -sh "$OUTPUT_DIR" 2>/dev/null | cut -f1 || echo 'N/A')"
echo "========================================================"
