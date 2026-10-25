# Wave 3 handoff — paste these into Antigravity

Repo: `E:\__Projects\RPi-VJ`
Every worker reads **two files only**: `tasks/GIG-2026-08-29/START_HERE.md` and its
own spec. Plus `gig/probe/PERF.md` where a prompt says so — that one holds the
measured hardware numbers and is short.

Order: **G-04b must finish before G-07 starts.** G-06 runs in parallel with G-04b.
G-12 and G-13 follow G-07.

---
## 1. G-04b — verify isf.js against the real shader convention  (cheap, BLOCKING)

```
Repo: E:\__Projects\RPi-VJ

Read exactly three files:
  tasks/GIG-2026-08-29/START_HERE.md
  tasks/GIG-2026-08-29/specs/G-04-isf-lite.md
  gig/probe/PERF.md

gig/src/isf.js already exists. It was written against a spec that was WRONG about
this project's shader convention. The spec has since been corrected. Your job is
to make isf.js match reality, which was verified on the Raspberry Pi on 2026-08-25.

The 13 shaders in gig/shaders/ do NOT use stock ISF names. They:
  - declare their own uniforms: u_time, u_resolution, u_bass, u_mid, u_treble,
    u_onset, u_volume, u_beat_phase
  - write to gl_FragColor (GLSL ES 1.00 style)
  - carry a /*{ ... }*/ JSON header whose INPUTS entries (e.g. flow_speed) are
    used in the body but NEVER declared there — the loader must generate those
    uniform declarations, typed from the header, initialised to DEFAULT
There is no TIME, RENDERSIZE or isf_FragNormCoord usage anywhere.

Critical: if you emit isf_FragNormCoord it MUST be a #define, never a global
variable initialiser. A global initialiser fails on every shader with
"'=' : global variable initializers must be constant expression". This was
observed on the Pi. Do not change it back.

A known-good reference implementation of exactly this loading is the build()
function inside gig/probe/shader-bench.html, which compiled all 13 shaders
successfully on the Pi. Read that function and match its behaviour.

Emit '#line 1' after your generated preamble so compile errors report line
numbers in the author's file, not yours.

Deliverable: gig/src/isf.js exporting parseISF(source) as the spec describes,
plus gig/probe/isf-test.html that loads all 13 shaders from gig/shaders/,
compiles each, and prints a pass/fail table.

Observable: all 13 compile. Run it on the Pi via
  ./gig/deploy/run-probe.sh isf-test.html
and read ~/vj-results/isftest.log. Report the pass/fail table.

Then: set G-04 in BOARD.md, append <=10 lines to RESULTS.md per its format.
If the spec and the shaders disagree, the SHADERS win — say so under `surprise:`.
```

---
## 2. G-06 — dual video deck  (cheap, parallel with G-04b)

```
Repo: E:\__Projects\RPi-VJ

Read exactly two files:
  tasks/GIG-2026-08-29/START_HERE.md
  tasks/GIG-2026-08-29/specs/G-06-video-deck.md

Implement gig/src/deck.js exactly as specified. The spec is self-contained.

Note: the spec lists G-02 (codec decision) as a blocker. It is NOT a blocker for
this code — <video> plays whatever the browser supports, so deck.js is
codec-agnostic. G-02 only decides what the clip LIBRARY gets transcoded to
(that is G-16). Build deck.js now.

Two performance points that matter on this hardware (Pi 5, V3D):
  - upload with RGB, not RGBA
  - upload ONLY when the frame is new (requestVideoFrameCallback where
    available, else track currentTime). Re-uploading an unchanged frame at 60Hz
    is the most common cause of a slow video path.
  - never call generateMipmap per frame

Deliverable: gig/src/deck.js + a test page gig/probe/deck-test.html that loads
two clips and draws both.

Observable: on the Pi, report fps with 0, 1 and 2 decks running. That delta is
the video budget and G-07 depends on it. If you have no clips to test with,
generate one with ffmpeg on the PC (testsrc, 1080p, 10s, h264) and say so.

Then: set G-06 in BOARD.md, append <=10 lines to RESULTS.md.
```

