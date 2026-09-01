#!/usr/bin/env bash
# Apply the 1080p-default-at-boot crash fix to the LIVE unit on the Pi.
# Run ON THE PI as rpi:  ./gig/deploy/apply-1080p-default.sh [--revert]
#
# WHY THIS SCRIPT EXISTS INSTEAD OF JUST SYNCING deploy/vj-kiosk.service:
# The deployed unit is ~/.config/systemd/user/vj-kiosk.service (a USER unit),
# and per prior investigation it is NOT a copy of this repo's
# deploy/vj-kiosk.service -- it runs plain chromium with hand-patched flags,
# not the repo's version. `sync.ps1` only copies the repo tree to
# ~/RPi-VJ/gig/, it does not touch ~/.config/systemd/user/. Copying the repo
# file over the live unit would also silently discard whatever hand-patches
# are already live (harden-kiosk-memory.sh's renderer-process-limit etc).
# So this script inserts ONLY the new ExecStartPre line into the unit that's
# actually running, the same way harden-kiosk-memory.sh does.
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }

U=~/.config/systemd/user/vj-kiosk.service
B="$U.bak-pre1080default"
[ -f "$U" ] || { echo "!! $U not found"; exit 1; }

if [ "${1:-}" = "--revert" ]; then
  [ -f "$B" ] || { echo "!! no backup at $B"; exit 1; }
  cp "$B" "$U"
  systemctl --user daemon-reload
  systemctl --user restart vj-kiosk.service
  echo "== reverted -- Pi will boot at whatever the TV negotiates again."
  exit 0
fi

[ -f "$B" ] || cp "$U" "$B"
echo "== backup at $B  (revert with: $0 --revert)"

MARKER="# vj-1080p-default"
if grep -q "$MARKER" "$U"; then
  echo "== already applied (marker found), skipping insert"
else
  python3 - "$U" "$MARKER" << 'PY'
import sys
path, marker = sys.argv[1], sys.argv[2]
s = open(path).read()
line = (
    marker + "\n"
    "ExecStartPre=-/bin/sh -c '/usr/bin/wlr-randr --output HDMI-A-1 --mode 1920x1080@60 2>/dev/null "
    "|| /usr/bin/wlr-randr --output HDMI-A-2 --mode 1920x1080@60 2>/dev/null "
    "|| /usr/bin/xrandr --output HDMI-1 --mode 1920x1080 2>/dev/null || true'\n"
)
if "ExecStart=" not in s:
    raise SystemExit("!! no ExecStart= line found, aborting")
# Insert right before the first ExecStart= (the chromium launch line).
idx = s.index("ExecStart=")
s = s[:idx] + line + s[idx:]
open(path, "w").write(s)
print("== inserted 1080p-forcing ExecStartPre before ExecStart=")
PY
fi

systemctl --user daemon-reload
systemctl --user restart vj-kiosk.service
sleep 6
systemctl --user --no-pager --lines=6 status vj-kiosk.service || true
echo
echo "== confirm the actual mode Chromium is now sizing against:"
wlr-randr 2>/dev/null | grep -A2 "HDMI-A-1\|HDMI-A-2" || xrandr 2>/dev/null | grep -A1 connected
echo
echo "== if the TV goes wrong in ANY way, revert immediately:  $0 --revert"
