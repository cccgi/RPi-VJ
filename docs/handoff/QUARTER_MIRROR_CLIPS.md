# Quarter-mirror clip optimization: investigation and implementation handoff

## Recommendation

Implement this, starting with **quad-symmetric clips only**, behind explicit clip metadata and a conservative detector. The current Neon bank contains enough eligible material to justify it: 7 of 27 sampled clips (25.9%) were strong quad candidates. Four more (14.8%) were horizontally symmetric, but half-frame modes should be a second phase because they require logical-aspect handling.

This is a useful resource optimization, not the primary Chromium-crash fix. A quarter-size source reduces compressed data, decoder work, decoded-frame memory, video-to-WebGL upload pixels, and the deck texture. The compositor still renders a 1920x1080 frame, so its fragment shader, framebuffer, blending, and post-processing costs remain essentially unchanged.

## What was inspected

The ingest path is:

```text
add-clips.ps1
  -> find-loops.py
  -> loop-cut.py
  -> build-clip-bank.py
  -> sync
```

`loop-cut.py` currently scales to the requested height and re-encodes H.264/yuv420p. The runtime uploads each decoded video frame to a WebGL texture in `Deck._uploadTexture()`.

The mixer already has horizontal, vertical, and quad mirror **creative effects**. They operate on a full-frame source and should not be reused as an asset-storage flag: reconstruction of a stored top-left quarter uses a different UV mapping, and creative effects must remain independently selectable.

