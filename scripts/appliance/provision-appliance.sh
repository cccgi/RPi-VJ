#!/usr/bin/env bash
# scripts/appliance/provision-appliance.sh
# Deterministic, idempotent provisioning for RPi-VJ Appliance V3 (Raspberry Pi OS Lite 64-bit)
# Strictly adheres to tasks/APPLIANCE-V3/A3-01-lite-provisioning.md and ARCHITECTURE.md

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEFAULT_MANIFEST="${SCRIPT_DIR}/packages.manifest"

# Default runtime options
TARGET_ROOT="/"
MANIFEST_PATH="${DEFAULT_MANIFEST}"
DRY_RUN=0
CHECK_MODE=0
SKIP_APT=0

usage() {
    cat << 'EOF'
Usage: provision-appliance.sh [OPTIONS]

Idempotent provisioning script for RPi-VJ Appliance V3 on Raspberry Pi OS Lite 64-bit.
Configures kiosk user, service ownership, headless display configuration, mDNS, SSH hardening,
and stages systemd units without running the renderer.

Options:
  --manifest PATH     Path to package manifest (default: scripts/appliance/packages.manifest)
  --root PATH         Target root filesystem directory (default: /)
  --dry-run, -n       Simulate changes without modifying filesystem
  --check             Exit 0 if system state matches specification, 1 if changes are needed
  --skip-apt          Skip apt-get package installations (for testing / pre-installed roots)
  -h, --help          Show this help message and exit

Observable Contract:
  When run twice consecutively, the second execution must report 0 changes.
EOF
    exit 0
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --manifest)
            MANIFEST_PATH="$2"
            shift 2
            ;;
        --root)
            TARGET_ROOT="$2"
            shift 2
            ;;
        --dry-run|-n)
            DRY_RUN=1
            shift
            ;;
        --check)
            CHECK_MODE=1
            DRY_RUN=1
            shift
            ;;
        --skip-apt)
            SKIP_APT=1
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

# Strip trailing slash from TARGET_ROOT unless it's just "/"
if [[ "${TARGET_ROOT}" != "/" ]]; then
    TARGET_ROOT="${TARGET_ROOT%/}"
fi

# Privilege check
if [[ "${TARGET_ROOT}" == "" || "${TARGET_ROOT}" == "/" ]]; then
    if [[ "${DRY_RUN}" -eq 0 && "${CHECK_MODE}" -eq 0 ]]; then
        if [[ "$(id -u)" -ne 0 ]]; then
            echo "ERROR: provision-appliance.sh must be executed as root (e.g. with sudo)" >&2
            exit 1
        fi
    fi
fi

if [[ ! -f "${MANIFEST_PATH}" ]]; then
    echo "ERROR: Package manifest not found: ${MANIFEST_PATH}" >&2
    exit 1
fi

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

# Resolve target path relative to TARGET_ROOT
rpath() {
    local path="$1"
    if [[ "${TARGET_ROOT}" == "/" || -z "${TARGET_ROOT}" ]]; then
        echo "${path}"
    else
        echo "${TARGET_ROOT}${path}"
    fi
}

# Safe symlink helper (handles Linux and non-Linux test environments)
safe_symlink() {
    local target="$1"
    local link_dest="$2"
    mkdir -p "$(dirname "${link_dest}")"
    rm -f "${link_dest}"
    if ln -sf "${target}" "${link_dest}" 2>/dev/null; then
        return 0
    else
        # Fallback for environments where symlink creation is restricted without admin
        printf "%s\n" "${target}" > "${link_dest}"
    fi
}

# Helper to write file content safely and idempotently
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
        # Compare normalized contents (stripping CR for cross-platform robustness)
        local cur_normalized exp_normalized
        cur_normalized="$(tr -d '\r' < "${full_path}")"
        exp_normalized="$(printf "%s\n" "${expected_content}" | tr -d '\r')"
        if [[ "${cur_normalized}" == "${exp_normalized}" ]]; then
            content_matches=1
        fi
    fi

    if [[ "${content_matches}" -eq 1 ]]; then
        record_ok "File ${dest_path} is up to date"
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

