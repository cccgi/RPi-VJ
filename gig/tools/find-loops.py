#!/usr/bin/env python3
"""Find the true seamless loop point of each clip.

WHY
---
These are rendered abstract loops: the visible motion repeats on some period,
and the file is that short loop already duplicated many times by whoever
exported it. Cutting at an arbitrary 60s lands mid-cycle and the wrap will
visibly jump. Cutting at the ACTUAL period wraps invisibly, because frame 0
and frame P are the same picture by construction.

METHOD
------
1. Decode the whole clip once, downscaled to a tiny greyscale thumbnail per
   frame (default 32x18). Motion structure survives; noise and compression
   detail do not, which is what we want.
2. Distance from frame 0 to every later frame. A true loop point is a sharp
   minimum: the picture is literally the same again.
3. Take the best candidates and VERIFY each by comparing a whole window of
   frames (default 45) starting at 0 against the window starting at P. One
   matching frame can be coincidence; forty-five in a row cannot.
4. Score the wrap itself: the frame-to-frame difference across the seam
   (P-1 -> 0) is compared against the clip's typical frame-to-frame
   difference. A seam that is no worse than an ordinary frame step is
   seamless by definition.

Prints a report and writes loops.json for loop-cut.sh to consume.
Nothing is modified.

    python3 gig/tools/find-loops.py                 # scan every clip
    python3 gig/tools/find-loops.py --min 3 --max 90
"""
import argparse, json, os, subprocess, sys

# numpy makes this fast but is NOT required. Needing a package install on a
# machine we cannot inspect is a bad dependency for a tool that has to work
# today, so there is a pure-Python fallback. The maths is identical; the
# fallback is a few seconds slower per clip, which is nothing next to the
# time spent decoding 4K video in the first place.
try:
    import numpy as np
    HAVE_NUMPY = True
except ImportError:
    np = None
    HAVE_NUMPY = False

W, H = 32, 18   # thumbnail size used for matching


def decode_gray(path, w=W, h=H, max_seconds=None):
    """Whole clip as an (frames, h*w) uint8 array of tiny greyscale frames."""
    cmd = ["ffmpeg", "-v", "error", "-i", path]
    if max_seconds and max_seconds > 0:
        cmd += ["-t", str(max_seconds)]
    cmd += [
           "-vf", f"scale={w}:{h}", "-pix_fmt", "gray",
           "-f", "rawvideo", "-"]
    out = subprocess.run(cmd, capture_output=True).stdout
    n = len(out) // (w * h)
    if n == 0:
        return None
    if HAVE_NUMPY:
        return np.frombuffer(out[: n * w * h], dtype=np.uint8).reshape(n, w * h)
    # list of bytes objects, one per frame
    px = w * h
    return [out[i * px:(i + 1) * px] for i in range(n)]


def fps_of(path):
    r = subprocess.run(
        ["ffprobe", "-v", "error", "-select_streams", "v:0",
         "-show_entries", "stream=r_frame_rate", "-of", "csv=p=0", path],
        capture_output=True, text=True).stdout.strip()
    try:
        a, b = r.split("/")
        return float(a) / float(b)
    except Exception:
        return 30.0


def _mad(a, b):
    """Mean absolute difference between two frames (pure-Python path)."""
    t = 0
    for x, y in zip(a, b):
        t += x - y if x > y else y - x
    return t / len(a)


