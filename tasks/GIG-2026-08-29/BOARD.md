# GIG BOARD — Sat 29 Aug 2026
**The only status file. One line edited per task. Read this, not the briefings.**
Spec G-20: `G-20_COMPOSITE_FX.md` · Refs: `refs/`
Protocol: `../AGENT_PROTOCOL_V2.md` · Plan: `GIG_PLAN.md` · Results: `RESULTS.md`

Target tree: everything new lives in `gig/`. Do not modify `vj/` or `src/`.

| ID | Task | File(s) | Model | Blocks | Status |
|---|---|---|---|---|---|
| **G-01** | Pi 5 GPU + WebGL2 capability probe | `gig/probe/probe.html`, `run-probe.sh` | cheap | ALL | **DONE** |
| **G-02** | Codec decision test (H.264 vs HEVC on Pi) | `gig/probe/codec.html` | cheap | G-06, G-16 | NEEDS-REVIEW(awaiting Pi run) |
| **G-03** | WebGL2 fullscreen-quad shader host | `gig/src/gl.js` | cheap | G-04,G-07 | **DONE** |
| **G-04** | ISF-lite parser — **VERIFY vs u_* convention** | `gig/src/isf.js` | cheap | G-07 | **DONE** |
| **G-05** | Web Audio 5-band analyser + kick detect | `gig/src/audio.js` | cheap | G-07 | **DONE** |
| **G-06** | Dual video deck → WebGL texture | `gig/src/deck.js` | cheap | G-07 | **DONE** |
| **G-07** | Compositor: deck × shader × blend | `gig/src/mixer.js` | mid | G-12 | **DONE** |
| **G-08** | WebHID Stream Deck XL driver | `gig/src/streamdeck.js` | mid | G-10 | **DONE** |
| **G-09** | udev rule + Chromium flags for WebHID | `gig/deploy/60-streamdeck.rules` | cheap | G-08 | **DONE** |
| **G-10** | show.json → 32-key binding + blackout | `gig/src/show.js`, `gig/show.json` | cheap | — | **DONE** |
| **G-11** | Shader perf audit on Pi (13 real) | `gig/probe/PERF.md` | cheap | G-14 | **DONE** |
| **G-12** | Post chain: feedback/bloom/CA/grain | `gig/src/post.js` | mid | — | NEEDS-REVIEW(awaiting Pi run) |
| **G-13** | Beat choreography / modulation matrix | `gig/src/choreo.js` | mid | — | NEEDS-REVIEW(awaiting Pi run) |
| **G-14** | 4 new Pi-budget shaders | `gig/shaders/*.fs` | mid | — | NEEDS-REVIEW |
| **G-15** | Kiosk systemd unit + autostart | `gig/deploy/` | cheap | — | **DONE** (needs 3x cold-boot log) |
| **G-16** | Transcode clip library on the 3090 | `gig/tools/transcode.sh` | cheap | — | NEEDS-REVIEW |
| **G-17** | Dress rehearsal + failure playbook | `gig/RUNBOOK.md` | mid | — | NEEDS-REVIEW |
| **G-18** | X-Touch Mini via WebMIDI | `gig/src/xtouch.js` | mid | — | NEEDS-REVIEW |
| **G-19** | Headless ops: permissions, shutdown, watchdog | `gig/tools/`, `gig/deploy/`, `gig/src/` | mid | — | **DONE** |
| **G-20** | Composite FX ids (mirror+posterize+palette) | `gig/src/mixer.js`, `gig/show.json` | cheap | — | TODO |
## Measured constraints (2026-08-25) — build against these, not guesses
- **9 shaders hold 1080p60**; see `gig/probe/PERF.md` for the set list.
- **4 raymarchers are cut or need optimising** — granular_nebula is unrecoverable.
- **No float-texture linear filtering** → G-12 bloom chain must be RGBA8.
- **Shaders use a `u_*` convention, not stock ISF** → G-04/`isf.js` must match.
- Do NOT trust any benchmark using `gl.finish()` or synthetic rAF timing here.

## Critical path
`G-01 → G-03 → G-04 → G-07 → G-12` — everything else is parallel or optional.

## SURFACE-ONLY constraint (added 2026-08-25)
The gig has **no monitor, keyboard or mouse** — Stream Deck XL + X-Touch Mini only.
- Every function must be on a key or encoder, including **shutdown**.
- **No permission dialog may ever appear** — it cannot be dismissed. See G-19 Part A.
- Recovery from a hang/crash must be automatic (G-19 Part C).
This raises G-08, G-10, G-18, G-19 to must-have. G-08 is no longer cuttable —
without the Stream Deck there is no way to control anything.

## Cut list (in order, if Friday arrives and things are red)
1. G-14 new shaders — the existing 14 are enough
2. G-13 choreography — manual Stream Deck control substitutes
3. G-06/G-07 video clips — **shaders-only is a complete, good show**
4. ~~G-08 Stream Deck~~ — **NO LONGER CUTTABLE.** There is no keyboard at the gig.
5. G-18 X-Touch — cuttable; the Stream Deck alone can run the show.

Never cut: G-15 (it must boot), G-19 (no prompts, working shutdown),
G-08 (the only control surface), and the blackout key in G-10.

## Status legend
`TODO` `WIP` `BLOCKED(why)` `NEEDS-REVIEW` `DONE`
`DONE` = the spec's observable was observed **on the Pi**. Not "it compiles."
