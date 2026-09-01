#!/usr/bin/env bash
# One-time setup session: pair the Stream Deck and grant mic permission.
# Needs a MOUSE attached to the Pi — navigator.hid.requestDevice() and the
# media permission prompt both require a real user gesture. Both grants then
# persist in the Chromium profile, so the gig runs headless.
# Run ON THE PI:  ./gig/deploy/setup-pair.sh
set -euo pipefail
CHROME="$(command -v chromium || command -v chromium-browser)"
eval "$(systemctl --user show-environment 2>/dev/null \
  | grep -E '^(WAYLAND_DISPLAY|DISPLAY|XDG_RUNTIME_DIR|XDG_SESSION_TYPE)=' \
  | sed 's/^/export /')" || true
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"

cat <<'MSG'
=========================================================================
 SETUP MODE — a mouse must be plugged into the Pi.
 On the TV you will see the show with the cursor visible.

   1. Click  PAIR STREAM DECK  (magenta, bottom-left)
      -> choose the Stream Deck in the chooser -> keys light up with labels
   2. Click  Start Audio  -> Allow  (grants the microphone/line-in)

 Both grants persist in the Chromium profile. Close the window when done.
 NEVER delete ~/.config/chromium — that is where these grants live.
=========================================================================
MSG

systemctl --user stop vj-kiosk.service 2>/dev/null || true
# make sure NOTHING else is holding the profile — two instances sharing one
# profile is what produced the nested double show
pkill -f 'chromium.*localhost:8080' 2>/dev/null || true
sleep 2
"$(dirname "$0")/sanitize-profile.sh"
systemctl --user start vj-server.service 2>/dev/null || true
until curl -sf http://127.0.0.1:8080/gig/ >/dev/null; do sleep 0.5; done

"$CHROME" --ozone-platform=wayland --enable-features=UseOzonePlatform \
  --password-store=basic --use-mock-keychain \
  --disable-session-crashed-bubble --hide-crash-restore-bubble \
  --noerrdialogs --no-first-run --no-default-browser-check \
  --disable-features=Translate,InfiniteSessionRestore \
  --autoplay-policy=no-user-gesture-required \
  --enable-gpu-rasterization --ignore-gpu-blocklist \
  --start-fullscreen "http://localhost:8080/gig/?setup=1" >/dev/null 2>&1 || true

echo
echo "== verifying what was granted..."
sleep 1
systemctl --user start vj-kiosk.service 2>/dev/null || true
sleep 6
echo "--- show.log tail ---"
tail -n 20 "$HOME/vj-results/show.log" 2>/dev/null || echo "(no log yet)"
