# RPi-VJ RUNBOOK — Saturday 29 August 2026
**Print this. Tape it to the case.**
Build string in every log line: `aud9-lockwindow6s` — if the log says something else, you are looking at old code.

---

## Stream Deck XL layout (keys 0-31, bottom-right = key 31)

```
[  0][ 1][ 2][ 3][ 4][ 5][ 6][ 7]   row 0: pads 0-7  (clips/shaders per bank)
[  8][ 9][10][11][12][13][14][15]   row 1: pads 8-15
[ 16][17][18][19][20][21][22][23]   row 2: pads 16-23
[ 24][25][26][27][28][29][30][31]   row 3: SYSTEM ROW
  <B  B>  --  --  --  --  TAP BLACK
```

- **Key 24** — `< BANK` (previous bank)
- **Key 25** — `BANK >` (next bank)
- **Key 30** — `TAP` (tap tempo)
- **Key 31** — `BLACK` (blackout, always lit RED — this key always works)

Banks in order: **CLIPS → MIX → SYS → FX → FX-LAYER**

---

## Pre-show checklist (do at venue, before load-in)

- [ ] Pi powered, active cooler spinning
- [ ] micro-HDMI cable seated in port **0** (closest to USB-C power)
- [ ] TV on correct input, showing VJ output
- [ ] Sound Blaster Play! 3 USB dongle in Pi USB-A port (3.0 preferred)
- [ ] Booth line-in cable from DDJ-FLX10 AUX → Sound Blaster 3.5mm input
- [ ] Stream Deck XL USB connected to Pi
- [ ] Stream Deck keys lighting up (if dark after 10s, see STREAM DECK DARK below)
- [ ] Sync from Windows laptop: `.\gig\tools\sync.ps1` (must say VERIFIED)

---

## Cold boot sequence (normal case — verified working)

1. Power on Pi. Wait 30–45 s.
2. TV shows Chromium fullscreen with the VJ grid. Stream Deck lights up.
3. **Verify build string is current:**
   ```
   ssh rpi@rpi-vj.local 'grep UNI ~/vj-results/show.log | tail -3'
   ```
   Every line must end `build=aud9-lockwindow6s`. If not, sync.ps1 was not run or a stale process is running.
4. **Verify audio is live:**
   ```
   ssh rpi@rpi-vj.local 'grep UNI ~/vj-results/show.log | tail -3'
   ```
   Look for `vol=` and `bass=` — values should move when music plays. `vol=0.00` with music playing means the audio device is not selected (see AUDIO DEAD below).
5. **Check temp:**
   ```
   ssh rpi@rpi-vj.local 'vcgencmd measure_temp'
   ```
   Warn at 75 °C, act at 80 °C (reduce shader complexity or cut post chain).

---

## During the show

### Switching scenes
- Use **< BANK / BANK >** (keys 24/25) to cycle: CLIPS → MIX → SYS → FX → FX-LAYER.
- Heavy shaders (POLYHEDRA, SHATTERED) **auto-drop to 540p** the moment they load — this is intentional, not a crash. STRATA and PARTICLE auto-drop to 720p.
- **DEEPDRIFT** (SYS key 14) is optimised for ambient use; it is the safe fallback if other shaders stutter.

### Blackout
Key 31 (bottom-right, RED). Press again to restore. **This key always works even if everything else is broken.**

### Tap tempo
Key 30 (`TAP`). 4+ taps sets BPM. The clock auto-locks to kick onsets when they are steady; `locked` readings 0–1 in the log are normal during breakdowns — this is not a fault.

### Monitoring (leave open on laptop during set)
```bash
ssh rpi@rpi-vj.local 'tail -f ~/vj-results/show.log | grep UNI'
```

---

## Failure playbook

### BLACK SCREEN (no output on TV)
1. Check TV input. Check micro-HDMI cable.
2. `ssh rpi@rpi-vj.local 'systemctl --user status vj-kiosk'`
   - If failed/inactive: `systemctl --user restart vj-kiosk` (services auto-restart every 3 s but this forces it immediately)
3. Check for JS errors: `ssh rpi@rpi-vj.local 'grep -i error ~/vj-results/show.log | tail -10'`
4. Hard reload: `ssh rpi@rpi-vj.local 'systemctl --user restart vj-kiosk vj-server'`
5. If still black after 30 s: press **key 31** (blackout off) — the canvas may be blacked out, not broken.

