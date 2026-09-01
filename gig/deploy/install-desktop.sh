#!/usr/bin/env bash
# RPi-VJ deployment for Raspberry Pi OS 64-bit WITH DESKTOP.
# Run ON THE PI, as user rpi, WITHOUT sudo:
#     cd ~/RPi-VJ && ./gig/deploy/install-desktop.sh
# Safe to run over ssh. Safe to re-run.
set -euo pipefail

# ---- guard: must NOT be root (systemctl --user needs a real user session)
if [ "$(id -u)" -eq 0 ]; then
  echo "!! Do not run this with sudo. Run it as the rpi user:"
  echo "!!     cd ~/RPi-VJ && ./gig/deploy/install-desktop.sh"
  echo "!! (it calls sudo itself for the few steps that need it)"
  exit 1
fi

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
UID_N="$(id -u)"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$UID_N}"
CHROME="$(command -v chromium || command -v chromium-browser || true)"
[ -n "$CHROME" ] || { echo "!! chromium not found — is this the Desktop image?"; exit 1; }

echo "== repo=$REPO user=$USER uid=$UID_N chromium=$CHROME"
echo "== arch=$(dpkg --print-architecture) kernel=$(uname -m)"

# ---- detect a real desktop session (NOT this shell's env — ssh has none)
COMPOSITOR="$(pgrep -u "$USER" -x 'labwc|wayfire|Xorg|weston|mutter' 2>/dev/null | head -1 || true)"
GRAPHICAL="$(loginctl list-sessions --no-legend 2>/dev/null | awk -v u="$USER" '$3==u{print $1}' | while read -r s; do
  loginctl show-session "$s" -p Type --value 2>/dev/null; done | grep -E 'wayland|x11' | head -1 || true)"
if [ -n "$COMPOSITOR" ] || [ -n "$GRAPHICAL" ]; then
  echo "== desktop session: RUNNING (${GRAPHICAL:-compositor pid $COMPOSITOR})"
else
  echo "== desktop session: NOT RUNNING — enabling autologin; it will start after reboot"
  sudo raspi-config nonint do_boot_behaviour B4 || true
fi

# ---------------------------------------------------------------- packages
sudo apt-get update -qq
sudo apt-get install -y unclutter curl python3

# ------------------------------------------------------------ screen blanking
sudo raspi-config nonint do_blanking 1 || true

# --------------------------------------------------------------- stream deck
sudo tee /etc/udev/rules.d/60-streamdeck.rules >/dev/null <<'EOF'
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="006c", MODE="0660", GROUP="plugdev", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="008f", MODE="0660", GROUP="plugdev", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="0fd9", MODE="0660", GROUP="plugdev", TAG+="uaccess"
EOF
sudo udevadm control --reload-rules && sudo udevadm trigger
sudo usermod -aG plugdev,video,render,input,audio "$USER"

# ---------------------------------------------------------------------- hdmi
CFG=/boot/firmware/config.txt; [ -f "$CFG" ] || CFG=/boot/config.txt
grep -q '^hdmi_group=1' "$CFG" || printf '\n# RPi-VJ: force 1080p60 instead of trusting venue EDID\nhdmi_group=1\nhdmi_mode=16\nhdmi_drive=2\nhdmi_force_hotplug=1\ndisable_overscan=1\n' | sudo tee -a "$CFG" >/dev/null

# ------------------------------------------------------------- user services
mkdir -p ~/.config/systemd/user

# (unit body removed -- install-units.sh owns this now)

# (unit body removed -- install-units.sh owns this now)

sudo loginctl enable-linger "$USER"
systemctl --user daemon-reload
systemctl --user enable vj-server.service vj-kiosk.service
systemctl --user restart vj-server.service || true

if [ -n "$COMPOSITOR$GRAPHICAL" ]; then
  systemctl --user restart vj-kiosk.service || true
  sleep 4
  systemctl --user --no-pager --lines=12 status vj-kiosk.service || true
else
  echo "== kiosk enabled but not started (no desktop yet). Reboot to bring it up."
fi

cat <<EOF

== done. Reboot once:  sudo reboot
   logs:   journalctl --user -u vj-kiosk -f
   stop:   systemctl --user stop vj-kiosk
   probe:  cd ~/RPi-VJ && ./gig/deploy/run-probe.sh probe.html
EOF

# ---------------------------------------------------------------------------
# UNITS ARE NOT WRITTEN HERE ANY MORE.
# This installer used to emit its own vj-server.service using
# `python3 -m http.server`, which serves the page but removes the watchdog,
# /heartbeat, /system/shutdown and all telemetry. Re-running it after a fix
# silently undid the fix. One script owns the units now.
# ---------------------------------------------------------------------------
echo
echo "== installing canonical units"
"$(dirname "$0")/install-units.sh"
