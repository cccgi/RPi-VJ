#!/usr/bin/env bash
# Add --password-store=basic (kills the gnome-keyring prompt) and restart.
# Run ON THE PI as rpi, no sudo.
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }
U=~/.config/systemd/user/vj-kiosk.service
[ -f "$U" ] || { echo "!! $U not found — run install-desktop.sh first"; exit 1; }
grep -q 'password-store' "$U" || \
  sed -i 's|--kiosk --app=|--password-store=basic --use-mock-keychain --kiosk --app=|' "$U"
systemctl --user daemon-reload
systemctl --user restart vj-kiosk.service
sleep 4
systemctl --user --no-pager --lines=6 status vj-kiosk.service || true
echo "== keyring prompt disabled. The TV should show the RPi-VJ landing page."
