#!/usr/bin/env bash
# Make the kiosk start automatically after the desktop session comes up.
# Run ON THE PI as rpi, no sudo:   ./gig/deploy/fix-autostart.sh
set -euo pipefail
[ "$(id -u)" -ne 0 ] || { echo "!! run as rpi, not root"; exit 1; }
REPO="$(cd "$(dirname "$0")/../.." && pwd)"
LAUNCH="$REPO/gig/deploy/kiosk-autostart.sh"

# --- the launcher the desktop session calls -------------------------------
cat > "$LAUNCH" <<'EOF'
#!/usr/bin/env bash
# Hand the compositor's environment to the systemd user manager, then start
# the kiosk. Without import-environment, Chromium starts with no WAYLAND_DISPLAY.
sleep 2

# Force display resolution to 1080p60 on Wayland / X11
/usr/bin/wlr-randr --output HDMI-A-1 --mode 1920x1080@60 2>/dev/null || \
/usr/bin/wlr-randr --output HDMI-A-2 --mode 1920x1080@60 2>/dev/null || \
/usr/bin/xrandr --output HDMI-1 --mode 1920x1080 2>/dev/null || true

# Hide mouse cursor completely on desktop
/usr/bin/unclutter -idle 0.1 -root 2>/dev/null &

# Move mouse cursor offscreen / to bottom-right corner (1920,1080)
/usr/bin/wlrctl pointer move 1920 1080 2>/dev/null || /usr/bin/xdotool mousemove 1920 1080 2>/dev/null || true

sleep 1

systemctl --user import-environment \
  WAYLAND_DISPLAY XDG_RUNTIME_DIR DISPLAY XDG_SESSION_TYPE XDG_CURRENT_DESKTOP 2>/dev/null || true
systemctl --user start vj-server.service 2>/dev/null || true
systemctl --user restart vj-kiosk.service 2>/dev/null || true
EOF
chmod +x "$LAUNCH"

# --- XDG autostart (works on most sessions) -------------------------------
mkdir -p ~/.config/autostart
cat > ~/.config/autostart/vj-kiosk.desktop <<EOF
[Desktop Entry]
Type=Application
Name=RPi-VJ Kiosk
Exec=$LAUNCH
NoDisplay=true
X-GNOME-Autostart-enabled=true
EOF

# --- belt and braces: compositor-native autostart --------------------------
mkdir -p ~/.config/labwc
touch ~/.config/labwc/autostart
grep -qF "$LAUNCH" ~/.config/labwc/autostart || echo "$LAUNCH &" >> ~/.config/labwc/autostart
chmod +x ~/.config/labwc/autostart 2>/dev/null || true

if [ -f ~/.config/wayfire.ini ] && ! grep -q 'vj_kiosk' ~/.config/wayfire.ini; then
  printf '\n[autostart]\nvj_kiosk = %s\n' "$LAUNCH" >> ~/.config/wayfire.ini
fi

# --- stop systemd from launching it too early -----------------------------
# graphical-session.target is never activated by labwc, and default.target
# would fire before a compositor exists. The desktop session owns startup now.
systemctl --user disable vj-kiosk.service 2>/dev/null || true
sudo loginctl disable-linger "$USER" 2>/dev/null || true
systemctl --user enable vj-server.service 2>/dev/null || true
systemctl --user daemon-reload

echo "== autostart wired via:"
echo "   ~/.config/autostart/vj-kiosk.desktop"
echo "   ~/.config/labwc/autostart"
echo "== testing now (kiosk should appear on the TV in ~5s)..."
"$LAUNCH"
sleep 5
systemctl --user --no-pager --lines=8 status vj-kiosk.service || true
echo
echo "== if that looked right:  sudo reboot   and confirm it comes up alone."