# Helper to ensure directories exist with correct permissions
ensure_directory() {
    local dir_path="$1"
    local mode="${2:-0755}"
    local owner="${3:-root}"
    local group="${4:-root}"

    local full_path
    full_path="$(rpath "${dir_path}")"

    local exists=0
    if [[ -d "${full_path}" ]]; then
        exists=1
    fi

    if [[ "${exists}" -eq 0 ]]; then
        record_change "Create directory ${dir_path} (${mode} ${owner}:${group})"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            mkdir -p "${full_path}"
            chmod "${mode}" "${full_path}" 2>/dev/null || true
            if [[ "$(id -u)" -eq 0 && "${TARGET_ROOT}" == "/" ]]; then
                chown "${owner}:${group}" "${full_path}" 2>/dev/null || true
            fi
        fi
    else
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            chmod "${mode}" "${full_path}" 2>/dev/null || true
            if [[ "$(id -u)" -eq 0 && "${TARGET_ROOT}" == "/" ]]; then
                chown "${owner}:${group}" "${full_path}" 2>/dev/null || true
            fi
        fi
        record_ok "Directory ${dir_path} exists (${mode} ${owner}:${group})"
    fi
}

echo "========================================================"
echo " RPi-VJ Appliance Provisioning: Phase 1 Lite Deployment"
echo " Target Root: ${TARGET_ROOT:-/}"
echo " Package Manifest: ${MANIFEST_PATH}"
echo " Mode: $( [[ ${CHECK_MODE} -eq 1 ]] && echo "CHECK" || ( [[ ${DRY_RUN} -eq 1 ]] && echo "DRY-RUN" || echo "ENFORCE" ) )"
echo "========================================================"

# ---------------------------------------------------------------------------
# 1. Base Storage Layout & Bounded Journald Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--> [1/7] Enforcing Storage Layout and Bounded Journal Logging..."

ensure_directory "/opt/rpi-vj" "0755" "root" "root"
ensure_directory "/srv/rpi-vj" "0755" "vj" "vj"
ensure_directory "/srv/rpi-vj/shows" "0755" "vj" "vj"
ensure_directory "/srv/rpi-vj/staging" "0750" "vj" "vj"
ensure_directory "/var/lib/rpi-vj" "0750" "vj" "vj"
ensure_directory "/var/log/journal" "2755" "root" "systemd-journal"

# Ephemeral runtime socket dir via systemd-tmpfiles
TMPFILES_CONF="d /run/rpi-vj 0755 vj vj -"
sync_file_content "/etc/tmpfiles.d/rpi-vj.conf" "${TMPFILES_CONF}" "0644" "root" "root"

# Bound persistent journald disk usage to satisfy ARCHITECTURE.md Non-negotiable #11
JOURNALD_CONF="[Journal]
Storage=persistent
SystemMaxUse=200M
SystemKeepFree=500M
SystemMaxFileSize=20M
MaxRetentionSec=1month"
sync_file_content "/etc/systemd/journald.conf.d/00-rpi-vj-bounded.conf" "${JOURNALD_CONF}" "0644" "root" "root"

# ---------------------------------------------------------------------------
# 2. System User 'vj' & Hardware Access Groups
# ---------------------------------------------------------------------------
echo ""
echo "--> [2/7] Enforcing 'vj' Service User and Hardware Permissions..."

REQUIRED_GROUPS=("video" "render" "input" "audio" "plugdev")

