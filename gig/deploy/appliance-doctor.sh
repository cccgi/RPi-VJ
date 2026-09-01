#!/usr/bin/env bash
# Read-only incident snapshot for the live Pi. Safe during a show.
# Usage: ./gig/deploy/appliance-doctor.sh | tee ~/vj-results/doctor-$(date +%s).log
set -u

echo "== RPi-VJ APPLIANCE DOCTOR $(date --iso-8601=seconds)"
echo "host=$(hostname) kernel=$(uname -r) arch=$(uname -m)"
echo

echo "== canonical unit verification"
"$(dirname "$0")/install-units.sh" --check || true
echo

echo "== watchdog"
curl -sf http://127.0.0.1:8080/system/watchdog-status || echo UNREACHABLE
echo
echo

echo "== memory / pressure"
free -h
grep -E 'MemAvailable|SwapFree|Slab|SReclaimable' /proc/meminfo || true
echo

echo "== Chromium process tree"
ps -eo pid,ppid,rss,stat,etimes,cmd --sort=-rss | grep -E '[c]hromium|[c]ollect_server' | head -30
echo

echo "== service state"
systemctl --user --no-pager status vj-kiosk.service vj-server.service || true
echo

echo "== recent kiosk/server journal"
journalctl --user -u vj-kiosk.service -u vj-server.service --since '-2 hours' --no-pager -n 250 || true
echo

echo "== kernel OOM / GPU / DRM evidence"
journalctl -k --since '-2 hours' --no-pager 2>/dev/null \
  | grep -Ei 'oom|out of memory|killed process|v3d|gpu|drm|hang|fault|thrott' | tail -150 || true
echo

echo "== application/watchdog tail"
tail -n 120 "$HOME/vj-results/system.log" 2>/dev/null || true
tail -n 80 "$HOME/vj-results/watchdog.log" 2>/dev/null || true
echo
echo "== END DOCTOR"

