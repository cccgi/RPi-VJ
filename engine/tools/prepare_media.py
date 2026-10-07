#!/usr/bin/env python3
"""Offline HEVC preparation. Never modifies source media or overwrites outputs."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile


def encode_command(source, destination):
    return [
        "ffmpeg", "-nostdin", "-v", "error", "-n", "-i", str(source),
        "-map", "0:v:0", "-an", "-sn", "-dn",
        "-vf", "scale=1920:1080:force_original_aspect_ratio=decrease:force_divisible_by=2,"
        "pad=1920:1080:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=60",
        "-c:v", "libx265", "-preset", "medium", "-crf", "20",
        "-pix_fmt", "yuv420p", "-profile:v", "main",
        "-x265-params", "keyint=30:min-keyint=30:scenecut=0:open-gop=0:bframes=0",
        "-tag:v", "hvc1", "-movflags", "+faststart", str(destination),
    ]


def verify(path):
    result = subprocess.run([
        "ffprobe", "-v", "error", "-select_streams", "v:0",
        "-show_entries", "stream=codec_name,width,height,pix_fmt,avg_frame_rate",
        "-of", "json", str(path),
    ], check=True, capture_output=True, text=True)
    streams = json.loads(result.stdout)["streams"]
    if not streams:
        raise ValueError("encoded file has no video")
    stream = streams[0]
    expected = {"codec_name": "hevc", "width": 1920, "height": 1080,
                "pix_fmt": "yuv420p", "avg_frame_rate": "60/1"}
    if any(stream.get(key) != value for key, value in expected.items()):
        raise ValueError(f"encoded stream does not match prepared-media profile: {stream}")
    # Decode the entire prepared file; a metadata-only probe cannot find corrupt late frames.
    subprocess.run(["ffmpeg", "-nostdin", "-v", "error", "-xerror", "-i", str(path),
                    "-map", "0:v:0", "-f", "null", "-"], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path, help="New output directory, outside the source tree")
    args = parser.parse_args()
    source, output = args.source.resolve(), args.output.resolve()
    if not source.is_dir():
        parser.error("source must be a directory")
    if output == source or source in output.parents or output in source.parents:
        parser.error("source and output must be separate, non-overlapping directories")
    if output.exists():
        parser.error("output already exists; choose a new directory")
    clips = sorted(p for p in source.rglob("*") if p.suffix.lower() in {".mp4", ".mov", ".mkv"})
    if not clips:
        parser.error("no video files found")
    output.mkdir(parents=True)
    # The final manifest is published only after every clip passes verification.
    manifest = {"version": 1, "audio_device": None, "clips": []}
    for index, clip in enumerate(clips):
        name = f"{index:04d}-{clip.stem}"
        destination = output / f"{name}.mp4"
        thumbnail = output / f"{name}.jpg"
        with tempfile.TemporaryDirectory(prefix="prepare-", dir=output) as work:
            temporary = Path(work) / "clip.mp4"
            subprocess.run(encode_command(clip, temporary), check=True)
            verify(temporary)
            subprocess.run(["ffmpeg", "-nostdin", "-v", "error", "-n", "-i", str(temporary),
                            "-frames:v", "1", "-vf", "scale=96:96", str(thumbnail)], check=True)
            temporary.rename(destination)
        manifest["clips"].append({"label": clip.stem, "path": destination.name, "thumbnail": thumbnail.name})
        print(f"Prepared {index + 1}/{len(clips)}: {clip}", flush=True)
    (output / "show.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
