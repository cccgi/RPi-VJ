#!/usr/bin/env bash
# Remove the --disk-cache-size=1 / --media-cache-size=1 flags from a unit that
# already has them. Those two forced buffered video to stay in RAM on a 2GB Pi
# and made the clip-press crash worse. Run ON THE PI.
set -euo pipefail
U=~/.config/systemd/user/vj-kiosk.service
sed -i 's/ --disk-cache-size=1//; s/ --media-cache-size=1//' "$U"
sed -i 's/--disk-cache-size=1 \\\\//; s/--media-cache-size=1 \\\\//' "$U"
sed -i '/^\s*--disk-cache-size=1 *\\\\*$/d; /^\s*--media-cache-size=1 *\\\\*$/d' "$U"
if grep -qE -- '--(disk|media)-cache-size' "$U"; then
  echo "!! still present:"; grep -nE -- '--(disk|media)-cache-size' "$U"
else
  echo "== cache-size flags removed"
fi
systemctl --user daemon-reload
systemctl --user restart vj-kiosk.service
sleep 5
systemctl --user is-active vj-kiosk.service
