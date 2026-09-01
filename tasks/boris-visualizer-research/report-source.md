# Boris-style real-time visualizer research source

Audience: RPi-VJ implementers. Date: 2026-08-28.

## Direct answer

Yes. Build an original, functionally similar real-time visualizer for the Workstation tier (Windows/i-series/RTX 3090) first. The present audio analyzer already provides five normalized bands, a 32-bin spectrum, hit impulses, beat/bar phase, BPM, and flowing clocks; the v2 ABI expands that to 128/256-bin FFT and waveform textures. The Workstation renderer permits eight ISF passes and eight visible layers. This is enough for spectrum/waveform primitives, polar warping, persistence, glow, transforms, and compositing.

Do not attempt Boris's tracking/ML/editor-host features. Do not copy its presets or art. Reproduce the broad visual grammar with original shaders and preset data.

## Supplied reference

`Boris Visualizer.MP4`: H.264/yuv420p, 720x1280, 25 fps, 28.99 seconds, AAC stereo 48 kHz, 23.82 MB. Contact-sheet inspection shows spectrum surfaces, mirrored wire geometry, radial rings/bars, dense particle/point fields, neon glow, kaleidoscopic forms, camera motion, text, and composited backgrounds.

## Official product behavior

BCC+ Audio Visualizer supports frequency or time-domain data; bars, dots and peak dots; resolution, scale, density and rounded shapes; taper; peak hold/falloff; waveform duration/offset; band gains and min/max frequency; neighboring-band smoothing; three-color gradients; pre/global 3D transforms; mirror and polar warp/swirl; crop/corner pin; motion blur; glow; color correction; and background composition. It can render on the GPU in 16-bit or 32-bit float. Source: https://borisfx.com/documentation/continuum/bcc-audio-visualizer/

## Project mapping

Current prototype: `gig/src/audio.js`, `gig/src/mixer.js`, `gig/src/isf.js`, existing spectrum/ring/oscilloscope shaders. Production design: `docs/v2/02-TARGET-ARCHITECTURE.md`, Workstation RTX 3090 tier, wgpu/Vulkan, audio feature snapshot plus `u_fft` and `u_waveform`, eight-pass limit.

## Recommended architecture

Audio snapshot -> visualizer state -> primitive mask pass -> optional polar/transform pass -> optional feedback/particle pass -> separable bloom -> composite. Use pooled RGBA16F targets. Render geometry analytically in fragment shaders for bars/dots/rings; use an instanced or compute-driven particle backend only for dense 3D point fields. Maintain peak hold and history in CPU DSP/state or bounded persistent textures. Use internal scale 0.5 for blur/particles and full scale only for crisp primitive/composite passes.

## MVP

One shader family with Cartesian and polar layouts, bar/dot/line styles, 128-bin FFT, waveform texture, band tuning, peak hold, mirror, 3-color gradient, transform, feedback trails, thresholded bloom, and original presets. Add point-cloud/particles and 3D camera in phase 2.

## Constraints

The reference promo is not evidence that every shot is produced by the documented Audio Visualizer filter alone. Performance is not asserted until measured on the actual 3090. The current browser prototype has only 32 spectrum bins, so a faithful detailed spectrum needs the planned 128/256-bin ABI or a stopgap 64-bin path.

## Claim ledger

- Product behavior and controls: “BCC+ Audio Visualizer,” Boris FX, accessed 2026-08-28, https://borisfx.com/documentation/continuum/bcc-audio-visualizer/
- Project hardware tiers and shader ABI: repository `docs/v2/00-OVERVIEW.md` and `docs/v2/02-TARGET-ARCHITECTURE.md`, accessed 2026-08-28.
- Supplied-video properties: local ffprobe inspection on 2026-08-28.