def check_quad_symmetry(frames, w=W, h=H):
    """Check if frames exhibit 4-way dual-axis (H+V) mirror symmetry."""
    if frames is None or len(frames) == 0:
        return False, 1.0, 1.0

    # Test up to 12 evenly distributed sample frames
    step = max(1, len(frames) // 12)
    sample_indices = range(0, len(frames), step)
    hh, hw = h // 2, w // 2

    errs = []
    if HAVE_NUMPY:
        for idx in sample_indices:
            f = frames[idx].reshape((h, w)).astype(np.float32)
            tl = f[:hh, :hw]
            tr = f[:hh, hw:][:, ::-1]   # Flip H
            bl = f[hh:, :hw][::-1, :]   # Flip V
            br = f[hh:, hw:][::-1, ::-1] # Flip H+V
            err = max(np.mean(np.abs(tl - tr)), np.mean(np.abs(tl - bl)), np.mean(np.abs(tl - br))) / 255.0
            errs.append(err)
    else:
        for idx in sample_indices:
            f = frames[idx]
            grid = [[f[y * w + x] for x in range(w)] for y in range(h)]
            diff_sum = 0
            count = 0
            for y in range(hh):
                for x in range(hw):
                    tl = grid[y][x]
                    tr = grid[y][w - 1 - x]
                    bl = grid[h - 1 - y][x]
                    br = grid[h - 1 - y][w - 1 - x]
                    diff_sum += abs(tl - tr) + abs(tl - bl) + abs(tl - br)
                    count += 3
            errs.append((diff_sum / (count * 255.0)) if count > 0 else 1.0)

    avg_err = float(sum(errs) / len(errs))
    max_err = float(max(errs))
    # Strict conservative symmetry threshold (< 1.5% average error, < 3.5% max)
    is_symmetric = bool(avg_err < 0.015 and max_err < 0.035)
    return is_symmetric, round(avg_err * 100, 2), round(max_err * 100, 2)


def find_period(frames, fps, min_sec, max_sec, window=45, top=40):
    n = len(frames)
    lo = max(2, int(min_sec * fps))
    hi = min(n - window - 1, int(max_sec * fps))
    if hi <= lo:
        return None

    if HAVE_NUMPY:
        f = frames.astype(np.float32)
        step = np.mean(np.abs(f[1:] - f[:-1]), axis=1)
        typical = float(np.median(step)) + 1e-6
        d0 = np.mean(np.abs(f[lo:hi] - f[0]), axis=1)
        cand = [int(x) + lo for x in np.argsort(d0)[:top]]

        def win_err(p):
            return float(np.mean(np.abs(f[0:window] - f[p:p + window])))

        def seam_err(p):
            return float(np.mean(np.abs(f[0] - f[p - 1])))
    else:
        f = frames
        steps = sorted(_mad(f[i], f[i - 1]) for i in range(1, n, max(1, n // 400)))
        typical = steps[len(steps) // 2] + 1e-6
        d0 = [(_mad(f[t], f[0]), t) for t in range(lo, hi)]
        d0.sort()
        cand = [t for _, t in d0[:top]]

        def win_err(p):
            return sum(_mad(f[k], f[p + k]) for k in range(window)) / window

        def seam_err(p):
            return _mad(f[0], f[p - 1])

    results = []
    for p in cand:
        p = int(p)
        if p + window >= n:
            continue
        win = win_err(p)
        seam = seam_err(p)
        results.append({
            "frames": p,
            "seconds": round(p / fps, 3),
            "window_err": round(win, 3),
            # Judge the window against the clip's OWN motion, not an absolute
            # number. Two encodings of identical content differ by a few
            # percent from compression noise alone; what matters is whether
            # the mismatch is small compared with how much the picture
            # normally changes between consecutive frames.
            "window_ratio": round(win / typical, 3),
            "seam_ratio": round(seam / typical, 3),
        })

    if not results:
        return None

    # seam_ratio is the decisive test: it asks "would a viewer see the wrap?"
    # window_ratio guards against a single frame matching by chance.
    def is_confident(r):
        # A near-perfect window match IS the loop, even if the seam ratio looks
        # high: seam_ratio is divided by the clip's typical motion, so on very
        # slow, smooth material the denominator is tiny and the ratio inflates
        # while the actual wrap is still invisible. window_ratio 0.01 means the
        # next 45 frames are the same picture -- that is not a coincidence.
        return r["window_ratio"] < 2.5 and (r["seam_ratio"] < 1.5
                                            or r["window_ratio"] < 0.5)

    good = [r for r in results if is_confident(r)]
    if good:
        best = min(good, key=lambda r: r["frames"])   # fundamental, not a multiple
        best["confident"] = True
    else:
        best = min(results, key=lambda r: r["seam_ratio"])
        best["confident"] = False

    # LOCAL REFINEMENT. The coarse search can land a frame or two either side
    # of the true period, and a one-frame error is exactly the kind of small
    # jump that is visible on a wrap.
    p0 = best["frames"]
    cands = []
    for p in range(max(lo, p0 - 3), min(n - window - 1, p0 + 4)):
        cands.append((win_err(p), p))
    if cands:
        errw, pbest = min(cands)
        if pbest != p0:
            best["refined_from"] = p0
        best["frames"] = int(pbest)
        best["seconds"] = round(pbest / fps, 3)
        best["window_err"] = round(errw, 3)
        best["window_ratio"] = round(errw / typical, 3)
        best["seam_ratio"] = round(seam_err(pbest) / typical, 3)

    # RE-JUDGE AFTER REFINEMENT. The verdict was previously decided before the
    # local refinement changed the numbers, so a row could print SEAMLESS next
    # to a seam ratio that fails the test. A label that disagrees with the
    # figures beside it is worse than no label.
    best["confident"] = is_confident(best)

    best["typical_step"] = round(typical, 3)
    return best


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dir", default=None)
    ap.add_argument("--min", type=float, default=2.0, help="shortest loop to consider (s)")
    ap.add_argument("--max", type=float, default=120.0, help="longest loop to consider (s)")
    ap.add_argument("--out", default=None)
    ap.add_argument("--clean", action="store_true",
                    help="DELETE the junk sidecar files it finds (AppleDouble "
                         "._*, .DS_Store, Thumbs.db). Off by default: a scan "
                         "should not delete things on your source drive.")
    a = ap.parse_args()

    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
    src = a.dir or os.path.join(root, "media", "h264")
    out = a.out or os.path.join(root, "media", "loops.json")

    everything = sorted(os.listdir(src))

    # JUNK SIDECARS.
    # Copying from a Mac to a non-Mac volume leaves an AppleDouble "._name"
    # beside every real file: a few KB of resource fork, not video. They match
    # *.mp4, so they were scanned, failed to decode, and buried the real
    # results in 27 lines of noise. Skipping them is correctness; deleting
    # them needs to be asked for.
    def is_junk(n):
        return (n.startswith("._")
                or n in (".DS_Store", "Thumbs.db", "desktop.ini")
                or n.startswith(".Spotlight")
                or n == "__MACOSX")

    junk = [f for f in everything if is_junk(f)]
    files = sorted(f for f in everything
                   if f.lower().endswith(".mp4") and not is_junk(f))

    if junk:
        total_kb = 0
        for j in junk:
            try:
                total_kb += os.path.getsize(os.path.join(src, j)) // 1024
            except OSError:
                pass
        if a.clean:
            removed = 0
            for j in junk:
                try:
                    os.remove(os.path.join(src, j)); removed += 1
                except OSError as e:
                    print(f"   could not delete {j}: {e}")
            print(f"== deleted {removed} junk sidecar file(s), {total_kb} KB")
        else:
            print(f"== skipping {len(junk)} junk sidecar file(s) "
                  f"({total_kb} KB) -- AppleDouble/.DS_Store etc.")
            print(f"   pass --clean to delete them from {src}")
        print()

    if not files:
        print("no clips in " + src); return 1

    print(f"scanning {len(files)} clips in {src}")
    print("engine: numpy" if HAVE_NUMPY else
          "engine: pure Python (slower; 'python -m pip install numpy' speeds it up)")
    print(f"{'clip':<44}{'dur':>7}{'loop':>9}{'winErr':>8}{'seam':>7}  verdict")
    print("-" * 88)

    report = {}
    for name in files:
        path = os.path.join(src, name)
        fps = fps_of(path)
        # find_period deliberately searches no farther than half the decoded
        # material, so two search horizons plus the 45-frame verification
        # window are sufficient. Decoding an entire 10-minute stock master to
        # find a 5-second repeat wastes minutes and hundreds of MB per clip.
        scan_seconds = (2.0 * a.max) + (50.0 / max(fps, 1.0)) + 0.25
        frames = decode_gray(path, max_seconds=scan_seconds)
        if frames is None:
            try:
                kb = os.path.getsize(path) // 1024
            except OSError:
                kb = -1
            hint = " (tiny file - not video?)" if 0 <= kb < 64 else ""
            print(f"{name[:42]:<44}{'?':>7}  decode failed, {kb} KB{hint}")
            report[name] = {"duration": 0, "loop": None, "error": "decode failed"}
            continue
        dur = len(frames) / fps
        best = find_period(frames, fps, a.min, min(a.max, dur / 2), )
        is_sym, avg_sym_err, max_sym_err = check_quad_symmetry(frames)
        layout = "mirror_hv_tl" if is_sym else "direct"
        short = name[:42]
        if best is None:
            sym_tag = " [H+V SYM]" if is_sym else ""
            print(f"{short:<44}{dur:>6.1f}s{'-':>9}{'-':>8}{'-':>7}  no loop found{sym_tag}")
            report[name] = {"duration": round(dur, 2), "loop": None, "layout": layout,
                            "symmetry": {"symmetric": is_sym, "avg_err": avg_sym_err, "max_err": max_sym_err}}
            continue
        verdict = "SEAMLESS" if best["confident"] else "uncertain - check"
        sym_tag = " [H+V SYM]" if is_sym else ""
        print(f"{short:<44}{dur:>6.1f}s{best['seconds']:>8.2f}s"
              f"{best['window_ratio']:>8.2f}{best['seam_ratio']:>7.2f}  {verdict}{sym_tag}")
        report[name] = {"duration": round(dur, 2), "fps": round(fps, 3),
                        "loop": best, "layout": layout,
                        "symmetry": {"symmetric": is_sym, "avg_err": avg_sym_err, "max_err": max_sym_err}}

    with open(out, "w") as fh:
        json.dump(report, fh, indent=1)
    ok = sum(1 for v in report.values() if v.get("loop", {}) and v["loop"].get("confident"))
    sym_total = sum(1 for v in report.values() if v.get("symmetry", {}).get("symmetric"))
    print("-" * 88)
    print(f"{ok}/{len(files)} confident loop points, {sym_total}/{len(files)} 4-way H+V symmetric -> {out}")
    print("winErr = 45-frame window mismatch as a MULTIPLE of the clip's own")
    print("         frame-to-frame motion (<2.5 is a match)")
    print("seam   = wrap discontinuity vs a normal frame step.")
    print("         1.0 = the wrap looks like any other frame. <1.5 is seamless.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
