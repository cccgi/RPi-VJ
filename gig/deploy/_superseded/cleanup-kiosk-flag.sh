#!/usr/bin/env bash
# Permanently lock Raspberry Pi OS Desktop, Kernel DRM KMS, Wayland, and Kiosk to 1080p (1920x1080@60Hz).
# Run ON THE PI as user rpi:
#   chmod +x ./gig/deploy/lock-1080p.sh && ./gig/deploy/lock-1080p.sh
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }

echo "== [1/5] Locking Linux Kernel KMS DRM video mode in cmdline.txt..."
CMDLINE=/boot/firmware/cmdline.txt
[ -f "$CMDLINE" ] || CMDLINE=/boot/cmdline.txt
if [ -f "$CMDLINE" ]; then
  sudo sed -i 's/video=[^ ]*//g' "$CMDLINE"
  sudo sed -i 's/$/ video=HDMI-A-1:1920x1080@60D video=HDMI-A-2:1920x1080@60D/' "$CMDLINE"
  echo "   Kernel video mode locked."
fi

echo "== [2/5] Disabling 4Kp60 in boot config.txt and forcing 1080p..."
CFG=/boot/firmware/config.txt
[ -f "$CFG" ] || CFG=/boot/config.txt
if [ -f "$CFG" ]; then
  sudo sed -i '/hdmi_enable_4kp60/d' "$CFG"
  sudo sed -i '/hdmi_group/d' "$CFG"
  sudo sed -i '/hdmi_mode/d' "$CFG"
  cat << 'EOF' | sudo tee -a "$CFG" >/dev/null

# RPi-VJ: Lock to 1080p60
hdmi_enable_4kp60=0
hdmi_group=1
hdmi_mode=16
hdmi_drive=2
hdmi_force_hotplug=1
EOF
  echo "   config.txt locked."
fi

echo "== [3/5] Locking Wayland / Wayfire / Labwc desktop compositor settings..."
mkdir -p ~/.config/wayfire ~/.config/labwc ~/.config/kanshi

WF_INI=~/.config/wayfire.ini
if [ -f "$WF_INI" ]; then
  sed -i '/\[output:HDMI-A-1\]/,/^$/d' "$WF_INI" || true
  sed -i '/\[output:HDMI-A-2\]/,/^$/d' "$WF_INI" || true
  cat >> "$WF_INI" <<'EOF'

[output:HDMI-A-1]
mode = 1920x1080@60000
position = 0,0
transform = normal

[output:HDMI-A-2]
mode = 1920x1080@60000
position = 0,0
transform = normal
EOF
fi

cat > ~/.config/kanshi/config <<'EOF'
profile default {
    output HDMI-A-1 mode 1920x1080@60Hz position 0,0
    output HDMI-A-2 mode 1920x1080@60Hz position 0,0
}
EOF

echo "== [4/5] Updating desktop autostart launcher with resolution switch..."
./gig/deploy/fix-autostart.sh

echo "== [5/5] Patching kiosk unit..."
U=~/.config/systemd/user/vj-kiosk.service
if [ -f "$U" ]; then
  grep -q 'wlr-randr' "$U" || sed -i '/ExecStart=/i ExecStartPre=-/bin/sh -c "/usr/bin/wlr-randr --output HDMI-A-1 --mode 1920x1080@60 2>/dev/null || /usr/bin/wlr-randr --output HDMI-A-2 --mode 1920x1080@60 2>/dev/null || /usr/bin/xrandr --output HDMI-1 --mode 1920x1080 2>/dev/null || true"' "$U"
  systemctl --user daemon-reload || true
  systemctl --user restart vj-kiosk.service || true
fi

# Apply immediately to current live session
/usr/bin/wlr-randr --output HDMI-A-1 --mode 1920x1080@60 2>/dev/null || \
/usr/bin/wlr-randr --output HDMI-A-2 --mode 1920x1080@60 2>/dev/null || \
/usr/bin/xrandr --output HDMI-1 --mode 1920x1080 2>/dev/null || true

echo "== 1080p resolution lock complete on all layers!"
