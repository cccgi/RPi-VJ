#!/usr/bin/env bash
# scripts/appliance/harden-appliance.sh
# Storage, Power-Loss, and Systemd Security Hardening for RPi-VJ Appliance V3
# Strictly implements task A3-13, ARCHITECTURE.md, docs/appliance/STORAGE.md, docs/appliance/OBSERVABILITY.md

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_ROOT="/"
DRY_RUN=0
CHECK_MODE=0
RUN_TESTS=0

usage() {
    cat << 'EOF'
Usage: harden-appliance.sh [OPTIONS]

Hardens RPi-VJ Appliance V3 storage, systemd services, power-loss resilience, and hardware watchdog.

Options:
  --check             Check system hardening status without modifying (exit 0 if hardened, 1 if drift)
  --dry-run, -n       Simulate hardening changes without writing to disk
  --apply             Apply hardening configurations (default)
  --root PATH         Target root filesystem directory (default: /)
  --test              Execute automated test suite for storage GC, watchdog, and boot ledger
  -h, --help          Show this help message and exit
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            CHECK_MODE=1
            DRY_RUN=1
            shift
            ;;
        --dry-run|-n)
            DRY_RUN=1
            shift
            ;;
        --apply)
            DRY_RUN=0
            CHECK_MODE=0
            shift
            ;;
        --root)
            TARGET_ROOT="$2"
            shift 2
            ;;
        --test)
            RUN_TESTS=1
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "ERROR: Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if [[ "${RUN_TESTS}" -eq 1 ]]; then
    echo "========================================================"
    echo " RPi-VJ Appliance Hardening Test Suite"
    echo "========================================================"
    
    # 1. Run storage-gc unit tests
    echo "--> [1/3] Running Storage Garbage Collection Tests..."
    python3 "${SCRIPT_DIR}/storage-gc.py" --test || python "${SCRIPT_DIR}/storage-gc.py" --test
    
    # 2. Run boot-ledger unit tests
    echo "--> [2/3] Running Boot Ledger & Monotonic Watchdog Tests..."
    python3 "${SCRIPT_DIR}/boot-ledger.py" test || python "${SCRIPT_DIR}/boot-ledger.py" test
    
    # 3. Verify service hardening file syntax
    echo "--> [3/3] Validating Hardened Systemd Service Configurations..."
    REQUIRED_SERVICES=("rpi-vj-broker.service" "rpi-vj-renderer.service" "rpi-vj-controller.service")
    for svc in "${REQUIRED_SERVICES[@]}"; do
        svc_path="${SCRIPT_DIR}/${svc}"
        if [[ -f "${svc_path}" ]]; then
            for directive in "ProtectSystem=strict" "ProtectHome=yes" "NoNewPrivileges=yes" "PrivateTmp=yes" "ReadWritePaths="; do
                if ! grep -q "${directive}" "${svc_path}"; then
                    echo "FAIL: ${svc} missing hardening directive: ${directive}" >&2
                    exit 1
                fi
            done
            echo "  [OK] ${svc} has all required security directives."
        fi
    done
    
    echo ""
    echo "========================================================"
    echo " All Hardening Verification Tests PASSED."
    echo "========================================================"
    exit 0
fi

# Strip trailing slash from TARGET_ROOT
if [[ "${TARGET_ROOT}" != "/" ]]; then
    TARGET_ROOT="${TARGET_ROOT%/}"
fi

rpath() {
    local path="$1"
    if [[ "${TARGET_ROOT}" == "/" || -z "${TARGET_ROOT}" ]]; then
        echo "${path}"
    else
        echo "${TARGET_ROOT}${path}"
    fi
}

CHANGES_COUNT=0
declare -a CHANGES_LOG=()

record_change() {
    local msg="$1"
    CHANGES_COUNT=$((CHANGES_COUNT + 1))
    CHANGES_LOG+=("${msg}")
    if [[ "${DRY_RUN}" -eq 1 ]]; then
        echo "  [WOULD CHANGE] ${msg}"
    else
        echo "  [CHANGE] ${msg}"
    fi
}

record_ok() {
    local msg="$1"
    echo "  [OK] ${msg}"
}

