#!/usr/bin/env bash
# Make the kiosk immune to Chromium's session-restore dialog, permanently.
# Run ON THE PI as rpi, no sudo.
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
U=~/.config/systemd/user/vj-kiosk.service
[ -f "$U" ] || { echo "!! $U not found"; exit 1; }

# sanitize the profile before every launch
grep -q 'sanitize-profile.sh' "$U" || \
  sed -i "s#^ExecStartPre=/bin/sh -c 'until curl#ExecStartPre=$REPO/gig/deploy/sanitize-profile.sh\nExecStartPre=/bin/sh -c 'until curl#" "$U"

# and belt-and-braces flags
grep -q 'hide-crash-restore-bubble' "$U" || \
  sed -i 's#--disable-session-crashed-bubble#--disable-session-crashed-bubble --hide-crash-restore-bubble --no-first-run --no-default-browser-check#' "$U"
grep -q 'InfiniteSessionRestore' "$U" || \
  sed -i 's#--disable-features=Translate#--disable-features=Translate,InfiniteSessionRestore#' "$U"

systemctl --user daemon-reload
systemctl --user restart vj-kiosk.service
sleep 4
grep -E 'ExecStartPre|disable-session|hide-crash' "$U" | sed 's/^/  /'
echo "== kiosk hardened against session-restore dialogs."
