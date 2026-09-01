# G-19 — Headless Operation: Permissions, Shutdown, Watchdog
**Model:** mid · **NEW, added 2026-08-25** · **Never cut.**

Saturday's rig has **no monitor, no keyboard, no mouse.** Only the Stream Deck XL
and X-Touch Mini. Anything that needs a click is a dead show.

## Part A — permission prompts (do this first, it blocks everything)
`getUserMedia` (Sound Blaster) and possibly WebMIDI raise permission dialogs.
Unclickable at the gig. Belt **and** braces:

1. **Persist the grant in the Chromium profile.** With keyboard and mouse
   attached during setup, load the show, click Allow once. The kiosk uses a
   persistent profile, so it survives reboots. **Never wipe that profile dir.**
2. **Add the auto-accept flag** to `vj-kiosk.service` as a backstop:
   `--use-fake-ui-for-media-stream`
   It auto-grants media capture. It may select the *default* input, so
   `audio.js` must still enumerate devices and re-open pinned to the Sound
   Blaster by label (G-05 already does this).
3. Use `requestMIDIAccess({sysex:false})` so MIDI never prompts (see G-18).

**Observable:** boot the Pi with **nothing attached but power, HDMI, the Sound
Blaster, and the two controllers.** Audio-reactive visuals must start with zero
interaction. If any dialog appears, this task is not done.

## Part B — shutdown from the Stream Deck
A browser cannot power off a machine. Route it through the local server.

Extend `gig/tools/collect_server.py` with `POST /system/<action>` for
`shutdown` | `reboot` | `restart-kiosk`, bound to **127.0.0.1 only** (it already is).

Grant only these three commands, nothing broader:
```
# /etc/sudoers.d/vj-power   (visudo -c to validate before installing)
rpi ALL=(root) NOPASSWD: /sbin/shutdown, /sbin/reboot, /bin/systemctl restart vj-kiosk
```

### Two-step arming — mandatory
A single accidental press must never end the night.
- `SHUTDOWN` arms and turns red; a 5-second countdown renders on the key icon.
- A **different, physically distant** key must confirm inside that window.
  Put ARM at key 0 (top-left of the SYS page) and CONFIRM at key 7 (top-right)
  so no thumb reaches both.
- Any other key press disarms.
- Before powering off, show `SHUTTING DOWN` fullscreen on the TV for ~2s so
  Frank can see it took, then `sudo shutdown -h now`.

## Part C — watchdog
`Restart=always` covers a Chromium crash. It does not cover a *hung* page.
Add a heartbeat: the show POSTs `/heartbeat` every 2s; if the server sees no
heartbeat for 15s it restarts `vj-kiosk`. Log every restart with a timestamp.

Recovery must be automatic and under ~10 seconds. Frank cannot SSH in mid-set.

## Part D — boot resilience
- Autostart the last-used show/bank so a mid-set power blip returns to roughly
  where he was, not to a default state. Persist to `~/vj-state.json` on change.
- If `show.json` is missing or unparseable, boot to black **with a working
  blackout key and working bank navigation** rather than to an error page.

## Observable
1. Cold boot, nothing but power/HDMI/audio/controllers → visuals, no prompts.
2. Shutdown arm + confirm powers the Pi off cleanly. Single press does not.
3. `pkill -STOP chromium` (a hang, not a crash) → watchdog restarts within 15s.
4. Pull power mid-show, boot → returns to the last bank.
Record all four in `../RESULTS.md`.