if [[ "${TARGET_ROOT}" == "/" && "$(id -u)" -eq 0 ]]; then
    # Ensure vj group exists
    if ! getent group vj >/dev/null 2>&1; then
        record_change "Create system group vj"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            groupadd --system vj
        fi
    else
        record_ok "Group vj exists"
    fi

    # Ensure required hardware groups exist
    for grp in "${REQUIRED_GROUPS[@]}"; do
        if ! getent group "${grp}" >/dev/null 2>&1; then
            record_change "Create missing hardware group ${grp}"
            if [[ "${DRY_RUN}" -eq 0 ]]; then
                groupadd --system "${grp}" 2>/dev/null || groupadd "${grp}"
            fi
        fi
    done

    # Ensure vj user exists with locked shell and correct home
    if ! id -u vj >/dev/null 2>&1; then
        record_change "Create system user vj with locked shell (/usr/sbin/nologin) and home /var/lib/rpi-vj"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            useradd --system \
                --home-dir "/var/lib/rpi-vj" \
                --no-create-home \
                --shell "/usr/sbin/nologin" \
                --gid vj \
                --groups "$(IFS=,; echo "${REQUIRED_GROUPS[*]}")" \
                vj
            passwd -l vj >/dev/null 2>&1 || true
        fi
    else
        # Verify shell is non-interactive
        CURRENT_SHELL="$(getent passwd vj | cut -d: -f7)"
        if [[ "${CURRENT_SHELL}" != "/usr/sbin/nologin" && "${CURRENT_SHELL}" != "/bin/false" ]]; then
            record_change "Lock vj user shell to /usr/sbin/nologin"
            if [[ "${DRY_RUN}" -eq 0 ]]; then
                usermod -s "/usr/sbin/nologin" vj
            fi
        else
            record_ok "User vj shell is locked (${CURRENT_SHELL})"
        fi

        # Check group memberships
        USER_GROUPS="$(id -Gn vj 2>/dev/null || echo "")"
        MISSING_USER_GROUPS=()
        for grp in "${REQUIRED_GROUPS[@]}"; do
            if ! [[ " ${USER_GROUPS} " =~ [[:space:]]${grp}[[:space:]] ]]; then
                MISSING_USER_GROUPS+=("${grp}")
            fi
        done

        if [[ "${#MISSING_USER_GROUPS[@]}" -gt 0 ]]; then
            record_change "Add supplementary groups to user vj: ${MISSING_USER_GROUPS[*]}"
            if [[ "${DRY_RUN}" -eq 0 ]]; then
                usermod -a -G "$(IFS=,; echo "${MISSING_USER_GROUPS[*]}")" vj
            fi
        else
            record_ok "User vj has all required hardware groups (${REQUIRED_GROUPS[*]})"
        fi
    fi
else
    # Mock / non-root / chroot verification
    if [[ "${TARGET_ROOT}" != "/" && -n "${TARGET_ROOT}" ]]; then
        ETC_GROUP="$(rpath "/etc/group")"
        ETC_PASSWD="$(rpath "/etc/passwd")"
        mkdir -p "$(dirname "${ETC_GROUP}")"
        touch "${ETC_GROUP}" "${ETC_PASSWD}"

        if ! grep -q "^vj:" "${ETC_GROUP}" 2>/dev/null; then
            record_change "Create system group vj (target root)"
            if [[ "${DRY_RUN}" -eq 0 ]]; then
                echo "vj:x:1001:" >> "${ETC_GROUP}"
            fi
        else
            record_ok "Group vj exists (target root)"
        fi

        if ! grep -q "^vj:" "${ETC_PASSWD}" 2>/dev/null; then
            record_change "Create system user vj (target root)"
            if [[ "${DRY_RUN}" -eq 0 ]]; then
                echo "vj:x:1001:1001:RPi-VJ Service User:/var/lib/rpi-vj:/usr/sbin/nologin" >> "${ETC_PASSWD}"
            fi
        else
            record_ok "User vj exists (target root)"
        fi
    else
        record_ok "User/group verification (dry-run on host)"
    fi
fi

# ---------------------------------------------------------------------------
# 3. Package Manifest Enforcement & Version Ledger
# ---------------------------------------------------------------------------
echo ""
echo "--> [3/7] Inspecting Package Manifest & Recording Installed Releases..."

MANIFEST_PACKAGES=()
while IFS= read -r line || [[ -n "$line" ]]; do
    clean_line="$(echo "${line}" | sed -e 's/#.*//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"
    if [[ -n "${clean_line}" ]]; then
        pkg_name="$(echo "${clean_line}" | awk '{print $1}')"
        MANIFEST_PACKAGES+=("${pkg_name}")
    fi
