#!/bin/sh
BROWSER=chromium-browser
command -v "$BROWSER" >/dev/null 2>&1 || BROWSER=chromium
exec "$BROWSER" --kiosk --enable-features=VaapiVideoDecoder \
  --ignore-gpu-blocklist --enable-gpu-rasterization \
  --autoplay-policy=no-user-gesture-required \
  "file://$(cd "$(dirname "$0")" && pwd)/probe.html"
