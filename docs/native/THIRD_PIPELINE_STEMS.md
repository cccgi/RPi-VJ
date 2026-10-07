# Third content pipeline: pre-rendered stems, mixed live — exploration, 2026-10-07

Status: workstation half prototyped and verified; Pi half designed, not built,
not measured.

## The constraint that shapes it

three.js, GSAP and HyperFrames are browser technology. The appliance has no
browser and will not get one. So these tools are used only on the workstation,
as an offline renderer, and the Pi gets files it can already play. A plain
pre-rendered loop would just be another clip with no audio reaction. The
pipeline below keeps the reaction live on the Pi while moving all the visual
complexity offline.

## Idea

Render a scene as separate **stems** — layers that each belong to one part of
the music — pack them into a single HEVC loop, and let a tiny native shader mix
them with the live audio uniforms. The Pi's per-pixel work is a handful of
texture fetches regardless of how expensive the scene was to render, so the
frame cost is fixed and small while the look is unconstrained.

| Stage | Where | Tool |
|---|---|---|
| Author scene, deterministic in time, seamless loop | workstation | three.js / GSAP / HTML in a HyperFrames composition |
| Render 1080p60 frames | workstation | `hyperframes render` (headless Chrome, GPU) |
| Encode to the appliance clip profile | workstation | ffmpeg libx265, Main 8-bit 4:2:0, GOP 30, no B-frames |
| Decode | Pi | existing libmpv deck (hardware HEVC) |
| Mix stems with audio | Pi | small GLSL stem shader sampling the deck texture |

## Stem packings

1. **Quad atlas** (prototyped): four 960x540 colour stems in one 1080p frame.
   One decoder, full colour per stem, each stem at quarter resolution.
2. **Channel pack**: three full-resolution greyscale mattes in R, G, B; the
   shader supplies colour. Sharper, but 4:2:0 chroma subsampling softens two of
   the three channels, so it needs a test encode before relying on it.
3. **Flipbook atlas**: a grid of frames in one still texture; audio picks the
   frame. For short reactive elements (sprites, logo stings) with no decoder.
4. **Still art plus depth**: one generated image and a depth map; the shader
   does audio-driven parallax, glow and displacement. Cheapest of all, and the
   natural home for generated concept art.

Packings 3 and 4 need static textures in the shader pass, which is the same
engine feature the Shadertoy texture shaders need.

## What was built and checked

- `studio/stem-demo/index.html` — one composition, four stems: a three.js
  tunnel (base), nested wireframe solids (bass), GSAP-driven lettering and
  sweep bars (mid), a seeded particle field (high). All motion is periodic over
  8 s.
- Render: 480 frames, 1920x1080 at 60 fps, 31 s wall time on this workstation.
- Encode: `target/studio/stem-demo/stems.mp4`, HEVC Main, yuv420p, 60 fps,
  480 frames, 23.7 MB (CRF 20; thin bright lines on black are costly to encode
  and the size can come down).
- Loop seam: last-to-first frame difference 5.76 against a normal
  frame-to-frame difference of 5.57 (mean absolute, 0-255), so the wrap is not
  visible as a jump.
- `studio/stem-demo/stems_quad.fs` — the Pi-side mixer shader, written against
  the engine's existing audio uniforms. It has not been compiled or run.

## What the engine needs

| Change | Size | Shared with |
|---|---|---|
| Bind the active deck texture as `u_video` during the shader pass | small, in `render.rs` | — |
| Static texture channels for shaders (2D, repeat/clamp, mipmaps) | medium | Shadertoy `needs_texture` class |
| Show-manifest entry type pairing a clip with its stem shader, one Stream Deck key | small | — |

None of this is started: `render.rs` is being changed by the benchmark work
and these go in after it is reviewed.

## Open questions to settle by measurement

- Cost of the stem shader on the Pi at 1080p with hardware HEVC decode running
  (expected to be small; unmeasured).
- Whether quadrant edges bleed after HEVC compression (the shader insets by
  0.4 %).
- Quad atlas versus channel pack for sharpness on a club projector.
- A stem clip occupies one of the two decoders, so a stem scene and an
  ordinary clip can run together but two stem scenes cannot crossfade.

## Generated art

The owner's local ComfyUI (`D:\_krea2`, RTX 3090) is the generator, driven
over its HTTP API. Verified: a 1024x1024 Flux.2 klein texture in about 32 s
(`D:\_krea2\output\rpi-vj\proof_chrome_00001_.png`). Krea2, Ideogram 4, Qwen
Image and MiniMax H3 are installed but not yet exercised from here. Generated
stills feed packing 4 directly, generated sprites feed packing 3, and tileable
textures can also serve native shaders once texture channels exist. The stem
prototype itself is still fully procedural.

## Side effect to know about

`hyperframes init` installed skill folders into `~/.claude/skills`
(`hyperframes-registry`, `hyperframes-studio`, `media-use` and related) and
wrote `CLAUDE.md`/`AGENTS.md` into the project folder, without asking.
