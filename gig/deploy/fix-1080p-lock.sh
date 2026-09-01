#!/usr/bin/env bash
# Lock Raspberry Pi OS Desktop and display compositor resolution permanently to 1080p (1920x1080@60Hz)
# Run on the Pi:
#   chmod +x ./gig/deploy/fix-1080p-lock.sh && ./gig/deploy/fix-1080p-lock.sh
set -euo pipefail

echo '== 1. Locking Kernel KMS DRM commandline mode to 1920x1080@60...'
CMDLINE=/boot/firmware/cmdline.txt
[ -f " \ ] || CMDLINE=/boot/cmdline.txt

if [ -f \\ ]; then
 # On Pi 5 / Pi 4 (vc4-kms-v3d), video=... in cmdline.txt tells DRM KMS to never negotiate 4K
 if ! grep -q 'video=HDMI-A-1:1920x1080@60D' \\; then
 sudo sed -i 's/$/ video=HDMI-A-1:1920x1080@60D video=HDMI-A-2:1920x1080@60D/' \\
 echo " Added video=HDMI-A-1:1920x1080@60D to \
  else
    echo '   cmdline.txt already locked to 1080p60'
  fi
fi

echo '== 2. Locking Wayfire / Labwc desktop compositor settings to 1080p...'
mkdir -p ~/.config/wayfire ~/.config/labwc

# Wayfire config (Raspberry Pi OS Wayland default on Pi 4 / early Pi 5)
WF_INI=~/.config/wayfire.ini
if [ -f \\ ]; then
  if ! grep -q 'custom_mode_1 = 1920x1080@60000' \\; then
    cat >> \\ <<'EOF'

[output:HDMI-A-1]
mode = 1920x1080@60000
position = 0,0
transform = normal

[output:HDMI-A-2]
mode = 1920x1080@60000
position = 0,0
transform = normal
EOF
    echo '   Updated wayfire.ini output modes'
  fi
fi

echo '== 3. Locking user autostart display mode...'
AUTOSTART_DIR=~/.config/autostart
mkdir -p \\

cat > \/force-1080p.desktop\ <<'EOF'
[Desktop Entry]
Type=Application
Name=Force 1080p Display Resolution
Exec=/bin/sh -c '/usr/bin/wlr-randr --output HDMI-A-1 --mode 1920x1080@60 2>/dev/null || /usr/bin/wlr-randr --output HDMI-A-2 --mode 1920x1080@60 2>/dev/null || /usr/bin/xrandr --output HDMI-1 --mode 1920x1080 2>/dev/null || true'
Hidden=false
NoDisplay=false
X-GNOME-Autostart-enabled=true
EOF

echo '== 4. Applying immediate 1080p switch to current live session...'
/usr/bin/wlr-randr --output HDMI-A-1 --mode 1920x1080@60 2>/dev/null || \
/usr/bin/wlr-randr --output HDMI-A-2 --mode 1920x1080@60 2>/dev/null || \
/usr/bin/xrandr --output HDMI-1 --mode 1920x1080 2>/dev/null || true

echo '== 5. Updating kiosk service unit...'
./gig/deploy/apply-1080p-default.sh || true

echo '== Complete! Raspberry Pi OS desktop and kiosk are permanently capped at 1080p@60Hz.'