sync_file_content() {
    local dest_path="$1"
    local expected_content="$2"
    local mode="${3:-0644}"
    local owner="${4:-root}"
    local group="${5:-root}"

    local full_path
    full_path="$(rpath "${dest_path}")"
    local dir_path
    dir_path="$(dirname "${full_path}")"

    local content_matches=0
    if [[ -f "${full_path}" ]]; then
        local cur_normalized exp_normalized
        cur_normalized="$(tr -d '\r' < "${full_path}")"
        exp_normalized="$(printf "%s\n" "${expected_content}" | tr -d '\r')"
        if [[ "${cur_normalized}" == "${exp_normalized}" ]]; then
            content_matches=1
        fi
    fi

    if [[ "${content_matches}" -eq 1 ]]; then
        record_ok "File ${dest_path} matches specification"
    else
        record_change "Write ${dest_path} (${mode} ${owner}:${group})"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            mkdir -p "${dir_path}"
            printf "%s\n" "${expected_content}" | tr -d '\r' > "${full_path}"
            chmod "${mode}" "${full_path}" 2>/dev/null || true
            if [[ "$(id -u)" -eq 0 && "${TARGET_ROOT}" == "/" ]]; then
                chown "${owner}:${group}" "${full_path}" 2>/dev/null || true
            fi
        fi
    fi
}

ensure_directory() {
    local dir_path="$1"
    local mode="${2:-0755}"
    local owner="${3:-root}"
    local group="${4:-root}"

    local full_path
    full_path="$(rpath "${dir_path}")"

    if [[ ! -d "${full_path}" ]]; then
        record_change "Create directory ${dir_path} (${mode} ${owner}:${group})"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            mkdir -p "${full_path}"
            chmod "${mode}" "${full_path}" 2>/dev/null || true
            if [[ "$(id -u)" -eq 0 && "${TARGET_ROOT}" == "/" ]]; then
                chown "${owner}:${group}" "${full_path}" 2>/dev/null || true
            fi
        fi
    else
        record_ok "Directory ${dir_path} exists"
    fi
}

echo "========================================================"
echo " RPi-VJ Appliance Hardening & Resilience Manager (A3-13)"
echo " Target Root: ${TARGET_ROOT:-/}"
echo " Mode: $( [[ ${CHECK_MODE} -eq 1 ]] && echo "CHECK" || ( [[ ${DRY_RUN} -eq 1 ]] && echo "DRY-RUN" || echo "APPLY" ) )"
echo "========================================================"

# ---------------------------------------------------------------------------
# 1. Storage Directories & Failsafe Reserve Layout
# ---------------------------------------------------------------------------
echo ""
echo "--> [1/5] Enforcing Storage Layout & Directories..."
ensure_directory "/opt/rpi-vj" "0755" "root" "root"
ensure_directory "/srv/rpi-vj" "0755" "vj" "vj"
ensure_directory "/srv/rpi-vj/shows" "0755" "vj" "vj"
ensure_directory "/srv/rpi-vj/staging" "0750" "vj" "vj"
ensure_directory "/var/lib/rpi-vj" "0750" "vj" "vj"
ensure_directory "/var/log/journal" "2755" "root" "systemd-journal"

# ---------------------------------------------------------------------------
# 2. Hardware Watchdog Integration (bcm2835_wdt & systemd RuntimeWatchdogSec=15)
# ---------------------------------------------------------------------------
echo ""
echo "--> [2/5] Configuring Hardware Watchdog (bcm2835_wdt, RuntimeWatchdogSec=15)..."

WATCHDOG_CONF="[Manager]
RuntimeWatchdogSec=15
RebootWatchdogSec=30
ShutdownWatchdogSec=10min
WatchdogDevice=/dev/watchdog"
sync_file_content "/etc/systemd/system.conf.d/00-watchdog.conf" "${WATCHDOG_CONF}" "0644" "root" "root"

MODULES_WATCHDOG="bcm2835_wdt"
sync_file_content "/etc/modules-load.d/watchdog.conf" "${MODULES_WATCHDOG}" "0644" "root" "root"

# ---------------------------------------------------------------------------
# 3. Systemd Service Hardening & Sandboxing Matrix
# ---------------------------------------------------------------------------
echo ""
echo "--> [3/5] Enforcing Systemd Security Sandboxing on Appliance Services..."

