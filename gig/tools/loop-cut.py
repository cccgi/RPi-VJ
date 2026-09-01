#!/usr/bin/env python3
"""Cut clips at their detected seamless loop point, optionally rescaling.

Runs on Windows or the Pi -- only ffmpeg and Python are needed. Encoding 4K
sources belongs on the PC, not on the Pi.

    # 1. detect loop points in the 4K masters
    python find-loops.py --dir "E:\\__VJ\\_4K VJ Backgrounds" --out loops.json

    # 2. read the table, then cut + downscale to 1080p
    python loop-cut.py --src "E:\\__VJ\\_4K VJ Backgrounds" \
                       --dst "C:\\Users\\Admin\\Projects\\rpi-vj\\gig\\media\\h264" \
                       --loops loops.json --height 1080

Cuts are FRAME-EXACT (-frames:v N). A stream copy can only cut on a keyframe,
and being a few frames out is exactly what makes a wrap visible -- so this
re-encodes, which we want anyway when rescaling.

Sources are never modified; output goes to --dst.
"""
import argparse, json, os, shutil, subprocess, sys


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--src", required=True, help="folder of source clips")
    ap.add_argument("--dst", required=True, help="folder to write cut clips into")
    ap.add_argument("--loops", required=True, help="loops.json from find-loops.py")
    ap.add_argument("--height", type=int, default=0, help="output height, e.g. 1080. 0 = keep source")
    ap.add_argument("--crf", type=int, default=20, help="quality, lower is better (18-23 sensible)")
    ap.add_argument("--preset", default="medium")
    ap.add_argument("--all", action="store_true", help="include loops flagged uncertain")
    ap.add_argument("--fallback", type=float, default=0,
                    help="seconds to use for clips with NO detected loop (0 = skip them)")
    ap.add_argument("--min-length", type=float, default=0,
                    help="shortest acceptable output (s). A 2s fundamental is "
                         "seamless but boring; this takes WHOLE MULTIPLES of it "
                         "so the result stays seamless while lasting longer.")
    ap.add_argument("--show-json", default=None,
                    help="verify output names against this show.json before encoding")
    ap.add_argument("--force", action="store_true",
                    help="encode even if names do not match show.json")
    ap.add_argument("--crop-quarter", action="store_true",
                    help="crop clips flagged as mirror_hv_tl to their top-left 1/4 quadrant")
    ap.add_argument("--force-crop-quarter", action="store_true",
                    help="crop every input to top-left 1/4; use only for a curated quad-mirror folder")
    a = ap.parse_args()

    if not shutil.which("ffmpeg"):
        print("!! ffmpeg not on PATH"); return 1
    loops = json.load(open(a.loops))
    os.makedirs(a.dst, exist_ok=True)

    todo, skipped = [], []
    for name, v in sorted(loops.items()):
        lp = v.get("loop")
        if lp and (lp.get("confident") or a.all):
            todo.append((name, int(lp["frames"]), lp["seconds"], bool(lp.get("confident"))))
        elif a.fallback > 0:
            fps = v.get("fps", 30.0)
            todo.append((name, int(a.fallback * fps), a.fallback, False))
        else:
            skipped.append(name)

    if not todo:
        print("!! nothing to cut. Use --all, or --fallback 60."); return 1

    # NAME MAPPING.
    # The 4K masters are named with spaces; the show's clips use underscores
    # ("A B  C.mp4" -> "A_B__C.mp4"). show.json and the thumbnails are keyed by
    # filename, so writing the master's name would leave every clip pad
    # pointing at a file that does not exist -- discovered only after an hour
    # of encoding. Map first, verify first, encode last.
    def out_name(src_name):
        return src_name.replace(" ", "_")

    if a.show_json:
        try:
            show = json.load(open(a.show_json))
        except Exception as e:
            print(f"!! could not read {a.show_json}: {e}"); return 1
        wanted = set()
        for bank in show.get("banks", []):
            for pad in bank.get("pads", []):
                if pad.get("type") == "clip" and pad.get("src"):
                    wanted.add(os.path.basename(pad["src"]))

        mapped = {out_name(n) for n, _, _, _ in todo}
        missing = sorted(wanted - mapped)
        extra = sorted(mapped - wanted)

        print(f"== show.json references {len(wanted)} clips")
        print(f"== this run would produce {len(mapped)}")
        if missing:
            print(f"!! {len(missing)} clip(s) the show needs would NOT be produced:")
            for m in missing[:10]:
                print("   " + m[:72])
            if len(missing) > 10:
                print(f"   ... and {len(missing) - 10} more")
        if extra:
            print(f"-- {len(extra)} produced file(s) are not referenced by the show "
                  f"(harmless, just unused)")
        if missing and not a.force:
            print()
            print("!! STOPPING before encoding. Those pads would point at missing files.")
            print("!! Re-run with --force to encode anyway, or tell me and I will")
            print("!! regenerate show.json to match the new names.")
            return 1
        if not missing:
            print("== every clip the show needs will be produced. Names line up.")
        print()

    print(f"== {len(todo)} to cut, {len(skipped)} skipped")
    if a.height:
        print(f"== scaling to {a.height}p, CRF {a.crf}, preset {a.preset}")
    print()

    # PROMOTE SHORT LOOPS TO WHOLE MULTIPLES.
    # Several clips have a genuine 2.0-7.5s fundamental. That is correct, but a
    # 2-second loop on a dancefloor reads as a stutter. Because the source
    # contains the loop many times over, taking k whole periods is still
    # frame-exact and still seamless -- just longer.
    if a.min_length > 0:
        promoted = []
        for name, frames, secs, conf in todo:
            fps = loops.get(name, {}).get("fps", 30.0)
            dur = loops.get(name, {}).get("duration", 0)
            if secs > 0 and secs < a.min_length:
                k = int(a.min_length / secs) + (1 if a.min_length % secs else 0)
                while k > 1 and (frames * k) / fps > dur:
                    k -= 1
                if k > 1:
                    promoted.append((name, frames * k, round(secs * k, 3), conf))
                    continue
            promoted.append((name, frames, secs, conf))
        todo = promoted

    ok = fail = 0
    for i, (name, frames, secs, conf) in enumerate(todo, 1):
        src = os.path.join(a.src, name)
        dst = os.path.join(a.dst, name.replace(" ", "_"))
        if not os.path.exists(src):
            print(f"-- [{i}/{len(todo)}] {name[:44]}  MISSING, skipped"); continue

        clip_meta = loops.get(name, {})
        is_sym = (clip_meta.get("layout") == "mirror_hv_tl")
        vf_filters = []
        crop_quarter = a.crop_quarter and (is_sym or a.force_crop_quarter)
        if crop_quarter:
            # Crop to top-left quadrant: width/2, height/2, x=0, y=0
            vf_filters.append("crop=iw/2:ih/2:0:0")
            if a.height:
                vf_filters.append(f"scale=-2:{a.height // 2}:flags=lanczos")
        elif a.height:
            vf_filters.append(f"scale=-2:{a.height}:flags=lanczos")

        vf = ",".join(vf_filters) if vf_filters else "null"
        cmd = ["ffmpeg", "-y", "-v", "error", "-i", src,
               "-frames:v", str(frames), "-vf", vf,
               "-c:v", "libx264", "-preset", a.preset, "-crf", str(a.crf),
               "-pix_fmt", "yuv420p", "-g", "60", "-an",
               "-movflags", "+faststart", dst]
        sym_note = " [1/4 CROP]" if crop_quarter else ""
        print(f"-- [{i}/{len(todo)}] {name[:44]:<46} {secs:>7.2f}s "
              f"{'seamless' if conf else 'UNCERTAIN'}{sym_note} ... ", end="", flush=True)
        r = subprocess.run(cmd, capture_output=True, text=True)
        if r.returncode == 0 and os.path.exists(dst):
            mb_in = os.path.getsize(src) / 1048576
            mb_out = os.path.getsize(dst) / 1048576
            print(f"{mb_in:.0f}MB -> {mb_out:.0f}MB")
            ok += 1
        else:
            print("FAILED")
            if r.stderr:
                print("     " + r.stderr.strip().split("\n")[-1][:160])
            fail += 1

    print()
    print(f"== done: {ok} written, {fail} failed, {len(skipped)} skipped")
    if skipped:
        print("== no loop detected (left out entirely):")
        for s in skipped:
            print("   " + s[:70])
        print("   Re-run with --fallback 60 to include them as plain 60s cuts.")
    return 0 if fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
