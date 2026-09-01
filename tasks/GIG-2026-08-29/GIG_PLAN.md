# GIG PLAN — Saturday 29 August 2026
**Target:** Raspberry Pi 5, headless, micro-HDMI → 100" TV. Booth line-in from
Pioneer DDJ-FLX10 via Sound Blaster Play! 3. Stream Deck XL for live control.
**Owner:** Frank · **Days available:** Tue 25 → Fri 28 (4 working days)

## The decision: browser engine, not Rust engine

The Rust workspace has never rendered a pixel or opened a device
(`../ENGINE_REALITY_CHECK.md`). Making it real means writing wgpu init, a
GStreamer pipeline, a hidapi loop and a PipeWire client, then debugging all four
on unfamiliar hardware. That is weeks. It is not available by Saturday.

**Everything the gig needs already exists as a shipped, hardware-accelerated,
Pi-supported browser API:**

| Need | Browser API | Pi 5 status |
|---|---|---|
| Audio-reactive shaders at 1080p | WebGL2 on Mesa/V3D | GLES 3.1 conformant — WebGL2 is a subset |
| Booth audio in | Web Audio `getUserMedia` + `AnalyserNode` | USB audio via ALSA, standard |
| Video clip playback | `<video>` | see codec risk below |
| Clips + shaders composited | `texImage2D(video)` into the shader | standard, cheap |
| Stream Deck XL | **WebHID** | Chromium supports it; deck is a plain HID device |
| Fullscreen HDMI out | Chromium `--kiosk` | standard |

So the gig rig is one Chromium window. No Rust, no drivers, no root. This also
means every piece is debuggable from a laptop browser before it ever meets the Pi.

**This is not throwing work away.** The Rust model layer is the spec for the
eventual engine, and the browser build validates every assumption in it against
real hardware for the first time — which is exactly what Phase 4 needed anyway.

## Where the "too basic" problem actually gets solved

The visuals feel thin compared to VS 2 / Arena, and the instinct is "write
fancier shaders." That is the expensive lever and the weakest one on a
VideoCore VII. What actually separates a VS2 look from a hobby shader:

1. **Feedback / trails buffer** — last frame re-sampled with a warp and decay.
   Single biggest perceived-richness win per GPU cycle. One extra FBO.
2. **Bloom** — threshold + separable blur + add. Makes everything look expensive.
3. **Chromatic aberration + grain** — cheap, sells "cinematic".
4. **Choreography** — uniforms modulated on bar/beat/phrase so scenes *develop*
   instead of looping. Repetitiveness is a modulation problem, not a shader
   problem. This is where "repetitive and too basic" is really coming from.
5. Only then: more shaders.

G-12 and G-13 carry more of the visual outcome than G-14 does. Budget accordingly.

## Known risks, ranked

| # | Risk | Mitigation | Decided by |
|---|---|---|---|
| R1 | VideoCore VII too slow for raymarching at 1080p | Render at internal scale (0.5–0.75) and upscale. On a 100" TV at 3m this is invisible. Adaptive governor drops scale before it drops frames. | G-01 |
| R2 | **Clip codec.** Pi 5 has *no* H.264 hardware decode (HEVC only). Chromium's HEVC support on Pi is not guaranteed. | Test both paths on real hardware Tuesday. Fallback: 720p H.264 software decode — 4×A76 @2.4GHz handles it — upscaled. | G-02 |
| R3 | WebHID can't see the Stream Deck without root | udev rule granting the `plugdev` group access to the hidraw node. Same rule the old `deploy/pi/` files already carry. | G-08/G-09 |
| R4 | USB audio device disappears or picks the wrong input | Pin by device name, add a re-acquire button, and keep the internal 128 BPM clock as fallback. | G-05 |
| R5 | Thermals — sustained GPU load in a hot booth | Active cooler mandatory. Log `vcgencmd measure_temp` during rehearsal. | G-17 |

## Non-negotiable schedule

| Day | Gate |
|---|---|
| **Tue 25** | G-01 + G-02 done. **We know the real fps and the real codec.** Every later task's budget depends on these two numbers. Do not write renderer code before they land. |
| **Wed 26** | Renderer path end-to-end: one shader, real audio, on the TV. |
| **Thu 27** | Stream Deck control + video decks + compositing. Feature freeze 22:00. |
| **Fri 28** | Post-chain, choreography, content pass, kiosk boot, full dress rehearsal with the actual DJ setup. |
| **Sat 29** | Gig. `G-17` failure playbook printed and taped to the case. |

**Fri is rehearsal, not development.** If something isn't working Thursday
night, it gets cut, not fixed. The set must survive any single component
failing — shaders alone with no clips is a good show; a black screen is not.

## Definition of done for Saturday
Pi boots with no keyboard → Chromium fullscreen on the TV → Stream Deck lights
up → pressing a key changes the visual → booth audio drives the visuals → and
a blackout key always works.