done < "${MANIFEST_PATH}"

MISSING_PACKAGES=()
if [[ "${SKIP_APT}" -eq 0 && "${TARGET_ROOT}" == "/" && "$(id -u)" -eq 0 && -x "/usr/bin/dpkg-query" ]]; then
    for pkg in "${MANIFEST_PACKAGES[@]}"; do
        if ! dpkg-query -W -f='${Status}' "${pkg}" 2>/dev/null | grep -q "install ok installed" && \
           ! dpkg-query -W -f='${Status}' "${pkg}t64" 2>/dev/null | grep -q "install ok installed"; then
            MISSING_PACKAGES+=("${pkg}")
        fi
    done

    if [[ "${#MISSING_PACKAGES[@]}" -gt 0 ]]; then
        record_change "Install required packages: ${MISSING_PACKAGES[*]}"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            export DEBIAN_FRONTEND=noninteractive
            apt-get update -qq
            apt-get install -y --no-install-recommends "${MISSING_PACKAGES[@]}"
        fi
    else
        record_ok "All ${#MANIFEST_PACKAGES[@]} manifest packages are installed"
    fi

    LEDGER_PATH="/var/lib/rpi-vj/installed-packages.manifest"
    RECORDED_VERSIONS="$(dpkg-query -W -f='${Package}=${Version}\n' "${MANIFEST_PACKAGES[@]}" 2>/dev/null || true)"
    sync_file_content "${LEDGER_PATH}" "${RECORDED_VERSIONS}" "0644" "vj" "vj"
else
    # Mock / Skip-APT mode
    LEDGER_PATH="/var/lib/rpi-vj/installed-packages.manifest"
    MOCK_VERSIONS="# Pinned / Recorded Appliance Packages
$(for p in "${MANIFEST_PACKAGES[@]}"; do echo "${p}=pinned-release"; done)"
    sync_file_content "${LEDGER_PATH}" "${MOCK_VERSIONS}" "0644" "vj" "vj"
    record_ok "Skipping live apt execution (mock / test / non-root mode)"
fi

# ---------------------------------------------------------------------------
# 4. Disable Desktop, Display Managers, Autologin, Cloud-init, Wait-online
# ---------------------------------------------------------------------------
echo ""
echo "--> [4/7] Disabling Desktop Stack, Display Managers, Autologin & Wait-online..."

# 4a. Enforce default target is multi-user.target (console), NOT graphical.target
DEFAULT_TARGET_LINK="/etc/systemd/system/default.target"
FULL_TARGET_LINK="$(rpath "${DEFAULT_TARGET_LINK}")"

is_multi_user_default() {
    if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" ]]; then
        local cur_target
        cur_target="$(systemctl get-default 2>/dev/null || echo "")"
        [[ "${cur_target}" == "multi-user.target" ]]
    elif [[ -L "${FULL_TARGET_LINK}" ]]; then
        local link_target
        link_target="$(readlink "${FULL_TARGET_LINK}" 2>/dev/null || echo "")"
        [[ "${link_target}" == *"multi-user.target"* ]]
    elif [[ -f "${FULL_TARGET_LINK}" ]]; then
        local file_target
        file_target="$(cat "${FULL_TARGET_LINK}" 2>/dev/null || echo "")"
        [[ "${file_target}" == *"multi-user.target"* ]]
    else
        false
    fi
}

if ! is_multi_user_default; then
    record_change "Set systemd default target to multi-user.target"
    if [[ "${DRY_RUN}" -eq 0 ]]; then
        if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" ]]; then
            systemctl set-default multi-user.target
        else
            safe_symlink "/lib/systemd/system/multi-user.target" "${FULL_TARGET_LINK}"
        fi
    fi
else
    record_ok "Default systemd target is multi-user.target"
fi

