#!/usr/bin/env bash
# scripts/appliance/capture-baseline.sh
# Freeze baseline system state and configuration of the Raspberry Pi appliance.
# Non-destructive: strictly reads from the system and writes to an output directory.
# Automatically redacts passwords, private keys, Wi-Fi PSKs, and samba secrets.

set -euo pipefail

export PATH="/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:${PATH}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
DEFAULT_OUT_DIR="${PWD}/baseline-${TIMESTAMP}"
OUT_DIR="${1:-${DEFAULT_OUT_DIR}}"

# Display usage
if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
    echo "Usage: $0 [OUTPUT_DIRECTORY]"
    echo ""
    echo "Captures a non-destructive snapshot of OS, kernel, packages, boot config,"
    echo "systemd units, mounts, block devices, udev rules, USB IDs, DRM connectors,"
    echo "network configuration, application checksums, and recent system logs."
    echo "All secrets (Wi-Fi PSKs, passwords, private keys) are automatically redacted."
    echo ""
    echo "Default output directory: ./baseline-<YYYYMMDD_HHMMSS>"
    exit 0
fi

echo "==> Creating baseline capture directory: ${OUT_DIR}"
mkdir -p "${OUT_DIR}"/{system,boot,systemd/custom-units,storage,hardware/udev-rules,hardware/drm-status,network,checksums,logs}

# Helper: Redact secrets in text stream
redact_stream() {
    sed -E \
        -e 's/(psk|password|wpa_passphrase|secret|token|api_key|auth|credential|credentials)[[:space:]]*=[[:space:]]*[^ \t\r\n;]+/ \1=[REDACTED]/Ig' \
        -e 's/(psk|password|wpa_passphrase|secret|token|api_key|auth):[[:space:]]*[^ \t\r\n;]+/ \1: [REDACTED]/Ig' \
        -e 's/([A-Za-z0-9_]*password[A-Za-z0-9_]*=)[^, \t\r\n;]+/\1[REDACTED]/Ig' \
        -e 's/(password=)[^, \t\r\n;]+/\1[REDACTED]/Ig'
}

# Helper: Run command and save output safely (does not fail if optional tool is missing)
capture_cmd() {
    local target_file="$1"
    shift
    local cmd_name="$1"

    if command -v "$cmd_name" >/dev/null 2>&1; then
        echo "--> Capturing: $*"
        if ! "$@" 2>&1 | redact_stream > "${OUT_DIR}/${target_file}"; then
            echo "[WARN] Command exited with non-zero status: $*" >> "${OUT_DIR}/${target_file}"
        fi
    else
        echo "--> Skipping (not found): $cmd_name"
        echo "[NOT FOUND] Command '${cmd_name}' is not installed." > "${OUT_DIR}/${target_file}"
    fi
}

# Helper: Copy text file with redaction and private key suppression
safe_copy_file() {
    local src="$1"
    local dst="$2"

    if [[ ! -f "$src" ]]; then
        return 0
    fi

    if [[ ! -r "$src" ]]; then
        echo "[UNREADABLE (PERMISSION DENIED): ${src}]" > "$dst" || true
        return 0
    fi

    # Check for private keys
    if grep -qE -- "BEGIN (RSA |EC |DSA |OPENSSH )?PRIVATE KEY" "$src" 2>/dev/null; then
        echo "[REDACTED PRIVATE KEY FILE: ${src}]" > "$dst"
        return 0
    fi

    # Redact sensitive key/value pairs
    if ! redact_stream < "$src" > "$dst" 2>/dev/null; then
        cp "$src" "$dst" 2>/dev/null || echo "[READ ERROR: ${src}]" > "$dst" || true
    fi
}

# Helper: Safe directory copy with recursion and redaction
safe_copy_dir() {
    local src_dir="$1"
    local dst_dir="$2"

    if [[ ! -d "$src_dir" ]]; then
        return 0
    fi

    mkdir -p "$dst_dir"
    find "$src_dir" -maxdepth 2 -type f 2>/dev/null | while IFS= read -r file; do
        if [[ -n "$file" ]]; then
            local rel_path="${file#"$src_dir"/}"
            local target_path="${dst_dir}/${rel_path}"
            mkdir -p "$(dirname "$target_path")"
            safe_copy_file "$file" "$target_path" || true
        fi
    done || true
}

echo "--- 1/9: Operating System, Kernel & Package Versions ---"
capture_cmd "system/uname.txt" uname -a
capture_cmd "system/hostnamectl.txt" hostnamectl
capture_cmd "system/os-release" cat /etc/os-release
capture_cmd "system/debian_version" cat /etc/debian_version
capture_cmd "system/rpi-issue" cat /etc/rpi-issue
capture_cmd "system/cpuinfo.txt" cat /proc/cpuinfo
capture_cmd "system/dpkg-installed.txt" dpkg-query -W -f='${Package}\t${Version}\t${Architecture}\t${Status}\n'
capture_cmd "system/environment.txt" cat /etc/environment

echo "--- 2/9: Boot Configuration & Firmware ---"
safe_copy_file "/boot/firmware/config.txt" "${OUT_DIR}/boot/config.txt"
if [[ ! -f "${OUT_DIR}/boot/config.txt" ]]; then
    safe_copy_file "/boot/config.txt" "${OUT_DIR}/boot/config.txt"
fi
safe_copy_file "/boot/firmware/cmdline.txt" "${OUT_DIR}/boot/cmdline.txt"
if [[ ! -f "${OUT_DIR}/boot/cmdline.txt" ]]; then
    safe_copy_file "/boot/cmdline.txt" "${OUT_DIR}/boot/cmdline.txt"
