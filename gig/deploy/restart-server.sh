#!/usr/bin/env bash
# The HTTP server runs as a user service and does NOT restart with the kiosk.
# After changing collect_server.py you must restart it explicitly.
set -euo pipefail
systemctl --user restart vj-server.service
sleep 1
systemctl --user --no-pager --lines=4 status vj-server.service || true
