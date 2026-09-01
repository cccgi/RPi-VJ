#!/usr/bin/env bash
# One-shot Pi 5 kiosk fix for Pi OS Lite (no desktop installed).
# Run:  ssh rpi@rpi-vj.local 'sudo bash -s' < gig/deploy/fix-kiosk.sh
set -euo pipefail

VJ_USER="${SUDO_USER:-rpi}"
VJ_UID="$(id -u "$VJ_USER")"
REPO="/home/$VJ_USER/RPi-VJ"
CHROME="$(command -v chromium || command -v chromium-browser)"

echo "== user=$VJ_USER uid=$VJ_UID repo=$REPO chromium=$CHROME"
echo "== arch: $(dpkg --print-architecture)  kernel: $(uname -m)"

# ---------------------------------------------------------------- 1. packages
apt-get update -qq
apt-get install -y cage seatd unclutter curl

# ------------------------------------------------------- 2. seat + gpu access
systemctl enable --now seatd
usermod -aG video,render,input,tty,_seatd,plugdev "$VJ_USER"

# --------------------------------------------------------------- 3. http unit
cat > /etc/systemd/system/vj-server.service <<EOF
[Unit]
Description=RPi VJ Static HTTP Server
After=network.target

[Service]
Type=simple
User=$VJ_USER
WorkingDirectory=$REPO
ExecStart=/usr/bin/python3 -m http.server 8080 --bind 127.0.0.1
Restart=always
RestartSec=2

[Install]
WantedBy=multi-user.target
EOF

# -------------------------------------------------------------- 4. kiosk unit
# PAMName=login + TTYPath is what gives cage a real logind seat.
# Without those two lines you get "Failed to start a DRM session".
cat > /etc/systemd/system/vj-kiosk.service <<EOF
[Unit]
Description=RPi VJ Kiosk (cage + chromium)
After=vj-server.service systemd-user-sessions.service seatd.service
Wants=vj-server.service
Conflicts=getty@tty1.service
After=getty@tty1.service

[Service]
Type=simple
User=$VJ_USER
PAMName=login
TTYPath=/dev/tty1
TTYReset=yes
TTYVHangup=yes
TTYVTDisallocate=yes
StandardInput=tty-fail
StandardOutput=journal
StandardError=journal
Environment=XDG_RUNTIME_DIR=/run/user/$VJ_UID
Environment=XDG_SESSION_TYPE=wayland
Environment=XDG_SEAT=seat0
Environment=WLR_RENDERER=gles2
Environment=WLR_NO_HARDWARE_CURSORS=1
ExecStartPre=/bin/sh -c 'until curl -sf http://127.0.0.1:8080/gig/ >/dev/null; do sleep 0.5; done'
ExecStart=/usr/bin/cage -- $CHROME \\
  --ozone-platform=wayland \\
  --enable-features=UseOzonePlatform \\
  --kiosk --app=http://localhost:8080/gig/ \\
  --autoplay-policy=no-user-gesture-required \\
  --enable-gpu-rasterization --ignore-gpu-blocklist \\
  --disable-session-crashed-bubble --disable-infobars --noerrdialogs \\
  --check-for-update-interval=31536000 \\
  --disable-pinch --overscroll-history-navigation=0
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF

# ------------------------------------------------------------- 5. stream deck
cat > /etc/udev/rules.d/60-streamdeck.rules <<'EOF'
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="006c", MODE="0660", GROUP="plugdev", TAG+="uaccess"
SUBSYSTEM=="hidraw", ATTRS{idVendor}=="0fd9", ATTRS{idProduct}=="008f", MODE="0660", GROUP="plugdev", TAG+="uaccess"
SUBSYSTEM=="usb", ATTRS{idVendor}=="0fd9", MODE="0660", GROUP="plugdev", TAG+="uaccess"
EOF
udevadm control --reload-rules && udevadm trigger

# ------------------------------------------------------------------ 6. hdmi
CFG=/boot/firmware/config.txt
[ -f "$CFG" ] || CFG=/boot/config.txt
if ! grep -q '^hdmi_group=1' "$CFG"; then
  printf '\n# RPi-VJ: force 1080p60 rather than trusting venue EDID\nhdmi_group=1\nhdmi_mode=16\nhdmi_drive=2\nhdmi_force_hotplug=1\ndisable_overscan=1\n' >> "$CFG"
  echo "== appended HDMI mode to $CFG (needs reboot)"
fi

# ------------------------------------------------------------------ 7. go
systemctl daemon-reload
systemctl enable vj-server.service vj-kiosk.service
systemctl restart vj-server.service
sleep 1
systemctl restart vj-kiosk.service
sleep 4
systemctl --no-pager --lines=15 status vj-kiosk.service || true
echo
echo "== if it is running, the TV should now show the show page."
echo "== group changes need a fresh login; reboot once when convenient."