FFmpeg officially supplies the required `crop`, `hflip`, `vflip`, and `ssim` filters, including a documented mirrored-half construction example. WebGL explicitly permits an `HTMLVideoElement` as a texture-upload source, so smaller decoded video dimensions directly reduce source upload dimensions. [FFmpeg filter documentation](https://ffmpeg.org/ffmpeg-filters.html), [WebGL 1 specification](https://registry.khronos.org/webgl/specs/latest/1.0/)

## Measured eligibility in the current library

I screened all 27 files under `gig/media/neon`. Frames from the first eight seconds were sampled at 1 fps and 320-pixel analysis width. For horizontal symmetry, the left half was compared with a flipped right half; for vertical symmetry, the top half was compared with a flipped bottom half using FFmpeg SSIM.

Strong quad candidates (horizontal and vertical mean luma SSIM at least 0.98):

| Clip prefix | H SSIM | V SSIM |
|---|---:|---:|
| 186371-877727709 | 0.9960 | 0.9952 |
| 236829 | 0.9952 | 0.9951 |
| 236830 | 0.9942 | 0.9940 |
| 236831 | 0.9947 | 0.9948 |
| 236832 | 0.9959 | 0.9957 |
| 63885-508273140 | 0.9932 | 0.9930 |
| 65495-514501835 | 0.9897 | 0.9880 |

Four additional clips were strong horizontal-only candidates: `26041-355469036`, `26475-360248610`, `27438-363642440`, and `27439-363642443`.

Important caveats:

- These measurements establish feasibility, not final classifications. The production scan must cover the exact detected loop interval.
- Mean SSIM alone can hide a short logo, asymmetric flash, or transition. Record per-frame values and test a low percentile as well as the mean.
- A 160-pixel analysis width unexpectedly caused false negatives for some obvious candidates. Use 320 pixels minimum and verify candidates at 640 pixels or native resolution.

## Expected resource effect

For a 1080p quad clip, the stored video becomes 960x540:

| Resource | Full 1920x1080 | Quarter 960x540 | Theoretical change |
|---|---:|---:|---:|
| YUV420 decoded frame | ~3.11 MB | ~0.78 MB | 75% less |
| RGB deck texture | ~5.93 MiB | ~1.48 MiB | 75% less |
| Source pixels decoded/uploaded | 2,073,600 | 518,400 | 75% less |

Decoder queues contain multiple frames, so real memory savings can exceed one-frame figures. File size will not reliably fall by 75% because H.264 already compresses repeated spatial structure; measure it rather than promising a fixed ratio.

The reconstruction shader is only an absolute-value UV fold in a fullscreen pass that already exists. Its incremental GPU cost should be tiny. Conversely, the output framebuffer and every downstream compositor/post-FX pass remain 1080p.

Chromium has historically performed color conversion and a GPU copy while uploading video frames to WebGL, making source resolution relevant, although the exact current Raspberry Pi path must be profiled rather than inferred from old Chromium internals. [Chromium video-upload implementation note (historical)](https://chromium.googlesource.com/chromium/src.git/+/7eac8fd70480589ca7594d5009537b3f7b4f53e6%5E%21/)

## Proposed ingest design

Add `find-symmetry.py` after seamless-loop detection and before the final encode. Analyze the selected loop interval, not the original file's arbitrary opening seconds.

Classifications:

```text
full             complete encoded frame
mirror_h_left    stored left half, horizontally reconstructed
mirror_v_top     stored top half, vertically reconstructed
mirror_hv_tl     stored top-left quarter, reconstructed on both axes
```

Phase 1 should emit only `full` and `mirror_hv_tl`. The other measurements may be recorded for later use.

Suggested initial detector:

1. Normalize analysis frames to an even 320-pixel width and sample the complete selected loop at 2 fps.
2. Compare left against flipped right and top against flipped bottom with luma SSIM.
3. Treat an axis as automatic only when mean SSIM >= 0.990, fifth-percentile SSIM >= 0.985, and minimum SSIM >= 0.970.
4. Verify automatic candidates again at 640-pixel width. Put borderline candidates in a review report; never crop them automatically.
5. Support explicit `off`, `force-full`, and `force-quad-tl` overrides.

Those thresholds are conservative starting values and must be tuned against reviewed false positives/negatives. FFmpeg's SSIM filter can write per-frame statistics with `stats_file`, which is essential here. [FFmpeg SSIM documentation](https://ffmpeg.org/ffmpeg-filters.html#ssim)

After scaling the logical full frame to 1920x1080, a quad candidate can be cropped with:

```text
scale=-2:1080:flags=lanczos,crop=iw/2:ih/2:0:0
```

The resulting encoded asset is 960x540. Ensure both full dimensions are divisible by four before halving so yuv420p chroma dimensions remain valid. Preserve the existing H.264 profile/pixel-format policy.

Add the result to the clip-bank entry rather than inferring it from a filename:

```json
{
  "type": "clip",
  "src": "media/neon/example.mp4",
  "layout": "mirror_hv_tl",
  "logicalWidth": 1920,
  "logicalHeight": 1080,
  "symmetry": {
    "hMean": 0.9952,
    "hP05": 0.9938,
    "vMean": 0.9949,
    "vP05": 0.9931,
    "analysisFps": 2,
    "verificationWidth": 640
  }
}
```

Keeping the classification and evidence in metadata makes the transformation auditable and reversible.

## Proposed playback design

Add a deck asset-layout enum/uniform, distinct from the existing FX enum:

```glsl
vec2 storedAssetUV(vec2 uv, int layout) {
    if (layout == 1) { // mirror_h_left
        return vec2(1.0 - abs(2.0 * uv.x - 1.0), uv.y);
    }
    if (layout == 2) { // mirror_v_top
        return vec2(uv.x, 1.0 - abs(2.0 * uv.y - 1.0));
    }
    if (layout == 3) { // mirror_hv_tl
        return 1.0 - abs(2.0 * uv - 1.0);
    }
    return uv;
}
```

Apply creative transforms in logical fullscreen coordinates, then apply `storedAssetUV` immediately before sampling the video texture. This keeps asset reconstruction transparent to existing effects.

For quad assets, the encoded and logical aspect ratios are both 16:9, so the current aspect behavior remains valid. Half-width and half-height assets do **not** retain the logical aspect ratio; that is why they should wait for phase 2 and use `logicalWidth`/`logicalHeight` rather than `videoWidth`/`videoHeight` for fit calculations.

Also benchmark replacing per-frame `texImage2D(video)` with a one-time texture allocation followed by `texSubImage2D(video)`. Khronos' Chromium conformance suite tests both video paths. This is separate from quarter storage and should have its own A/B result. [Khronos WebGL video texture conformance test](https://chromium.googlesource.com/external/khronosgroup/webgl/+/HEAD/conformance-suites/1.0.0/conformance/tex-image-and-sub-image-2d-with-video.html)

## Gig-readiness acceptance test

Do not bulk-convert the library until this passes on the Pi 5:

1. Produce full and quarter variants from the same seven known candidates.
2. Run identical 30-minute alternating sequences, then a 2-hour mixed-bank soak.
3. Record browser renderer/GPU-process PSS or `smaps_rollup`, CPU per process, thermal/throttle state, dropped/rendered frames, context-loss events, switch latency, and restarts.
4. Include simultaneous two-deck crossfades, shaders, and audio analysis; idle testing alone is insufficient.
5. Visually inspect center seams and all frames containing text, logos, particles, or asymmetric flashes.
6. Ship only if quarter mode has zero material visual differences and shows repeatable memory, CPU, upload, or frame-time improvement.

Keep the originals or make the operation reproducible from originals. On a crash-prone production browser, this optimization should accompany—not replace—bounded texture/program lifetimes, context-loss recovery, process supervision, and soak testing.

## Implementation order

1. Add analysis-only symmetry reporting to `add-clips.ps1`; make no encoding changes.
2. Review the detected candidates and tune thresholds.
3. Add `layout`, logical dimensions, and detector evidence to the clip-bank schema.
4. Implement quad reconstruction in the mixer and unit-test UV coordinates at corners, edges, and center.
5. Encode duplicate quarter variants and perform the Pi A/B test.
6. Enable automatic quad output only after the soak-test gate passes.
7. Consider horizontal/vertical half modes later if their extra eligible clips justify the aspect-ratio complexity.

