#!/usr/bin/env bash
# gig/deploy/install.sh — Idempotent installer for RPi VJ kiosk autostart & system configuration
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"
TARGET_USER="${SUDO_USER:-$USER}"

echo "=== RPi-VJ Kiosk Installer ==="
echo "Target User: ${TARGET_USER}"
echo "Repo Root:   ${REPO_ROOT}"

if [ "$(id -u)" -ne 0 ]; then
  echo "Error: install.sh must be run with sudo/root privileges."
  echo "Usage: sudo ./gig/deploy/install.sh"
  exit 1
fi

NEEDS_REBOOT=0

# 1. Install dependencies
echo "--> Checking/Installing runtime packages..."
apt-get update -qq || true
apt-get install -y -qq chromium-browser python3 unclutter x11-xserver-utils curl iw libinput-tools 2>/dev/null || true

# 2. Configure user groups (plugdev for WebHID / hidraw access)
echo "--> Adding ${TARGET_USER} to plugdev and video groups..."
usermod -a -G plugdev,video "${TARGET_USER}" || true

# 3. Copy udev rules
echo "--> Installing udev rules..."
UDEV_SRC="${SCRIPT_DIR}/60-streamdeck.rules"
if [ ! -f "${UDEV_SRC}" ] && [ -f "${REPO_ROOT}/config/udev/70-rpi-vj.rules" ]; then
  UDEV_SRC="${REPO_ROOT}/config/udev/70-rpi-vj.rules"
fi
cp "${UDEV_SRC}" /etc/udev/rules.d/60-streamdeck.rules
udevadm control --reload-rules && udevadm trigger || true

# 4. Configure HDMI output in config.txt
echo "--> Configuring HDMI display output (1080p@60)..."
BOOT_CONFIG="/boot/firmware/config.txt"
if [ ! -f "${BOOT_CONFIG}" ]; then
  BOOT_CONFIG="/boot/config.txt"
fi

if [ -f "${BOOT_CONFIG}" ]; then
  # Ensure HDMI force hotplug and 1080p60 mode settings
  if ! grep -q "hdmi_force_hotplug=1" "${BOOT_CONFIG}"; then
    echo "hdmi_force_hotplug=1" >> "${BOOT_CONFIG}"
    NEEDS_REBOOT=1
  fi
  if ! grep -q "hdmi_group=1" "${BOOT_CONFIG}"; then
    echo "hdmi_group=1" >> "${BOOT_CONFIG}"
    NEEDS_REBOOT=1
  fi
  if ! grep -q "hdmi_mode=16" "${BOOT_CONFIG}"; then
    echo "hdmi_mode=16" >> "${BOOT_CONFIG}"
    NEEDS_REBOOT=1
  fi
fi

# 5. Disable Wi-Fi power saving
echo "--> Disabling Wi-Fi power management..."
if command -v iw >/dev/null 2>&1; then
  iw dev wlan0 set power_save off 2>/dev/null || true
fi
# Persistent via /etc/network/interfaces or rc.local hook if available
if [ -d /etc/network/interfaces.d ]; then
  echo -e "#!/bin/sh\niw dev wlan0 set power_save off 2>/dev/null || true\n" > /etc/network/if-up.d/rpi-vj-wifi-powersave || true
  chmod +x /etc/network/if-up.d/rpi-vj-wifi-powersave 2>/dev/null || true
fi

# 6. Install systemd services
echo "--> Installing and configuring systemd units..."
sed "s|%I|${TARGET_USER}|g; s|WorkingDirectory=/home/${TARGET_USER}/RPi-VJ|WorkingDirectory=${REPO_ROOT}|g" \
  "${SCRIPT_DIR}/vj-server.service" > /etc/systemd/system/vj-server.service

CHROMIUM_BIN="$(command -v chromium-browser || command -v chromium || echo /usr/bin/chromium)"

sed "s|%I|${TARGET_USER}|g; s|%U|$(id -u ${TARGET_USER})|g; s|/usr/bin/chromium|${CHROMIUM_BIN}|g" \
  "${SCRIPT_DIR}/vj-kiosk.service" > /etc/systemd/system/vj-kiosk.service

# 7. Configure X11 / Wayfire screen blanking & cursor hiding
echo "--> Configuring screen blanking & autostart overrides..."
mkdir -p "/home/${TARGET_USER}/.config/lxsession/LXDE-pi"
AUTOSTART_FILE="/home/${TARGET_USER}/.config/lxsession/LXDE-pi/autostart"
if [ ! -f "${AUTOSTART_FILE}" ] && [ -f "/etc/xdg/lxsession/LXDE-pi/autostart" ]; then
  cp "/etc/xdg/lxsession/LXDE-pi/autostart" "${AUTOSTART_FILE}"
fi

if [ -f "${AUTOSTART_FILE}" ]; then
  sed -i '/xset s /d; /xset -dpms/d; /unclutter/d' "${AUTOSTART_FILE}"
  echo "@xset s off" >> "${AUTOSTART_FILE}"
  echo "@xset -dpms" >> "${AUTOSTART_FILE}"
  echo "@xset s noblank" >> "${AUTOSTART_FILE}"
  echo "@unclutter -idle 0 -root" >> "${AUTOSTART_FILE}"
  chown -R "${TARGET_USER}:${TARGET_USER}" "/home/${TARGET_USER}/.config" || true
fi

# Wayfire/Labwc blanking inhibitor for Wayland desktop
WAYFIRE_CONFIG="/home/${TARGET_USER}/.config/wayfire.ini"
if [ -f "${WAYFIRE_CONFIG}" ]; then
  if ! grep -q "idle_inhibit" "${WAYFIRE_CONFIG}"; then
    echo -e "\n[idle]\ndpms_timeout = 0\nscreensaver_timeout = 0\n" >> "${WAYFIRE_CONFIG}"
    chown "${TARGET_USER}:${TARGET_USER}" "${WAYFIRE_CONFIG}" || true
  fi
fi

# 8. Reload systemd and enable services
echo "--> Enabling and starting services..."
systemctl daemon-reload
systemctl enable vj-server.service
systemctl enable vj-kiosk.service

systemctl restart vj-server.service || true
systemctl restart vj-kiosk.service || true

echo "=== Installation complete ==="
echo "Installed:"
echo " - /etc/udev/rules.d/60-streamdeck.rules"
echo " - /etc/systemd/system/vj-server.service (enabled)"
echo " - /etc/systemd/system/vj-kiosk.service (enabled)"
echo " - Configured screen blanking prevention & 1080p60 display setting"

if [ "${NEEDS_REBOOT}" -eq 1 ]; then
  echo "NOTE: Boot configuration (/boot/firmware/config.txt) was modified. A reboot is recommended."
else
  echo "Services are running. Test via cold boot."
fi
