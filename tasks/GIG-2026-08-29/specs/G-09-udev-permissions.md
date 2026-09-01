# G-09 — udev Rule + Chromium Flags for WebHID
**Model:** cheap · **Blocks:** G-08

## Why
WebHID on Linux needs the browser's user to have read/write on the hidraw node.
By default only root does, so `requestDevice()` shows the device and then fails
to open — a confusing failure mode worth eliminating before G-08 debugging.

## Deliverable
`gig/deploy/60-streamdeck.rules`:
```
# Elgato Stream Deck XL — allow non-root access for WebHID
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="006c", MODE="0660", GROUP="plugdev", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="008f", MODE="0660", GROUP="plugdev", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="006c", MODE="0660", GROUP="plugdev", TAG+="uaccess"
```
Install:
```sh
sudo cp gig/deploy/60-streamdeck.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG plugdev $USER    # then log out and back in
```

## Chromium
WebHID requires a secure context. `file://` is **not** one. Serve the show over
`http://localhost` — add `gig/tools/serve.sh`:
```sh
#!/bin/sh
cd "$(dirname "$0")/.." && python3 -m http.server 8080
```
`http://localhost:8080` counts as a secure context. Note this in the runbook —
it changes how G-15's kiosk unit launches (it must start the server first).

Also confirm `chrome://device-log` and `navigator.hid` exist in the Pi's
Chromium build. If WebHID is missing entirely, **stop and report immediately** —
that invalidates G-08 and we fall back to keyboard control from the cut list.

## Observable
`ls -l /dev/hidraw*` shows group `plugdev` with the deck plugged in, and
`navigator.hid.requestDevice({filters:[{vendorId:0x0fd9}]})` in the Pi's
Chromium console lists the Stream Deck and opens without error. Paste both
into `../RESULTS.md`.
