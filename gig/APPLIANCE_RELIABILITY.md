# Appliance reliability fixes — 2026-08-29

## Incident summary

Three observed symptoms shared startup/recovery coupling:

1. Chromium could show a white/blank output while its page or GPU path was no longer useful.
2. The cursor remained at the upper-left because child controls declared `cursor:pointer`, overriding the body’s inherited `cursor:none`.
3. Stream Deck keys could remain blank because the first hardware upload ran before `show.json` built key bindings; failed HID writes were then cached as successful, and reconnect did not request a repaint.

## Implemented corrections

- `show.json` persistently disables thumbnails with `streamDeckThumbnails: false`; restart no longer turns them back on.
- Show loading completes before Stream Deck initialization and first repaint.
- Every binding rebuild schedules a complete hardware repaint.
- HID writes use timers independent of the WebGL/rAF loop.
- A failed HID report throws and leaves its hash dirty; five bounded repaint attempts recover transient USB failures.
- Physical reconnect invalidates every cached key and requests a full repaint.
- Kiosk CSS hides the cursor on every descendant outside `?setup=1`.
- Concurrent clip loads are generation-cancelled and time out after 12 seconds.
- Video texture storage is allocated only on resolution changes; frames use `texSubImage2D`.
- Render heartbeat timestamps are written after the render path completes, not on entry.
- Watchdog health now includes heartbeat freshness.
- The systemd service kills the complete Chromium control group and escalates after a five-second stop timeout, preventing orphan renderer/GPU processes from surviving a restart.

## Deploy

From the Windows project root while the Pi is reachable:

```powershell
.\gig\tools\sync.ps1
ssh rpi@rpi-vj.local "cd ~/RPi-VJ && ./gig/deploy/install-units.sh"
```

The second command is required because copying the unit file does not update the live systemd unit.

## Immediate verification

On the Pi:

```bash
cd ~/RPi-VJ
./gig/deploy/install-units.sh --check
./gig/deploy/appliance-doctor.sh | tee ~/vj-results/doctor-before-soak.log
```

Expected:

- server and kiosk are active;
- watchdog `enabled`, `thread_alive`, `heartbeat_fresh`, and `healthy` are true after startup;
- the unit reports `KillMode=control-group`;
- thumbnails report persistently disabled;
- exactly one Chromium browser tree owns the kiosk profile;
- no recent OOM kill, V3D hang, or thermal throttle evidence.

## Fault tests

Perform these before trusting the appliance at a show:

1. Start on page 1 and trigger every clip twice, then rapidly trigger 1→8→2. The latest press wins; output never becomes an unrecoverable white page.
2. Disconnect/reconnect the Stream Deck. Labels return automatically within ten seconds without touching the browser.
3. Restart `vj-kiosk`. Labels return with no thumbnails and the cursor remains invisible.
4. In a diagnostic session, suspend Chromium or stop frame presentation. The watchdog must replace it after approximately 15 seconds.
5. Select a missing/corrupt clip. The current output remains; the failed load times out rather than poisoning later selections.
6. Run a minimum two-hour mixed clip/shader soak while recording `appliance-doctor.sh` before and after. The performance release gate remains eight hours.

## Recovery during a show

If output is bad but SSH is available:

```bash
systemctl --user restart vj-kiosk.service
```

If it does not recover within 20 seconds:

```bash
cd ~/RPi-VJ
./gig/deploy/appliance-doctor.sh > ~/vj-results/doctor-incident.log
systemctl --user stop vj-kiosk.service
systemctl --user kill --kill-who=all --signal=KILL vj-kiosk.service
systemctl --user start vj-kiosk.service
```

Preserve `~/vj-results`, the user journal, and the Chromium profile. Do not delete `~/.config/chromium`; it contains WebHID and audio permissions.