---
## 3. G-07 — the compositor  (mid tier, ONE agent, critical path)

```
Repo: E:\__Projects\RPi-VJ

Read exactly three files:
  tasks/GIG-2026-08-29/START_HERE.md
  tasks/GIG-2026-08-29/specs/G-07-mixer.md
  gig/probe/PERF.md

This is the task where gl.js, isf.js, audio.js and deck.js become a VJ engine.
Do not split it across parallel agents.

Build gig/src/mixer.js and gig/index.html exactly as the spec describes.
gig/index.html currently holds a placeholder landing page — replace it.

Measured hardware constraints (already verified on the target Pi — do not
re-measure, do not design around guesses):
  - frame budget 16.7ms at 1080p60
  - Tier A shaders cost 2.1-4.2ms, Tier B 5.6-8.4ms (see PERF.md)
  - vs_cyber_wormhole is the heaviest usable shader at 8.4ms
  - float FBOs exist but CANNOT be linearly filtered
  - 9 shaders are usable; 4 are cut or pending optimisation

The adaptive governor in the spec is mandatory, not optional. On this hardware
the margin is real but not generous.

The blackout path must work even when a shader fails to compile or a deck has
stalled. Test it that way deliberately.

Observable: on the Pi, on the TV — a video clip playing with an audio-reactive
shader composited over it in add mode, driven by live audio if the Sound Blaster
is attached (internal clock if not), holding >=55fps with the governor active.
Record the settled render scale and fps in RESULTS.md.

Then: set G-07 in BOARD.md. When this is DONE the show exists and everything
after it is polish.
```

---
## 4. G-12 — post chain  (mid, after G-07) — highest visual value

```
Repo: E:\__Projects\RPi-VJ

Read exactly three files:
  tasks/GIG-2026-08-29/START_HERE.md
  tasks/GIG-2026-08-29/specs/G-12-post-chain.md
  gig/probe/PERF.md

Build gig/src/post.js. Implement the stages IN THE ORDER the spec gives:
feedback/trails first (biggest visual win per GPU cycle), then bloom, then the
combined finish pass.

Two measured constraints, already verified on the Pi:
  - OES_texture_float_linear is NOT available. Use RGBA8 throughout the bloom
    chain. Do not attempt RGBA16F. This is not negotiable and not a preference.
  - the entire post chain must fit in about 5ms at 1080p, so that a Tier B
    shader (8.4ms) plus a video deck still fits inside 16.7ms.

Measure and report the cost of EACH stage separately. If the chain exceeds 5ms,
say so rather than shipping it — the governor drops bloom before render scale,
so a too-expensive bloom just means it is never on.

Observable: on the Pi, on the TV — the same shader with post off vs post full,
plus per-stage ms. Then judge it by eye on the 100" screen. If 'full' is not
dramatically richer than 'off', the parameters are wrong; say so in RESULTS.md.
```

---
## 5. G-13 — choreography  (mid, after G-07)

```
Repo: E:\__Projects\RPi-VJ

Read exactly two files:
  tasks/GIG-2026-08-29/START_HERE.md
  tasks/GIG-2026-08-29/specs/G-13-choreography.md

Build gig/src/choreo.js as specified.

The single most important design point: LFO rates are expressed in BARS and
driven from barPhase, never in wall-clock Hz. That is what makes movement land
on the music for free.

Tap tempo must always outrank auto-BPM for at least 30 seconds after a tap. At
1am a DJ trusts the tap they just made over an algorithm.

Observable: on the Pi with real music playing — tapped BPM tracks within 2 BPM
and holds; an 8-bar LFO visibly completes its cycle on the downbeat. Record
measured vs actual BPM for three tracks in RESULTS.md.
```

