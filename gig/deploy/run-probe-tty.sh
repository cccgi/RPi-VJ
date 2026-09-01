#!/usr/bin/env bash
# Run a probe page fullscreen on the TV, from an SSH session.
# Stops the kiosk, runs cage on tty1, restores the kiosk on exit.
# Usage:  ./run-probe-tty.sh probe.html     (or codec.html, shader-bench.html)
set -euo pipefail
PAGE="${1:-probe.html}"
CHROME="$(command -v chromium || command -v chromium-browser)"
sudo systemctl stop vj-kiosk.service 2>/dev/null || true
sudo systemctl start vj-server.service
until curl -sf "http://127.0.0.1:8080/gig/probe/$PAGE" >/dev/null; do sleep 0.5; done
echo "== launching $PAGE on tty1 — watch the TV. Ctrl-C here when done."
sudo systemd-run --pty --same-dir --wait --collect \
  --unit=vj-probe \
  -p PAMName=login -p TTYPath=/dev/tty1 -p StandardInput=tty-fail \
  -p TTYReset=yes -p TTYVHangup=yes \
  -p User="$USER" \
  -p Environment=XDG_RUNTIME_DIR=/run/user/$(id -u) \
  -p Environment=WLR_RENDERER=gles2 \
  -p Environment=WLR_NO_HARDWARE_CURSORS=1 \
  /usr/bin/cage -- "$CHROME" \
    --ozone-platform=wayland --enable-features=UseOzonePlatform \
    --user-data-dir="$HOME/.cache/vj-probe-profile" \
    --kiosk --autoplay-policy=no-user-gesture-required \
    --enable-gpu-rasterization --ignore-gpu-blocklist \
    "http://localhost:8080/gig/probe/$PAGE"
sudo systemctl start vj-kiosk.service || true