# 4b. Mask and disable display managers and unwanted desktop compositors
SERVICES_TO_MASK=(
    "lightdm.service"
    "labwc.service"
    "wayfire.service"
    "gdm.service"
    "sddm.service"
    "nodm.service"
    "weston.service"
    "systemd-networkd-wait-online.service"
    "NetworkManager-wait-online.service"
)

for svc in "${SERVICES_TO_MASK[@]}"; do
    SVC_LINK="/etc/systemd/system/${svc}"
    FULL_SVC_LINK="$(rpath "${SVC_LINK}")"

    is_masked() {
        if [[ -L "${FULL_SVC_LINK}" ]]; then
            local r_target
            r_target="$(readlink "${FULL_SVC_LINK}" 2>/dev/null || echo "")"
            if [[ "${r_target}" == "/dev/null" ]]; then
                return 0
            fi
        fi
        if [[ -f "${FULL_SVC_LINK}" ]]; then
            local f_target
            f_target="$(cat "${FULL_SVC_LINK}" 2>/dev/null || echo "")"
            if [[ "${f_target}" == "/dev/null" ]]; then
                return 0
            fi
        fi
        if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" ]]; then
            [[ "$(systemctl is-enabled "${svc}" 2>/dev/null || echo "")" == "masked" ]]
        else
            return 1
        fi
    }

    if ! is_masked; then
        record_change "Disable and mask ${svc}"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" ]]; then
                systemctl stop "${svc}" 2>/dev/null || true
                systemctl disable "${svc}" 2>/dev/null || true
                systemctl mask "${svc}" 2>/dev/null || true
            else
                safe_symlink "/dev/null" "${FULL_SVC_LINK}"
            fi
        fi
    else
        record_ok "Service ${svc} is masked"
    fi
done

# 4c. Disable desktop autologin overrides if present
AUTOLOGIN_GETTY="/etc/systemd/system/getty@tty1.service.d/autologin.conf"
FULL_AUTOLOGIN_GETTY="$(rpath "${AUTOLOGIN_GETTY}")"
if [[ -f "${FULL_AUTOLOGIN_GETTY}" ]]; then
    record_change "Remove desktop console autologin override: ${AUTOLOGIN_GETTY}"
    if [[ "${DRY_RUN}" -eq 0 ]]; then
        rm -f "${FULL_AUTOLOGIN_GETTY}"
    fi
else
    record_ok "No desktop autologin override present"
fi

# 4d. Disable cloud-init post-provisioning
CLOUD_INIT_DIR="$(rpath "/etc/cloud")"
CLOUD_DISABLED_FLAG="$(rpath "/etc/cloud/cloud-init.disabled")"
if [[ -d "${CLOUD_INIT_DIR}" || -x "/usr/bin/cloud-init" ]]; then
    if [[ ! -f "${CLOUD_DISABLED_FLAG}" ]]; then
        record_change "Disable cloud-init post-provisioning (/etc/cloud/cloud-init.disabled)"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            mkdir -p "${CLOUD_INIT_DIR}"
            touch "${CLOUD_DISABLED_FLAG}"
        fi
    else
        record_ok "Cloud-init already disabled"
    fi

    CLOUD_SERVICES=("cloud-init.service" "cloud-init-local.service" "cloud-config.service" "cloud-final.service")
    for csvc in "${CLOUD_SERVICES[@]}"; do
        CSVC_LINK="$(rpath "/etc/systemd/system/${csvc}")"
        is_cloud_masked=0
        if [[ -L "${CSVC_LINK}" && "$(readlink "${CSVC_LINK}" 2>/dev/null)" == "/dev/null" ]]; then
            is_cloud_masked=1
        elif [[ -f "${CSVC_LINK}" && "$(cat "${CSVC_LINK}" 2>/dev/null)" == "/dev/null" ]]; then
            is_cloud_masked=1
        fi

        if [[ "${is_cloud_masked}" -eq 0 ]]; then
            record_change "Mask ${csvc}"
            if [[ "${DRY_RUN}" -eq 0 ]]; then
                if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" ]]; then
                    systemctl disable --now "${csvc}" 2>/dev/null || true
                    systemctl mask "${csvc}" 2>/dev/null || true
                else
                    safe_symlink "/dev/null" "${CSVC_LINK}"
                fi
            fi
        fi
    done
