#!/usr/bin/env bash
# Run a probe page fullscreen on the TV. Works over ssh.
# Usage:  ./gig/deploy/run-probe.sh [probe.html|codec.html|shader-bench.html]
set -euo pipefail
PAGE="${1:-probe.html}"
CHROME="$(command -v chromium || command -v chromium-browser)"

# An ssh shell has no display env. The desktop session's env was imported into
# the systemd user manager by kiosk-autostart.sh — pull it back out of there.
eval "$(systemctl --user show-environment 2>/dev/null \
  | grep -E '^(WAYLAND_DISPLAY|DISPLAY|XDG_RUNTIME_DIR|XDG_SESSION_TYPE)=' \
  | sed 's/^/export /')" || true
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-wayland-0}"

if [ ! -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]; then
  echo "!! No wayland socket at $XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
  echo "!! Is the desktop logged in? Try: ~/RPi-VJ/gig/deploy/kiosk-autostart.sh"
  exit 1
fi
echo "== display: $WAYLAND_DISPLAY  runtime: $XDG_RUNTIME_DIR"

RES="$HOME/vj-results/$(echo "${PAGE%%.*}" | tr -d "-").log"
show_results() {
  echo
  if [ -f "$RES" ]; then
    echo "================ RESULTS ($RES) ================"
    cat "$RES"
  else
    echo "!! no results file at $RES"
  fi
  # Kill the probe browser too. Leaving it running is what poisoned the
  # profile for every subsequent run.
  pkill -f "user-data-dir=$HOME/.cache/vj-probe-profile" 2>/dev/null || true
  systemctl --user start vj-kiosk.service 2>/dev/null || true
  curl -sf -X POST "http://127.0.0.1:8080/system/watchdog-on" >/dev/null 2>&1 || true
  # VERIFY it actually came back. The watchdog is the only thing that recovers
  # a crashed kiosk when there is no keyboard at the gig, and it previously
  # died permanently the first time a probe disarmed it.
  WD="$(curl -sf "http://127.0.0.1:8080/system/watchdog-status" 2>/dev/null || echo '')"
  case "$WD" in
    *'"healthy": true'*) echo "== watchdog re-armed and healthy" ;;
    "")                  echo "!! could not read watchdog status - CHECK BEFORE THE GIG" ;;
    *)                   echo "!! WATCHDOG NOT HEALTHY: $WD"
                         echo "!! fix with: systemctl --user restart vj-server" ;;
  esac
  echo "== kiosk restored."
}
trap 'show_results; exit 0' INT TERM

systemctl --user stop vj-kiosk.service 2>/dev/null || true
systemctl --user start vj-server.service 2>/dev/null || true
until curl -sf "http://127.0.0.1:8080/gig/probe/$PAGE" >/dev/null; do sleep 0.5; done

# Suspend the watchdog only AFTER the server is confirmed up. Doing it first
# meant the request hit a server that was not listening yet, failed silently
# behind `|| true`, and left the watchdog armed for the whole probe -- which
# then restarted the kiosk on top of the probe window and lost the results.
if curl -sf -X POST "http://127.0.0.1:8080/system/watchdog-off" >/dev/null 2>&1; then
  echo "== watchdog suspended for this probe"
else
  echo "!! could not suspend the watchdog - it may restart the kiosk mid-probe"
fi

# verify the collector endpoint is live before relying on it
mkdir -p "$HOME/vj-results"
if curl -sf -X POST --data "selftest" "http://127.0.0.1:8080/collect?name=_selftest" >/dev/null 2>&1; then
  echo "== collector OK (POST /collect accepted)"
else
  echo "!! COLLECTOR NOT RUNNING — results will not be saved."
  echo "!! Run:  ./gig/deploy/patch-collector.sh   then retry."
  exit 1
fi
rm -f "$HOME/vj-results/$(echo "${PAGE%%.*}" | tr -d "-").log"

echo "== $PAGE is on the TV. Let the sweep finish, then press Ctrl-C here."

# SEPARATE PROFILE. Chromium allows one instance per user-data-dir; the probe
# used to share the kiosk's default profile, so whenever a SingletonLock from
# the kiosk was still present the probe forwarded its URL to that instance and
# exited instantly -- no window, no sweep, and a bare "no results file" that
# looked like the bench was broken. Its own profile removes the contention.
PROBE_PROFILE="$HOME/.cache/vj-probe-profile"
mkdir -p "$PROBE_PROFILE"
CHROME_LOG="$HOME/vj-results/chromium-probe.log"
: > "$CHROME_LOG"

# A probe Chromium from an earlier run survives Ctrl-C (the trap restores the
# kiosk but never killed it), keeps holding this profile, and every later run
# then reports "Opening in existing browser session" and exits in 0s. Clear it
# out before launching, and drop a stale singleton lock if one is left behind.
if pgrep -f "user-data-dir=$PROBE_PROFILE" >/dev/null 2>&1; then
  echo "== killing orphaned probe Chromium from a previous run"
  pkill -f "user-data-dir=$PROBE_PROFILE" 2>/dev/null || true
  for _i in $(seq 1 20); do
    pgrep -f "user-data-dir=$PROBE_PROFILE" >/dev/null 2>&1 || break
    sleep 0.25
  done
  pkill -9 -f "user-data-dir=$PROBE_PROFILE" 2>/dev/null || true
  sleep 0.5
fi
rm -f "$PROBE_PROFILE/SingletonLock" "$PROBE_PROFILE/SingletonSocket" \
      "$PROBE_PROFILE/SingletonCookie" 2>/dev/null || true

_t0=$(date +%s)
"$CHROME" --ozone-platform=wayland --enable-features=UseOzonePlatform \
  --user-data-dir="$PROBE_PROFILE" \
  --password-store=basic --use-mock-keychain \
  --kiosk --app="http://localhost:8080/gig/probe/$PAGE" \
  --autoplay-policy=no-user-gesture-required \
  --enable-gpu-rasterization --ignore-gpu-blocklist \
  --no-first-run --no-default-browser-check --noerrdialogs --disable-infobars \
  --disable-session-crashed-bubble --hide-crash-restore-bubble \
  --disable-features=Translate,InfiniteSessionRestore >"$CHROME_LOG" 2>&1 || true
_t1=$(date +%s)

# A browser that exits in seconds never rendered anything. Print WHY -- its
# stderr was previously sent to /dev/null, which is the only reason this took
# more than one round to diagnose.
if [ $((_t1 - _t0)) -lt 5 ]; then
  echo "!! Chromium exited after $((_t1 - _t0))s - it never displayed the page."
  echo "---- chromium output ------------------------------------------------"
  tail -30 "$CHROME_LOG" 2>/dev/null || echo "(no output captured)"
  echo "---------------------------------------------------------------------"
  echo "-- other chromium processes:"
  pgrep -a chromium || echo "   (none)"
fi

show_results
