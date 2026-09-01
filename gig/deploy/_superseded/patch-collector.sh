#!/usr/bin/env bash
# Switch vj-server to the collector server (adds POST /collect). Run as rpi.
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
U=~/.config/systemd/user/vj-server.service
sed -i "s|ExecStart=.*|ExecStart=/usr/bin/python3 $REPO/gig/tools/collect_server.py|" "$U"
mkdir -p ~/vj-results
systemctl --user daemon-reload
systemctl --user restart vj-server.service
sleep 1
systemctl --user --no-pager --lines=5 status vj-server.service || true