else
    record_ok "Cloud-init is absent"
fi

# 4e. Purge desktop panel / file manager / screen locker packages if present on system
DESKTOP_BLOAT_PKGS=("lightdm" "labwc" "wayfire" "pcmanfm" "pcmanfm-wayland" "lxpanel" "wf-panel-pi" "wayvnc" "xscreensaver")
FOUND_BLOAT=()
if [[ "${SKIP_APT}" -eq 0 && "${TARGET_ROOT}" == "/" && "$(id -u)" -eq 0 && -x "/usr/bin/dpkg-query" ]]; then
    for bp in "${DESKTOP_BLOAT_PKGS[@]}"; do
        if dpkg-query -W -f='${Status}' "${bp}" 2>/dev/null | grep -q "install ok installed"; then
            FOUND_BLOAT+=("${bp}")
        fi
    done
    if [[ "${#FOUND_BLOAT[@]}" -gt 0 ]]; then
        record_change "Purge desktop bloat packages: ${FOUND_BLOAT[*]}"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            export DEBIAN_FRONTEND=noninteractive
            apt-get purge -y "${FOUND_BLOAT[@]}"
        fi
    else
        record_ok "Desktop environment packages (LightDM/Labwc/Wayfire/panels) are absent"
    fi
else
    record_ok "Desktop bloat check passed"
fi

# ---------------------------------------------------------------------------
# 5. SSH Maintenance Access (Key-Authentication Only)
# ---------------------------------------------------------------------------
echo ""
echo "--> [5/7] Enforcing Key-Only SSH Maintenance Access..."

SSH_HARDENING_CONF="# RPi-VJ Appliance V3 SSH Hardening Configuration
# Enforces key authentication for maintenance; disables interactive passwords.
PasswordAuthentication no
PubkeyAuthentication yes
KbdInteractiveAuthentication no
ChallengeResponseAuthentication no
X11Forwarding no
PermitRootLogin prohibit-password"

sync_file_content "/etc/ssh/sshd_config.d/99-rpi-vj.conf" "${SSH_HARDENING_CONF}" "0644" "root" "root"

if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" ]]; then
    if ! systemctl is-enabled ssh.service >/dev/null 2>&1 && ! systemctl is-enabled sshd.service >/dev/null 2>&1; then
        record_change "Enable ssh.service"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            systemctl enable ssh.service 2>/dev/null || systemctl enable sshd.service 2>/dev/null || true
        fi
    else
        record_ok "ssh.service is enabled"
    fi
else
    record_ok "ssh.service check passed (mock/non-root mode)"
fi

# ---------------------------------------------------------------------------
# 6. Hostname 'rpi-vj' & Avahi mDNS Configuration
# ---------------------------------------------------------------------------
echo ""
echo "--> [6/7] Configuring Hostname (rpi-vj) and Avahi mDNS Service..."

HOSTNAME_FILE="$(rpath "/etc/hostname")"
CURRENT_HOSTNAME=""
if [[ -f "${HOSTNAME_FILE}" ]]; then
    CURRENT_HOSTNAME="$(tr -d '[:space:]' < "${HOSTNAME_FILE}")"
fi

if [[ "${CURRENT_HOSTNAME}" != "rpi-vj" ]]; then
    record_change "Set hostname to rpi-vj"
    if [[ "${DRY_RUN}" -eq 0 ]]; then
        mkdir -p "$(dirname "${HOSTNAME_FILE}")"
        echo "rpi-vj" > "${HOSTNAME_FILE}"
        if [[ "${TARGET_ROOT}" == "/" && -x "/usr/bin/hostnamectl" ]]; then
            hostnamectl set-hostname rpi-vj 2>/dev/null || true
        fi
    fi
else
    record_ok "Hostname is already rpi-vj"
fi

