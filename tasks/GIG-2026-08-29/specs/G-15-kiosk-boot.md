# G-15 — Kiosk Boot
**Model:** cheap · **Never cut. The rig must come up with no keyboard.**

## Deliverables
`gig/deploy/vj-kiosk.service`, `gig/deploy/vj-server.service`,
`gig/deploy/install.sh`

## The constraint that shapes this
WebHID needs a secure context, so the show is served over `http://localhost`,
not `file://`. Two units: the server must be up before the browser.

`vj-server.service` — `python3 -m http.server 8080` from the repo root,
`Restart=always`.

`vj-kiosk.service` — `After=vj-server.service graphical.target`, `Restart=always`,
running as the normal user (not root — udev handles the deck), launching:
```
chromium-browser --kiosk --app=http://localhost:8080/gig/ \
  --autoplay-policy=no-user-gesture-required \
  --disable-features=Translate,InfobarScreenshot \
  --disable-session-crashed-bubble --disable-infobars \
  --noerrdialogs --check-for-update-interval=31536000 \
  --enable-gpu-rasterization --ignore-gpu-blocklist \
  --disable-pinch --overscroll-history-navigation=0
```
`--check-for-update-interval` is not paranoia: a Chromium update prompt
appearing over the projector mid-set is a real failure mode.

## Also required
- Disable screen blanking and DPMS (`xset s off -dpms`, or the `wlr-randr`
  equivalent under Wayland — check which the Pi is running).
- Hide the cursor (`unclutter -idle 0`, or `--kiosk` plus a CSS
  `cursor:none` on the body).
- Force HDMI mode to 1920x1080@60 in `/boot/firmware/config.txt` rather than
  relying on EDID negotiation — a venue TV or a long HDMI run will otherwise
  hand you 30 Hz or 4K and you will not notice until it's dark.
- Disable Wi-Fi power management (`iw dev wlan0 set power_save off`) if you're
  controlling anything over the network.
- Autologin to the graphical session.

## install.sh
Idempotent. Copies units, copies the udev rule from G-09, enables everything,
prints what it did and what still needs a reboot.

## Observable
**Cold boot the Pi with no keyboard, no mouse, no SSH.** The show appears
fullscreen on the TV and the Stream Deck lights up. Do this **three times** —
boot races are intermittent and one success proves nothing. Record all three
outcomes and the boot-to-visuals time in `../RESULTS.md`.
