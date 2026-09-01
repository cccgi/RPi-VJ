#!/usr/bin/env bash
# Reduce Chromium's memory footprint on a 2GB Pi 5.  Run ON THE PI as rpi.
#   ./gig/deploy/harden-kiosk-memory.sh            apply
#   ./gig/deploy/harden-kiosk-memory.sh --revert   restore the backup
#
# WHY
# `free -m` reports 2006 MB total -- this is a 2GB Pi, not a 4/8GB one -- with
# ~890 MB already in Chromium and 159 MB of swap in use while merely idle.
# Chromium renderers abort on failed allocation without the kernel OOM killer
# ever logging anything, which matches four Crashpad dumps in one day and an
# empty dmesg.
#
# The other half of the problem is that /usr/bin/chromium is a WRAPPER SCRIPT
# that sources /etc/chromium.d/* and injects flags nobody here asked for. The
# running process showed:
#     --force-renderer-accessibility     (builds an a11y tree for every frame)
#     --enable-remote-extensions --load-extension
#     --js-flags=--no-decommit-pooled-pages   (tells V8 to HOLD pooled memory)
# On a 2GB box driving WebGL at 1080p60 those are all working against us. This
# script points the unit at the real binary, bypassing the wrapper entirely,
# and sets an explicit flag set instead.
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }

U=~/.config/systemd/user/vj-kiosk.service
B="$U.bak-prememory"
[ -f "$U" ] || { echo "!! $U not found"; exit 1; }

if [ "${1:-}" = "--revert" ]; then
  [ -f "$B" ] || { echo "!! no backup at $B"; exit 1; }
  cp "$B" "$U"
  systemctl --user daemon-reload
  systemctl --user restart vj-kiosk.service
  echo "== reverted to the pre-hardening unit."
  exit 0
fi

[ -f "$B" ] || cp "$U" "$B"
echo "== backup at $B  (revert with: $0 --revert)"

REAL=/usr/lib/chromium/chromium
[ -x "$REAL" ] || { echo "!! $REAL not executable; aborting"; exit 1; }

python3 - "$U" "$REAL" <<'PY'
import re, sys
path, real = sys.argv[1], sys.argv[2]
s = open(path).read()

# Bypass the /etc/chromium.d wrapper.
s = s.replace("ExecStart=/usr/bin/chromium", "ExecStart=" + real)

EXTRA = [
    # One renderer is all a single-page kiosk needs.
    "--renderer-process-limit=1",
    "--process-per-site",
    # Undo the wrapper's memory-retention default and cap the JS heap.
    "--js-flags=--max-old-space-size=256",
    # Chromium was running an on-device AI model service on this Pi.
    "--disable-features=Translate,InfiniteSessionRestore,OptimizationGuideModelDownloading,OptimizationGuideOnDeviceModel",
    # NOT --media-cache-size / --disk-cache-size.
    # Setting these to 1 was a mistake: it forces Chromium to hold buffered
    # video in MEMORY instead of spilling it to disk, on the one machine that
    # cannot spare memory. It made the clip-press crash more frequent, not
    # less. The media cache is the pressure valve here, not the leak.
    # No extensions, no background chatter, no update checks on gig night.
    "--disable-extensions",
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-sync",
    # NOT --disable-breakpad: crash dumps are the only evidence we have about
    # why this Pi crashes while idle. Turning off crash reporting during a
    # crash investigation destroys the very thing being investigated.
]
for f in EXTRA:
    key = f.split("=")[0]
    if key not in s:
        s = s.replace("--kiosk --app=", f + " \\\n  --kiosk --app=", 1)

open(path, "w").write(s)
print("== unit rewritten")
PY

systemctl --user daemon-reload
systemctl --user restart vj-kiosk.service
sleep 6
systemctl --user --no-pager --lines=4 status vj-kiosk.service || true
echo
echo "== memory after restart:"
free -m
echo
ps -eo rss,comm --sort=-rss | head -6
echo
echo "== watchdog:"
curl -s http://127.0.0.1:8080/system/watchdog-status; echo
echo "== if the TV is wrong in ANY way, revert immediately:  $0 --revert"