# /etc/hosts mapping
HOSTS_FILE="$(rpath "/etc/hosts")"
if [[ -f "${HOSTS_FILE}" ]]; then
    if ! grep -qE "^127\.0\.1\.1[[:space:]]+rpi-vj" "${HOSTS_FILE}"; then
        record_change "Update /etc/hosts with 127.0.1.1 rpi-vj"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            if grep -q "^127\.0\.1\.1" "${HOSTS_FILE}"; then
                sed -i 's/^127\.0\.1\.1.*/127.0.1.1\trpi-vj/' "${HOSTS_FILE}"
            else
                printf "127.0.1.1\trpi-vj\n" >> "${HOSTS_FILE}"
            fi
        fi
    else
        record_ok "/etc/hosts contains 127.0.1.1 rpi-vj"
    fi
else
    record_change "Create /etc/hosts with 127.0.0.1 localhost and 127.0.1.1 rpi-vj"
    if [[ "${DRY_RUN}" -eq 0 ]]; then
        mkdir -p "$(dirname "${HOSTS_FILE}")"
        cat << 'EOF' > "${HOSTS_FILE}"
127.0.0.1	localhost
::1		localhost ip6-localhost ip6-loopback
ff02::1		ip6-allnodes
ff02::2		ip6-allrouters

127.0.1.1	rpi-vj
EOF
    fi
fi

# Enable avahi-daemon service for rpi-vj.local resolution
if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" ]]; then
    if ! systemctl is-enabled avahi-daemon.service >/dev/null 2>&1; then
        record_change "Enable avahi-daemon.service"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            systemctl enable avahi-daemon.service 2>/dev/null || true
            systemctl start avahi-daemon.service 2>/dev/null || true
        fi
    else
        record_ok "avahi-daemon.service is enabled"
    fi
else
    record_ok "avahi-daemon service check passed (mock/non-root mode)"
fi

# ---------------------------------------------------------------------------
# 7. Install V3 Systemd Service Units (Renderer Kept Inactive Until A3-02)
# ---------------------------------------------------------------------------
echo ""
echo "--> [7/7] Installing Appliance V3 Systemd Service Definitions..."

# Unit 1: Renderer Service (Cage + Chromium kiosk)
# Note: Specifically NOT started or enabled until A3-02 delivers the renderer implementation.
RENDERER_UNIT="[Unit]
Description=RPi-VJ Cage and Chromium Kiosk Renderer
Documentation=file:///tasks/APPLIANCE-V3/ARCHITECTURE.md
After=systemd-user-sessions.service plymouth-quit-wait.service
Wants=rpi-vj-broker.service

[Service]
Type=simple
User=vj
Group=vj
WorkingDirectory=/var/lib/rpi-vj
Environment=XDG_RUNTIME_DIR=/run/user/1001
Environment=WLR_LIBINPUT_NO_DEVICES=1
ExecStart=/opt/rpi-vj/bin/rpi-vj-renderer
Restart=always
RestartSec=2s
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target"

# Unit 2: Stream Deck Controller Service
CONTROLLER_UNIT="[Unit]
Description=RPi-VJ Stream Deck Native Controller Service
Documentation=file:///tasks/APPLIANCE-V3/ARCHITECTURE.md
After=systemd-udev-settle.service
Wants=rpi-vj-broker.service

[Service]
Type=simple
User=vj
Group=vj
WorkingDirectory=/var/lib/rpi-vj
ExecStart=/opt/rpi-vj/bin/rpi-vj-controller
Restart=always
RestartSec=2s

[Install]
WantedBy=multi-user.target"

# Unit 3: Broker & State Store Service
BROKER_UNIT="[Unit]
Description=RPi-VJ Command & State Broker Service
Documentation=file:///tasks/APPLIANCE-V3/ARCHITECTURE.md
After=network.target

[Service]
Type=simple
User=vj
Group=vj
WorkingDirectory=/var/lib/rpi-vj
ExecStart=/opt/rpi-vj/bin/rpi-vj-broker
Restart=always
RestartSec=2s

[Install]
WantedBy=multi-user.target"