### AUDIO DEAD (vol=0.00 with music playing, no visual reaction)
1. Check booth cable is in the Sound Blaster's **input** jack (not headphone out).
2. `ssh rpi@rpi-vj.local 'arecord -l'` — Sound Blaster Play! 3 must appear.
3. If missing: unplug/replug the Sound Blaster USB, then `systemctl --user restart vj-server vj-kiosk`.
4. **Fallback:** the internal clock runs at 128 BPM. Visual motion continues — only audio reactivity is lost. This is acceptable.

### STREAM DECK DARK (no LEDs after 10 s)
1. Unplug/replug Stream Deck USB.
2. `ssh rpi@rpi-vj.local 'grep -i streamdeck ~/vj-results/show.log | tail -5'`
3. If "WebHID not available": the udev rule may not have run. `ssh rpi@rpi-vj.local 'sudo udevadm control --reload && sudo udevadm trigger'`, then restart kiosk.
4. **The show runs without the Stream Deck** (shaders auto-play) — blackout is only lost. SSH can blackout: `ssh rpi@rpi-vj.local 'curl -s http://localhost:8080/api/blackout'`.

### SHADER COMPILE FAIL or STUTTER
1. Log: `ssh rpi@rpi-vj.local 'grep "COMPILE\|FAIL\|fps" ~/vj-results/show.log | tail -10'`
2. Switch to a different shader via the Stream Deck.
3. If any shader stalls at full-screen resolution, the governor steps down automatically (1.0→0.85→0.7→0.55→0.4). Let it settle — do not restart.
4. Safe fallback shaders (confirmed >1.5x headroom): **DEEPDRIFT**, **PLASMA**, **TUNNEL**, **KALEIDO**, **LIQMETAL**.

### CLIP STUCK / WON'T PLAY
1. Press a different clip key. Stuck clips are per-instance — switching recovers immediately.
2. If all clips fail: `ssh rpi@rpi-vj.local 'grep -i video ~/vj-results/show.log | tail -5'`
3. **Shaders-only is a complete, good show.** Switch to MIX or SYS bank and use shaders with no clip underneath.

### OVERHEATING (> 80 °C)
1. Cut the post chain if it is on (it should be off by default — do not enable it at the gig without pre-benching on the Pi).
2. Switch to lower-cost shaders: DEEPDRIFT, PLASMA, OSCILLO.
3. Reduce CLIPS bank usage (clips + shader compositing costs more than shader-only).
4. If temp exceeds 85 °C: controlled shutdown (see SHUTDOWN below), let the Pi cool 5 min, reboot.

### COMPLETE CRASH / REBOOT LOOP
- vj-kiosk.service and vj-server.service both have `Restart=always, RestartSec=3`. Wait 15 s — they will come back.
- If still looping: `ssh rpi@rpi-vj.local 'journalctl --user -u vj-kiosk -n 30'`
- Last resort: `ssh rpi@rpi-vj.local 'sudo reboot'` — cold boot is verified working (30–45 s to live).

---

## Shutdown

```bash
ssh rpi@rpi-vj.local 'sudo poweroff'
```

Wait for the green activity LED to stop blinking before pulling power. Pulling power mid-write can corrupt the SD card.

---

## Post chain (G-12) — DO NOT ENABLE AT THE GIG without prior bench

The post chain (trails/bloom/CA/grain) is wired in and RGBA8-clean but has **not been benched** on this Pi. Its per-frame cost is unknown. If you want to enable it for dress rehearsal:

```bash
# On the Pi, run the probe with post chain forced on:
ssh rpi@rpi-vj.local '~/RPi-VJ/gig/deploy/run-probe.sh shader-bench.html'
# Then check headroom with post on vs off:
ssh rpi@rpi-vj.local 'grep -E "fps|headroom|post" ~/vj-results/show.log | tail -20'
```

Enable only if every shader still shows ≥ 1.2× headroom with the post chain running. If any shader drops below 1.0× with it on, leave `postEnabled = false`.

---

## Cut list (if something is not working Friday night — cut it, don't fix it)

1. **X-Touch Mini** — Stream Deck alone runs the show.
2. **Beat choreography (G-13)** — visuals still react to audio; manual control substitutes.
3. **Video clips** — shaders-only is a complete, good show. Switch to MIX/SYS bank.
4. **Post chain** — leave `postEnabled = false` (already the default).
5. ~~Stream Deck~~ — NOT cuttable. There is no keyboard at the gig.
6. ~~Blackout (key 31)~~ — NOT cuttable. It must always work.
7. ~~Boot~~ — NOT cuttable. If it won't boot Friday, fix it Friday.
