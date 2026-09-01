#!/usr/bin/env bash
set -euo pipefail

echo "=========================================="
echo " RPi-VJ: Raspberry Pi 5 Appliance Setup  "
echo "=========================================="

if [ "$EUID" -ne 0 ]; then
    echo "Please run as root: sudo bash setup-pi.sh"
    exit 1
fi

echo "[1/5] Installing system dependencies..."
apt-get update
apt-get install -y \
    build-essential \
    pkg-config \
    ffmpeg \
    pipewire \
    pipewire-audio-client-libraries \
    libpipewire-0.3-dev \
    libspa-0.2-dev \
    libasound2-dev \
    libhidapi-dev \
    libudev-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-libav \
    usbmount

echo "[2/5] Installing udev rules..."
cp 70-rpi-vj.rules /etc/udev/rules.d/
udevadm control --reload-rules
udevadm trigger

echo "[3/5] Setting up service user permissions..."
usermod -aG audio,video,input,plugdev pi

echo "[4/5] Configuring systemd service..."
cp rpi-vj.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable rpi-vj.service

echo "[5/5] Creating show directory..."
mkdir -p /media/show
chown -R pi:pi /media/show

echo "=========================================="
echo " Setup complete! Ready for show deployment."
echo "=========================================="
