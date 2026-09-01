# RPi-VJ — state at 2026-08-26, audio path CLOSED

Gig: Sat 29 Aug. Read this file only; do not re-read the old transcripts.

## Ground rules that were learned the hard way
1. When a guess about an interface fails once, **print the actual values**.
   Never reason a second time. Every bug below was found by one dump.
2. `.\gig\tools\sync.ps1` hashes every file against the Pi and refuses to
   restart if any differ. Trust it. If it says verified, the Pi is current.
3. Every `UNI` diag line ends in `build=<AUDIO_BUILD>` (const at top of
   `gig/src/mixer.js`). **Bump it with any audio change.** If the log does not
   show your build string, you are looking at old code — fix that first.
4. `run-probe.sh` must run ON THE PI, not in PowerShell:
   `ssh rpi@rpi-vj.local '~/RPi-VJ/gig/deploy/run-probe.sh shader-bench.html'`
5. `python3 gig/tools/check-uniforms.py` must pass. It fails if
   `probe/shader-bench.html` and `src/isf.js` disagree about which uniforms
   exist — that mismatch caused five false COMPILE FAILs.

## Diagnostics available with no keyboard/monitor
- `ssh rpi@rpi-vj.local 'grep UNI ~/vj-results/show.log | tail -5'`
  Prints every audio uniform the shaders receive, every 2s, plus PEAK-held
  transients and RAW pre-gate band readings with gate verdicts.
- `~/vj-results/chromium-probe.log` — probe browser stderr.
- `~/vj-results/show.log` — all VJ_DIAG output including JS errors.

## Audio: FIXED (verified in the log, build aud8-peakhold)
Five bugs, all confirmed corrected:
- Spectrum bins were normalised against the BASS band's floor/ceiling, so all
  32 read 0. Now per-bin slow peak. (`live=11-19/32`)
- Energy was a flat mean of 5 bands; dead bands dragged it to 0.10 while bass
  was 0.76. Now `0.6*max + 0.4*mean`. (`vol=0.64`)
- Onset/hit detectors compared a band to its own FAST envelope — could never
  fire. Now a 400ms slow reference. (`PEAK onset=1.00 hit=1.00/1.00/1.00`)
- The 5 wide bands used a flat mean; treble (5k-16k) was dragged to 0 by the
  empty top of the band. Now `0.35*mean + 0.65*peak`. (`treb` peaks 0.93)
- Server sent no cache headers; Chromium could run stale ES modules. Now
  `no-store` in `collect_server.py`.

## Engine additions worth knowing
- `u_flow`, `u_flow_low/mid/high`, `u_spec_flow[32]`: audio-INTEGRATED clocks.
  Rate follows the music, so a shader that uses these instead of `u_time`
  changes SPEED with the track and never jumps. This is the fix for "audio
  only changes brightness, not motion".
- `u_locked`: 0..1 tempo-clock confidence. Multiply beat/bar-phase terms by it
  so a free-running clock never drives visuals.

## G-02 CODEC: CLOSED — 2026-08-26. KEEP H.264. DO NOT TRANSCODE.
Measured on the Pi, not inferred:
    canPlayType hvc1 : ""        <- empty = unsupported
    canPlayType hev1 : ""
    canPlayType avc1 : "probably"
    VIDEO ERROR: HEVC NOT DECODABLE
Chromium on this Raspberry Pi OS build has NO HEVC decoder. Transcoding the
library to HEVC would not accelerate anything -- it would stop all 28 clips
playing. The Pi's hardware HEVC block is irrelevant because the browser cannot
reach it. x265 also encoded 720p at only 10.6 fps here, so a full library pass
would take most of a day to produce an unplayable library.
The clips are h264 High / 1280x720 / 30fps / 3.5-8 Mbps and decode in software
at roughly 20% of one A76 core. This is settled; do not revisit.

## (superseded, kept for context) original codec caution
The 28 clips are ALREADY h264 High / 1280x720 / 30fps / 3.5-8 Mbps. Only the
filenames say "4K". 720p30 h264 software decode costs ~20% of one A76 core, so
this is NOT the bottleneck. Chromium on RPi OS often ships WITHOUT HEVC, in
which case transcoding stops clips playing rather than speeding them up.
Run `./gig/tools/test-hevc-support.sh` then
`./gig/deploy/run-probe.sh hevc-test.html` and read `~/vj-results/hevc.log`.
Only transcode if that log says HEVC WORKS *and* the load test below fails.

## OPEN — in priority order
1. `deep_drift` at 1.0x headroom (no room for a clip underneath or post FX).
   `liquid_metal` was fixed the same way: 3 `liquidHeight()` taps -> 1, using
   `dFdx/dFdy`. Apply the same pattern.
2. `vs_sacred_polyhedron`, `vs_shattered_glitch_core` need 540p;
   `strata_drift`, `particle_warp` need 720p. Confirm the mixer actually
   drops shader FBO resolution per-shader, or they will stutter live.
3. `locked` oscillates 0.06-1.00. Phase-lock gain is 0.25 in
   `choreo.js:trackKick`. Raise if it will not hold.
4. G-12 post chain (trails/bloom) written, never verified. Must be RGBA8:
   `OES_texture_float_linear` is NOT available on this Pi.
5. G-17 dress rehearsal Friday + RUNBOOK. (G-15 cold boot: VERIFIED by user.
   Shutdown: VERIFIED by user.)
7. COSMETIC, DO LAST, ON A CLONED SD CARD: custom boot splash, and booting
   straight to kiosk without the desktop. Cold boot is currently PROVEN
   WORKING; changing the boot path risks the one thing that is known good.
   Do not touch this before the dress rehearsal.
   - Splash only (low risk): replace /usr/share/plymouth/themes/pix/splash.png
     with a 1920x1080 PNG, then `sudo plymouth-set-default-theme -R pix`.
     Also `sudo raspi-config` -> Boot -> Splash to hide the rainbow/text.
   - Skip desktop (higher risk): the kiosk currently depends on labwc providing
     the wayland socket that vj-kiosk.service and run-probe.sh both use. The
     replacement is `cage` on a TTY (see deploy/run-probe-tty.sh, which already
     does exactly this). Savings are ~150MB RAM and a few seconds of boot, NOT
     GPU time -- the desktop is idle once the kiosk is fullscreen. Low reward,
     real risk.
6. X-Touch Editor: set the 8 page buttons to `Toggle` LED mode (manual step).

## Do NOT re-litigate
`tasks/ENGINE_REALITY_CHECK.md` — the `vj/` Rust workspace has no I/O drivers.
The whole engine is `gig/` running in Chromium. 18 shaders, all compile, all
placed exactly once across pages CLIPS/MIX/SYS.
