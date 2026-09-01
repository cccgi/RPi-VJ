#!/bin/sh
set -eu
test "$(id -u)" -eq 0 || { echo "run as root" >&2; exit 1; }
apt-get update
apt-get install -y build-essential cargo clang libclang-dev libdrm-dev libegl-dev libgbm-dev libhidapi-dev libpipewire-0.3-dev libudev-dev mpv pipewire pipewire-audio wireplumber
getent group plugdev >/dev/null || groupadd --system plugdev
id vj >/dev/null 2>&1 || useradd --system --create-home --groups audio,video,render,plugdev vj
install -Dm0644 config/udev/70-rpi-vj.rules /etc/udev/rules.d/70-rpi-vj.rules
install -Dm0644 config/pipewire/90-rpi-vj.conf /etc/pipewire/pipewire.conf.d/90-rpi-vj.conf
install -Dm0644 config/systemd/rpi-vj.service /etc/systemd/system/rpi-vj.service
cargo build --release
install -Dm0755 target/release/rpi-vj /opt/rpi-vj/bin/rpi-vj
cc -O3 -Wall -Wextra -Werror native/pw_capture.c -o /opt/rpi-vj/bin/pw_capture $(pkg-config --cflags --libs libpipewire-0.3) -lm
mkdir -p /opt/rpi-vj/clips
udevadm control --reload-rules
systemctl daemon-reload
echo "Copy clips to /opt/rpi-vj/clips, edit ExecStart, then: systemctl enable --now rpi-vj"
