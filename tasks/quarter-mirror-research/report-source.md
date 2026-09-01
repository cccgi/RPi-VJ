# Quarter-mirror clip research source

## Decision

Pursue this as a metadata-driven asset optimization, beginning with quad-symmetric clips. It is worthwhile for decoder, decoded-frame, upload, texture-memory, storage, and I/O costs, but it is not a substitute for fixing Chromium/WebGL leaks or context loss. The final compositor still shades every 1920x1080 output pixel.

## Repository findings

- `gig/tools/add-clips.ps1` invokes `find-loops.py`, `loop-cut.py`, and `build-clip-bank.py`.
- `loop-cut.py` currently scales to the target height and encodes H.264/yuv420p.
- `gig/src/mixer.js` already contains creative mirror effects 9-11, but those sample a full-size texture and must remain distinct from asset reconstruction.
- Video frames are uploaded with `texImage2D`; smaller stored video dimensions therefore reduce uploaded source pixels and texture dimensions.

## Corpus measurement

Twenty-seven clips in `gig/media/neon` were sampled over their first eight seconds at 1 fps and 320-pixel analysis width. Opposing halves were flipped and compared with FFmpeg SSIM.

- Seven clips passed both horizontal and vertical luma mean SSIM >= 0.98 (25.9%).
- Four additional clips strongly passed horizontal symmetry (14.8%).
- Thus 25.9% are immediate quad candidates and about 40.7% expose at least one useful symmetry axis.
- A 160-pixel analysis width produced false negatives on known candidates; use at least 320 pixels and verify candidates at 640 pixels or higher.

These are screening figures, not a production acceptance test. Mean SSIM can hide short asymmetric overlays or bursts, so the detector must evaluate per-frame distribution across the selected seamless loop.

## Cost model

- 1920x1080 YUV420 frame: about 3.11 MB; 960x540: about 0.78 MB.
- 1920x1080 RGB texture: about 5.93 MiB; 960x540: about 1.48 MiB.
- Quad storage therefore processes one quarter of the source pixels during decode and upload. Actual file-size reduction will be less predictable because H.264 already exploits spatial redundancy.
- Reconstruction is a small UV transform in the fragment shader. It does not shrink the fullscreen framebuffer, post-processing, or compositor pass.

## Authoritative references

- FFmpeg filter documentation: crop, hflip, vflip, SSIM, and mirrored-half examples: https://ffmpeg.org/ffmpeg-filters.html
- WebGL 1 specification: HTMLVideoElement is a valid texImage2D/texSubImage2D source: https://registry.khronos.org/webgl/specs/latest/1.0/
- Khronos/Chromium conformance test for video texImage2D and preallocated texSubImage2D: https://chromium.googlesource.com/external/khronosgroup/webgl/+/HEAD/conformance-suites/1.0.0/conformance/tex-image-and-sub-image-2d-with-video.html
- Historical Chromium implementation note on video upload conversion/copy cost: https://chromium.googlesource.com/chromium/src.git/+/7eac8fd70480589ca7594d5009537b3f7b4f53e6%5E%21/