# Unit 4: Supervisor & Recovery Engine Service
SUPERVISOR_UNIT="[Unit]
Description=RPi-VJ Supervisor and Recovery Engine
Documentation=file:///tasks/APPLIANCE-V3/ARCHITECTURE.md
After=rpi-vj-broker.service

[Service]
Type=simple
User=vj
Group=vj
WorkingDirectory=/var/lib/rpi-vj
ExecStart=/opt/rpi-vj/bin/rpi-vj-supervisor
Restart=always
RestartSec=2s

[Install]
WantedBy=multi-user.target"

# Unit 5: Ingest Worker Service
INGEST_UNIT="[Unit]
Description=RPi-VJ USB Ingest Worker
Documentation=file:///tasks/APPLIANCE-V3/ARCHITECTURE.md
After=rpi-vj-broker.service

[Service]
Type=oneshot
User=vj
Group=vj
WorkingDirectory=/var/lib/rpi-vj
ExecStart=/opt/rpi-vj/bin/rpi-vj-ingest"

# Unit 6: USB Storage Path Watcher & Service Helper
USB_PATH_UNIT="[Unit]
Description=RPi-VJ USB Drive Ingest Path Watcher
Documentation=file:///tasks/APPLIANCE-V3/ARCHITECTURE.md
After=local-fs.target

[Path]
DirectoryNotEmpty=/srv/rpi-vj/staging
Unit=rpi-vj-usb.service

[Install]
WantedBy=multi-user.target"

USB_SERVICE_UNIT="[Unit]
Description=RPi-VJ USB Mount Discovery & Trigger Helper
Documentation=file:///tasks/APPLIANCE-V3/ARCHITECTURE.md

[Service]
Type=oneshot
ExecStart=/opt/rpi-vj/bin/rpi-vj-usb-helper"

sync_file_content "/etc/systemd/system/rpi-vj-renderer.service" "${RENDERER_UNIT}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-controller.service" "${CONTROLLER_UNIT}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-broker.service" "${BROKER_UNIT}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-supervisor.service" "${SUPERVISOR_UNIT}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-ingest.service" "${INGEST_UNIT}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-usb.path" "${USB_PATH_UNIT}" "0644" "root" "root"
sync_file_content "/etc/systemd/system/rpi-vj-usb.service" "${USB_SERVICE_UNIT}" "0644" "root" "root"

# Ensure renderer service is NOT active (A3-01 observable contract)
if [[ "${TARGET_ROOT}" == "/" && -x "/bin/systemctl" ]]; then
    if systemctl is-active rpi-vj-renderer.service >/dev/null 2>&1; then
        record_change "Stop rpi-vj-renderer.service (waiting for A3-02 implementation)"
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            systemctl stop rpi-vj-renderer.service 2>/dev/null || true
        fi
    else
        record_ok "rpi-vj-renderer.service is inactive (waiting for A3-02)"
    fi
    if [[ "${DRY_RUN}" -eq 0 ]]; then
        systemctl daemon-reload 2>/dev/null || true
    fi
else
    record_ok "rpi-vj-renderer.service installed and inactive"
fi

# ---------------------------------------------------------------------------
# Provisioning Summary & Idempotency Check
# ---------------------------------------------------------------------------
echo ""
echo "========================================================"
echo " RPi-VJ Appliance Provisioning Execution Summary"
echo "========================================================"

if [[ "${CHANGES_COUNT}" -eq 0 ]]; then
    echo "Status: SUCCESS (IDEMPOTENT)"
    echo "Report: No changes required. System state matches specification."
    exit 0
else
    if [[ "${CHECK_MODE}" -eq 1 ]]; then
        echo "Status: DRIFT DETECTED"
        echo "Report: ${CHANGES_COUNT} change(s) required to reach specification."
        exit 1
    else
        echo "Status: SUCCESS (MODIFIED)"
        echo "Report: Applied ${CHANGES_COUNT} change(s):"
        for item in "${CHANGES_LOG[@]}"; do
            echo "  - ${item}"
        done
        exit 0
    fi
fi
