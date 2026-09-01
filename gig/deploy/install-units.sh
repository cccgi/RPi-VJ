#!/usr/bin/env bash
# THE ONLY script that installs systemd units. Run ON THE PI as rpi.
#
#   ./gig/deploy/install-units.sh          install/refresh + verify
#   ./gig/deploy/install-units.sh --check   verify only, change nothing
#
# WHY THIS EXISTS
# Seventeen scripts in deploy/ used to write or patch units, and three of them
# set the server to `python3 -m http.server` -- which serves the page but has
# no /heartbeat, no watchdog, no /system/shutdown and no telemetry. Running the
# wrong installer silently disarmed every recovery mechanism while leaving the
# show looking healthy. The kiosk unit likewise reverted to the /usr/bin
# Chromium wrapper, undoing the memory hardening.
#
# There is now exactly one pair of canonical units, checked into deploy/, and
# exactly one script that installs them.
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }

HERE="$(cd "$(dirname "$0")" && pwd)"
DEST="$HOME/.config/systemd/user"
CHECK_ONLY=0
[ "${1:-}" = "--check" ] && CHECK_ONLY=1

fail=0

# Read a live unit with COMMENTS STRIPPED.
#
# The canonical units document why certain flags are deliberately absent, so
# they contain the strings "http.server" and "--remote-debugging-port" inside
# comments. Grepping the raw unit therefore reported "server is a PLAIN FILE
# SERVER" on a unit that correctly runs collect_server.py -- a checker that
# contradicts itself in the same output. Only directives count.
live_directives() {
  systemctl --user cat "$1" 2>/dev/null | sed 's/#.*//' | grep -v '^[[:space:]]*$'
}

check_live() {
  local unit="$1" want="$2" label="$3"
  local live
  live="$(live_directives "$unit")"
  if [ -z "$live" ]; then
    echo "  !! $unit is not installed"; fail=1; return
  fi
  if echo "$live" | grep -qF -- "$want"; then
    echo "  ok  $label"
  else
    echo "  !! $label  -- NOT FOUND in the live unit"; fail=1
  fi
}

verify() {
  echo "== verifying the LIVE units (not the files on disk)"
  check_live vj-server.service "collect_server.py" \
    "server runs collect_server.py (heartbeat/watchdog/shutdown/telemetry)"
  check_live vj-kiosk.service "/usr/lib/chromium/chromium" \
    "kiosk bypasses the /etc/chromium.d wrapper"
  check_live vj-kiosk.service "--renderer-process-limit=1" \
    "kiosk memory hardening present"
  check_live vj-kiosk.service "KillMode=control-group" \
    "restart kills Chromium renderer/GPU child processes"
  check_live vj-kiosk.service "wlr-randr" \
    "1080p forced before Chromium starts"

  if live_directives vj-kiosk.service | grep -q -- "--remote-debugging-port"; then
    echo "  !! remote debugging port is ENABLED - remove before a show"; fail=1
  else
    echo "  ok  no remote debugging port"
  fi
  if live_directives vj-server.service | grep -q "http\.server"; then
    echo "  !! server is a PLAIN FILE SERVER - no watchdog, no shutdown"; fail=1
  else
    echo "  ok  server is not a plain file server"
  fi

  echo
  echo "== runtime"
  for u in vj-server vj-kiosk; do
    printf "  %-12s %s  restarts=%s\n" "$u" \
      "$(systemctl --user is-active $u.service 2>/dev/null || echo inactive)" \
      "$(systemctl --user show $u.service -p NRestarts --value 2>/dev/null || echo ?)"
  done
  local wd
  wd="$(curl -sf http://127.0.0.1:8080/system/watchdog-status 2>/dev/null || echo '')"
  echo "  watchdog     ${wd:-UNREACHABLE}"

  local thumbs
  thumbs="$(python3 - "$HERE/../show.json" <<'PY' 2>/dev/null || true
import json,sys
try: print(json.load(open(sys.argv[1],encoding='utf-8')).get('streamDeckThumbnails'))
except Exception: print('INVALID')
PY
)"
  if [ "$thumbs" = "False" ]; then
    echo "  ok  Stream Deck thumbnails disabled persistently"
  else
    echo "  !! show.json streamDeckThumbnails is $thumbs (expected false)"; fail=1
  fi
}

if [ "$CHECK_ONLY" -eq 1 ]; then
  verify
  [ "$fail" -eq 0 ] && echo && echo "== ALL CHECKS PASSED" || { echo; echo "!! CHECKS FAILED - run without --check to reinstall"; exit 1; }
  exit 0
fi

mkdir -p "$DEST"
for u in vj-server.service vj-kiosk.service; do
  if [ -f "$DEST/$u" ] && ! cmp -s "$HERE/$u" "$DEST/$u"; then
    cp "$DEST/$u" "$DEST/$u.bak-$(date +%Y%m%d-%H%M%S)"
    echo "== backed up the previous $u"
  fi
  cp "$HERE/$u" "$DEST/$u"
  echo "== installed $u"
done

systemctl --user daemon-reload
systemctl --user enable vj-server.service vj-kiosk.service 2>/dev/null || true
systemctl --user restart vj-server.service
sleep 2
systemctl --user restart vj-kiosk.service
sleep 6
echo
verify
echo
[ "$fail" -eq 0 ] && echo "== ALL CHECKS PASSED" || { echo "!! CHECKS FAILED"; exit 1; }