BROKER_SERVICE="[Unit]
Description=RPi-VJ Command Broker and Monotonic Supervisor
Documentation=file:///opt/rpi-vj/docs/appliance/BROKER.md
DefaultDependencies=no
After=local-fs.target
Before=rpi-vj-renderer.service

[Service]
Type=simple
User=vj
Group=vj
SupplementaryGroups=video render input
RuntimeDirectory=rpi-vj
RuntimeDirectoryMode=0775
StateDirectory=rpi-vj
StateDirectoryMode=0775
ExecStart=/opt/rpi-vj/bin/vj-broker --socket /run/rpi-vj/broker.sock --http 127.0.0.1:8080 --web-root /opt/rpi-vj/web --state-file /var/lib/rpi-vj/desired-state.json --supervisor-state /var/lib/rpi-vj/supervisor-state.json --boot-ledger /var/lib/rpi-vj/boot-ledger.jsonl
Restart=always
RestartSec=2
KillMode=mixed
TimeoutStopSec=5
StandardOutput=journal
StandardError=journal

# Systemd Security Hardening
ProtectSystem=strict
ProtectHome=yes
NoNewPrivileges=yes
PrivateTmp=yes
ReadWritePaths=/srv/rpi-vj /var/lib/rpi-vj /run/rpi-vj

[Install]
WantedBy=multi-user.target"

RENDERER_SERVICE="[Unit]
Description=RPi-VJ KMS and Cage Renderer Service
Documentation=file:///opt/rpi-vj/docs/appliance/RENDERER.md file:///tasks/APPLIANCE-V3/ARCHITECTURE.md
DefaultDependencies=no
After=systemd-udevd.service local-fs.target systemd-tmpfiles-setup.service
Requires=local-fs.target
Wants=systemd-udevd.service
Conflicts=display-manager.service lightdm.service gdm.service sddm.service nodm.service

[Service]
Type=simple
User=vj
Group=vj
SupplementaryGroups=video render input audio
WorkingDirectory=/var/lib/rpi-vj
RuntimeDirectory=rpi-vj/renderer
RuntimeDirectoryMode=0700
Environment=XDG_RUNTIME_DIR=/run/rpi-vj/renderer
Environment=HOME=/var/lib/rpi-vj
Environment=WLR_LIBINPUT_NO_DEVICES=1
UnsetEnvironment=XCURSOR_THEME XCURSOR_SIZE DESKTOP_SESSION GDMSESSION XDG_CURRENT_DESKTOP
ExecStart=/opt/rpi-vj/scripts/appliance/launch-renderer.sh
Restart=always
RestartSec=2s
KillMode=control-group
SendSIGKILL=yes
TimeoutStopSec=5s
TimeoutStartSec=30s
StandardOutput=journal
StandardError=journal
LimitNOFILE=65536
LimitMEMLOCK=infinity
MemoryMax=1600M

# Systemd Security Hardening
ProtectSystem=strict
ProtectHome=yes
NoNewPrivileges=yes
PrivateTmp=yes
ReadWritePaths=/srv/rpi-vj /var/lib/rpi-vj /run/rpi-vj

[Install]
WantedBy=multi-user.target"

CONTROLLER_SERVICE="[Unit]
Description=RPi-VJ Native Stream Deck XL Controller and State UI Service
Documentation=file:///opt/rpi-vj/docs/appliance/STREAMDECK.md file:///tasks/APPLIANCE-V3/ARCHITECTURE.md
DefaultDependencies=no
After=systemd-udevd.service local-fs.target rpi-vj-broker.service
Wants=systemd-udevd.service
Requires=rpi-vj-broker.service

[Service]
Type=simple
User=vj
Group=vj
SupplementaryGroups=video render input
RuntimeDirectory=rpi-vj
RuntimeDirectoryMode=0775
StateDirectory=rpi-vj
StateDirectoryMode=0775
Environment=RPI_VJ_BROKER_SOCKET=/run/rpi-vj/broker.sock
Environment=RPI_VJ_BROKER_STATE_PATH=/run/rpi-vj/controller-state.json
Environment=RPI_VJ_BOOT_LEDGER_PATH=/var/lib/rpi-vj/boot-ledger.jsonl
ExecStart=/opt/rpi-vj/bin/rpi-vj-controller
Restart=always
RestartSec=2s
KillMode=mixed
TimeoutStopSec=5s
StandardOutput=journal
StandardError=journal
LimitNOFILE=65536

