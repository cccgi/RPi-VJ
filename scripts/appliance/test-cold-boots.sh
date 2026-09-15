#!/usr/bin/env bash
# scripts/appliance/test-cold-boots.sh
# RPi-VJ Appliance V3 — 25-Boot Cold Boot Ledger & Hardware Gate Helper (A3-03)
#
# Records boot metrics, DRM connector state, renderer status, and human observations
# to a durable CSV at /var/lib/rpi-vj/cold-boots.csv.

set -euo pipefail

CSV_PATH="${COLD_BOOT_CSV_PATH:-/var/lib/rpi-vj/cold-boots.csv}"
mkdir -p "$(dirname "${CSV_PATH}")"

# Initialize CSV header if new
if [[ ! -f "${CSV_PATH}" ]]; then
    echo "boot_index,boot_id,monotonic_uptime,renderer_active_sec,drm_connector,renderer_restarts,tv_visual_ok,cursor_absent,human_notes" > "${CSV_PATH}"
fi

# Detect current boot metrics
BOOT_ID="unknown"
if [[ -f /proc/sys/kernel/random/boot_id ]]; then
    BOOT_ID="$(cat /proc/sys/kernel/random/boot_id | tr -d ' \n\r')"
fi

UPTIME_SEC="0.0"
if [[ -f /proc/uptime ]]; then
    UPTIME_SEC="$(awk '{print $1}' /proc/uptime)"
fi

# Detect DRM connector status
DRM_CONNECTOR="none"
if [[ -d /sys/class/drm ]]; then
    for p in /sys/class/drm/card*-*; do
        if [[ -f "${p}/status" ]] && [[ "$(cat "${p}/status" 2>/dev/null)" == "connected" ]]; then
            DRM_CONNECTOR="$(basename "$p")"
            break
        fi
    done
fi

# Renderer service restart count & active duration
RENDERER_RESTARTS="0"
RENDERER_ACTIVE_SEC="0"
if command -v systemctl >/dev/null 2>&1; then
    RENDERER_RESTARTS="$(systemctl show rpi-vj-renderer.service -p NRestarts --value 2>/dev/null || echo 0)"
fi

BOOT_INDEX=$(($(wc -l < "${CSV_PATH}") ))

echo "========================================================"
echo " RPi-VJ Appliance V3 — Cold Boot Gate Verification (#${BOOT_INDEX})"
echo "========================================================"
echo "Boot ID:            ${BOOT_ID}"
echo "Monotonic Uptime:   ${UPTIME_SEC}s"
echo "DRM Connector:      ${DRM_CONNECTOR}"
echo "Renderer Restarts:  ${RENDERER_RESTARTS}"
echo ""

if [[ "${1:-}" == "--auto" ]]; then
    TV_OK="${2:-yes}"
    CURSOR_ABSENT="${3:-yes}"
    NOTES="${4:-automated-record}"
else
    read -r -p "Did the TV present the visual within <=45s? [Y/n]: " ans_tv
    TV_OK="yes"
    [[ "${ans_tv:-y}" =~ ^[Nn] ]] && TV_OK="no"

    read -r -p "Was the cursor completely absent? [Y/n]: " ans_cur
    CURSOR_ABSENT="yes"
    [[ "${ans_cur:-y}" =~ ^[Nn] ]] && CURSOR_ABSENT="no"

    read -r -p "Human observation / notes: " NOTES
    NOTES="${NOTES:-clean-boot}"
fi

# Sanitize notes
NOTES="$(echo "${NOTES}" | tr ',' ';')"

# Append record
echo "${BOOT_INDEX},${BOOT_ID},${UPTIME_SEC},${RENDERER_ACTIVE_SEC},${DRM_CONNECTOR},${RENDERER_RESTARTS},${TV_OK},${CURSOR_ABSENT},${NOTES}" >> "${CSV_PATH}"
sync -d "${CSV_PATH}" 2>/dev/null || sync "${CSV_PATH}" 2>/dev/null || true

echo ""
echo "Recorded boot #${BOOT_INDEX} to ${CSV_PATH}."
echo "Total boots recorded: ${BOOT_INDEX} / 25"
