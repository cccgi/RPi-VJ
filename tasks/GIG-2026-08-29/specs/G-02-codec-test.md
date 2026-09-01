# G-02 — Codec Decision Test
**Model:** cheap · **Blocks:** G-06, G-16 · **Do this Tuesday.**

## Why
The Pi 5's BCM2712 has **no H.264 hardware decode** — HEVC only. But whether
Chromium on Pi OS actually exposes HEVC decode to `<video>` is unverified.
Frank's clip library is H.264/ProRes and must be transcoded to *something*
before Saturday. This task decides to what. Guessing costs the gig.

## Deliverable
`gig/probe/codec.html` — one file, no dependencies.

## Requirements
1. Print the result of `MediaSource.isTypeSupported()` and
   `document.createElement('video').canPlayType()` for each of:
   - `video/mp4; codecs="avc1.640028"` (H.264 High L4.0)
   - `video/mp4; codecs="hvc1.1.6.L120.90"` (HEVC Main)
   - `video/mp4; codecs="hev1.1.6.L120.90"`
   - `video/webm; codecs="vp9"`
2. Accept a file via `<input type="file" multiple>` (no server needed), play
   each clip looped into a `<video>`, and for 10 seconds report:
   - `video.getVideoPlaybackQuality()` → `totalVideoFrames`, `droppedVideoFrames`
   - measured playback fps
   - a **rendered-through-WebGL** check: upload the video to a texture with
     `texImage2D` every frame and draw it, since that is how G-06 will use it.
     Report fps for that path separately — decode that works in a `<video>`
     tag can still stall when uploaded to a texture.
3. Print CPU load hint: run alongside `top -bn1 | head -5` from SSH and paste it.

## Test material (create on the PC first, `gig/tools/`)
From one representative 1080p clip, produce five encodes with ffmpeg:
```
1080p60 HEVC   : -c:v libx265 -preset medium -crf 20 -tag:v hvc1 -g 15 -keyint_min 15
1080p30 HEVC   : same, -r 30
1080p60 H.264  : -c:v libx264 -preset medium -crf 20 -g 15 -keyint_min 15
 720p60 H.264  : same, -vf scale=1280:720
 720p30 H.264  : same, -r 30
```
Closed GOP of 15 matters for clean loop points — keep `-g 15 -keyint_min 15`
and add `-sc_threshold 0`.

## Observable
A table in `../RESULTS.md`: for each of the five encodes, dropped frames out of
total, fps in `<video>`, fps through the WebGL texture path, and Pi CPU %.

Then a one-line **DECISION** naming the format the clip library will be
transcoded to for Saturday, with the number that justifies it.

## Bias
When two options are close, pick the one with the lower CPU load, not the
higher resolution. GPU headroom is for shaders; the TV is 100" but the audience
is not measuring pixels. 720p upscaled with good post-processing beats 1080p
that stutters on a kick.