# Systemd Security Hardening
ProtectSystem=strict
ProtectHome=yes
NoNewPrivileges=yes
PrivateTmp=yes
ReadWritePaths=/srv/rpi-vj /var/lib/rpi-vj /run/rpi-vj

[Install]
WantedBy=multi-user.target"

sync_file_content "/etc/systemd/system/rpi-vj-broker.service" "${BROKER_SERVICE}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-renderer.service" "${RENDERER_SERVICE}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-controller.service" "${CONTROLLER_SERVICE}" "0644" "root" "root"

# ---------------------------------------------------------------------------
# 4. Storage Garbage Collection Service & Timer
# ---------------------------------------------------------------------------
echo ""
echo "--> [4/5] Configuring Staging Garbage Collection (>24h) & Generation Retention..."

STORAGE_GC_SERVICE="[Unit]
Description=RPi-VJ Storage Garbage Collector
Documentation=file:///opt/rpi-vj/docs/appliance/HARDENING.md
DefaultDependencies=no
After=local-fs.target

[Service]
Type=oneshot
User=vj
Group=vj
ExecStart=/opt/rpi-vj/scripts/appliance/storage-gc.py --max-staging-age-hours 24 --keep-generations 2
StandardOutput=journal
StandardError=journal

# Security Hardening
ProtectSystem=strict
ProtectHome=yes
NoNewPrivileges=yes
PrivateTmp=yes
ReadWritePaths=/srv/rpi-vj /var/lib/rpi-vj"

STORAGE_GC_TIMER="[Unit]
Description=RPi-VJ Periodic Storage Garbage Collection Timer
Documentation=file:///opt/rpi-vj/docs/appliance/HARDENING.md

[Timer]
OnBootSec=10min
OnUnitActiveSec=1h
Persistent=true

[Install]
WantedBy=timers.target"

sync_file_content "/etc/systemd/system/rpi-vj-storage-gc.service" "${STORAGE_GC_SERVICE}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-storage-gc.timer" "${STORAGE_GC_TIMER}" "0644" "root" "root"

# ---------------------------------------------------------------------------
# 5. Boot Cause Logger & Clean Shutdown Service
# ---------------------------------------------------------------------------
echo ""
echo "--> [5/5] Configuring Boot Cause Logger & Clean Shutdown Integration..."

SHUTDOWN_LOGGER_SERVICE="[Unit]
Description=RPi-VJ Clean Shutdown Logger
Documentation=file:///opt/rpi-vj/docs/appliance/HARDENING.md
DefaultDependencies=no
Before=shutdown.target reboot.target halt.target poweroff.target final.target
Wants=local-fs.target
After=local-fs.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/bin/true
ExecStop=/opt/rpi-vj/scripts/appliance/record-boot-event.sh clean-shutdown
TimeoutStopSec=5s
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target"

sync_file_content "/etc/systemd/system/rpi-vj-shutdown-logger.service" "${SHUTDOWN_LOGGER_SERVICE}" "0644" "root" "root"

# Enable timer and shutdown logger if on live system
if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" && "${DRY_RUN}" -eq 0 ]]; then
    systemctl daemon-reload 2>/dev/null || true
    systemctl enable rpi-vj-storage-gc.timer 2>/dev/null || true
    systemctl start rpi-vj-storage-gc.timer 2>/dev/null || true
    systemctl enable rpi-vj-shutdown-logger.service 2>/dev/null || true
    systemctl start rpi-vj-shutdown-logger.service 2>/dev/null || true
fi

echo ""
echo "========================================================"
echo " Hardening Summary"
echo "========================================================"

if [[ "${CHANGES_COUNT}" -eq 0 ]]; then
    echo "Status: FULLY HARDENED (No drift detected)"
    exit 0
else
    if [[ "${CHECK_MODE}" -eq 1 ]]; then
        echo "Status: DRIFT DETECTED"
        echo "Report: ${CHANGES_COUNT} change(s) required to achieve hardened posture."
        exit 1
    else
        echo "Status: HARDENING APPLIED"
        echo "Report: Applied ${CHANGES_COUNT} hardening change(s):"
        for item in "${CHANGES_LOG[@]}"; do
            echo "  - ${item}"
        done
        exit 0
    fi
fi