fi
capture_cmd "boot/vcgencmd-version.txt" vcgencmd version
capture_cmd "boot/vcgencmd-config.txt" vcgencmd get_config int
capture_cmd "boot/device-tree-model.txt" cat /proc/device-tree/model

echo "--- 3/9: Systemd Units & Service Definitions ---"
capture_cmd "systemd/unit-files.txt" systemctl list-unit-files --no-pager
capture_cmd "systemd/active-units.txt" systemctl list-units --all --no-pager
capture_cmd "systemd/running-services.txt" systemctl list-units --type=service --state=running --no-pager
capture_cmd "systemd/timers.txt" systemctl list-timers --all --no-pager

# Copy custom systemd units from /etc/systemd/system
safe_copy_dir "/etc/systemd/system" "${OUT_DIR}/systemd/custom-units"

echo "--- 4/9: Storage, Mounts & Block Devices ---"
capture_cmd "storage/mounts.txt" mount
capture_cmd "storage/proc-mounts.txt" cat /proc/mounts
safe_copy_file "/etc/fstab" "${OUT_DIR}/storage/fstab.txt"
capture_cmd "storage/lsblk.txt" lsblk -o NAME,MAJ:MIN,RM,SIZE,RO,TYPE,MOUNTPOINTS,FSTYPE,LABEL,UUID,PARTUUID
capture_cmd "storage/blkid.txt" blkid
capture_cmd "storage/df-hT.txt" df -hT

echo "--- 5/9: Udev Rules & Hardware Identities ---"
safe_copy_dir "/etc/udev/rules.d" "${OUT_DIR}/hardware/udev-rules"
capture_cmd "hardware/lsusb.txt" lsusb
capture_cmd "hardware/lsusb-tree.txt" lsusb -t
capture_cmd "hardware/usb-devices.txt" usb-devices

echo "--- 6/9: DRM Connectors & Display Status ---"
if [[ -d "/sys/class/drm" ]]; then
    for connector_path in /sys/class/drm/card*-*; do
        if [[ -d "$connector_path" ]]; then
            connector_name="$(basename "$connector_path")"
            conn_dst="${OUT_DIR}/hardware/drm-status/${connector_name}"
            mkdir -p "$conn_dst"
            for attr in status enabled dpms modes; do
                if [[ -f "${connector_path}/${attr}" ]]; then
                    cp "${connector_path}/${attr}" "${conn_dst}/${attr}" 2>/dev/null || true
                fi
            done
        fi
    done
fi
capture_cmd "hardware/modetest.txt" modetest -c -m
capture_cmd "hardware/drm_info.txt" drm_info

echo "--- 7/9: Network Configuration (Sanitized) ---"
capture_cmd "network/ip-addr.txt" ip addr
capture_cmd "network/ip-route.txt" ip route
capture_cmd "network/ip-link.txt" ip -s link
safe_copy_file "/etc/resolv.conf" "${OUT_DIR}/network/resolv.conf"
safe_copy_file "/etc/hosts" "${OUT_DIR}/network/hosts"
if [[ -d "/etc/NetworkManager/system-connections" ]]; then
    safe_copy_dir "/etc/NetworkManager/system-connections" "${OUT_DIR}/network/network-manager"
fi
if [[ -f "/etc/network/interfaces" ]]; then
    safe_copy_file "/etc/network/interfaces" "${OUT_DIR}/network/interfaces"
fi

echo "--- 8/9: Application & Configuration Checksums ---"
{
    echo "# Baseline checksums generated at ${TIMESTAMP}"
    echo "# Target: /opt/rpi-vj"
    if [[ -d "/opt/rpi-vj" ]]; then
        find /opt/rpi-vj -type f -exec sha256sum {} + 2>/dev/null || true
    else
        echo "# /opt/rpi-vj does not exist"
    fi

    echo "# Target: System configurations"
    for cfg in /etc/pipewire/pipewire.conf.d/90-rpi-vj.conf \
               /etc/systemd/system/rpi-vj.service \
               /etc/udev/rules.d/70-rpi-vj.rules; do
        if [[ -f "$cfg" ]]; then
            sha256sum "$cfg" 2>/dev/null || true
        fi
    done

    echo "# Target: System multimedia and compositor binaries"
    for bin in /usr/bin/mpv /usr/bin/pipewire /usr/bin/wireplumber /usr/bin/pw-cat /usr/bin/wayfire /usr/bin/labwc; do
        if command -v "$bin" >/dev/null 2>&1; then
            sha256sum "$bin" 2>/dev/null || true
        fi
    done
} > "${OUT_DIR}/checksums/application-checksums.sha256"

echo "--- 9/9: Recent System & Kernel Logs ---"
capture_cmd "logs/journalctl-recent.log" journalctl -b -n 500 --no-pager
capture_cmd "logs/dmesg.log" dmesg -T

# Generate final SHA-256 manifest of all captured artifacts
echo "==> Generating manifest checksums..."
(
    cd "${OUT_DIR}"
    find . -type f ! -name "MANIFEST.sha256" -exec sha256sum {} + | sort -k 2 > MANIFEST.sha256
)

echo "==> Baseline capture complete: ${OUT_DIR}"
echo "    Artifact count: $(find "${OUT_DIR}" -type f | wc -l)"
echo "    Manifest: ${OUT_DIR}/MANIFEST.sha256"