---
## Still Frank's to run, not a subagent's
- **G-02 codec test** — needs test clips made on the PC first with
  `gig/tools/make-test-clips.sh`, then run on the Pi. Gates G-16 only.
- **G-15 cold-boot verification** — three power-cycles, logged.
- **G-17 dress rehearsal** — Friday.

---
---
# ADDENDUM — surface-only gig (added 2026-08-25, after Wave 3 dispatch)

The gig runs with **no monitor, keyboard or mouse**. Stream Deck XL + X-Touch
Mini only, HDMI straight to the crowd's TV. Two new specs: `G-18` (X-Touch via
WebMIDI) and `G-19` (headless ops). `G-10`'s key map is revised.

**Run G-19 Part A before anything else** — a permission dialog cannot be
dismissed at the gig, and it would hide the entire show behind a modal.

## 6. G-19 — headless operation  (mid, HIGHEST PRIORITY of the remaining work)

```
Repo: E:\__Projects\RPi-VJ

Read exactly two files:
  tasks/GIG-2026-08-29/START_HERE.md
  tasks/GIG-2026-08-29/specs/G-19-headless-ops.md

Context that changes the priority: on Saturday this Pi runs with NO monitor,
NO keyboard and NO mouse. Only a Stream Deck XL and an X-Touch Mini. Anything
requiring a click is unreachable, and a permission dialog would cover the show
on a 100-inch screen in front of an audience.

Do Part A (permission prompts) FIRST and report it before starting Part B.
It blocks the value of every other task in the project.

Parts B, C and D add: a two-step armed shutdown driven from the Stream Deck,
a heartbeat watchdog that restarts a HUNG (not crashed) page, and
last-state restore after a power blip.

Security note for Part B: the sudoers entry must grant exactly three commands,
never a wildcard. Validate with `visudo -c` before installing it. The
/system endpoint must remain bound to 127.0.0.1.

Observable: all four checks listed at the end of the spec, run on the Pi, with
results in RESULTS.md. Check 1 (cold boot with nothing attached but power,
HDMI, audio interface and the two controllers, reaching live visuals with zero
interaction) is the one that decides whether Saturday works.
```

## 7. G-18 — X-Touch Mini via WebMIDI  (mid, after G-07)

```
Repo: E:\__Projects\RPi-VJ

Read exactly two files:
  tasks/GIG-2026-08-29/START_HERE.md
  tasks/GIG-2026-08-29/specs/G-18-xtouch-midi.md

Build gig/src/xtouch.js as specified.

Two things the older project docs get WRONG — trust the spec, not the docs:
  - the X-Touch Mini has ONE fader, not eight (eight is the X-Touch Compact).
    It has 8 encoders with LED rings, 16 buttons, 1 fader, Layer A/B.
  - you must call requestMIDIAccess({sysex: false}). sysex triggers a permission
    prompt, and at the gig there is no mouse to dismiss it. LED rings work over
    plain CC, so sysex is never needed.

Drive the LED rings from real engine state, not from the last value received,
so that recalling a scene does not leave the rings misreporting the parameters.

Observable: on the Pi, headless — every encoder visibly changes the output on
the TV, rings track, all 16 buttons fire, fader crossfades, unplug/replug
recovers. Report the ACTUAL CC and note numbers you observe in RESULTS.md; they
may differ from the spec's table depending on the X-Touch Editor configuration.
```

## Revised G-10 note for whoever picks it up
The key map is now fixed: bottom row (24-31) is identical on every bank
(BANK◀ BANK▶ PAGE TAP / FX STOPALL SCENE BLACKOUT), banks cycle
CLIPS → SHADERS → SCENES → SYS, and the SYS bank carries the armed shutdown plus
live fps/temp/memory readouts drawn onto the key icons. Those readouts are the
only health telemetry available with no monitor attached.
