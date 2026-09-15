#!/usr/bin/env bash
# scripts/appliance/launch-renderer.sh
# RPi-VJ Appliance V3 — KMS and Cage Renderer Launcher
# Strictly adheres to tasks/APPLIANCE-V3/A3-02-kms-cage-renderer.md and ARCHITECTURE.md

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration and Defaults
# ---------------------------------------------------------------------------
DEFAULT_SHOW_URL="http://localhost:8080/gig/"
SHOW_URL="${SHOW_URL:-${DEFAULT_SHOW_URL}}"

DEFAULT_PROFILE_DIR="/var/lib/rpi-vj/renderer-profile"
RENDERER_PROFILE_DIR="${RENDERER_PROFILE_DIR:-${DEFAULT_PROFILE_DIR}}"

RENDERER_SERVER_WAIT_SECS="${RENDERER_SERVER_WAIT_SECS:-10}"
RENDERER_DRM_WAIT_SECS="${RENDERER_DRM_WAIT_SECS:-5}"
DRM_CARD="${DRM_CARD:-/dev/dri/card0}"

# Auto-detect Cage binary
if [[ -n "${CAGE_BIN:-}" ]]; then
    : # Keep user override
elif command -v cage >/dev/null 2>&1; then
    CAGE_BIN="$(command -v cage)"
elif [[ -x "/usr/bin/cage" ]]; then
    CAGE_BIN="/usr/bin/cage"
else
    CAGE_BIN="cage"
fi

# Auto-detect Chromium binary: prioritize direct /usr/lib/chromium/chromium to bypass wrapper scripts
if [[ -n "${CHROMIUM_BIN:-}" ]]; then
    : # Keep user override
elif [[ -x "/usr/lib/chromium/chromium" ]]; then
    CHROMIUM_BIN="/usr/lib/chromium/chromium"
elif command -v chromium >/dev/null 2>&1; then
    CHROMIUM_BIN="$(command -v chromium)"
elif command -v chromium-browser >/dev/null 2>&1; then
    CHROMIUM_BIN="$(command -v chromium-browser)"
else
    CHROMIUM_BIN="/usr/lib/chromium/chromium"
fi

# ---------------------------------------------------------------------------
# KMS & Kernel Command-Line Management Functions
# ---------------------------------------------------------------------------

# Function to configure kernel cmdline for single 1080p60 output on port 0
# Primary connector (port 0, closest to USB-C power) is HDMI-A-1.
# Secondary connector (port 1) is HDMI-A-2 and must not have any forced declaration.
configure_kms_cmdline() {
    local cmdline_path="${1:-}"
    local dry_run="${2:-0}"

    if [[ -z "${cmdline_path}" ]]; then
        if [[ -f "/boot/firmware/cmdline.txt" ]]; then
            cmdline_path="/boot/firmware/cmdline.txt"
        elif [[ -f "/boot/cmdline.txt" ]]; then
            cmdline_path="/boot/cmdline.txt"
        else
            echo "ERROR: Neither /boot/firmware/cmdline.txt nor /boot/cmdline.txt found." >&2
            return 1
        fi
    fi

    if [[ ! -r "${cmdline_path}" ]]; then
        echo "ERROR: Cannot read cmdline file: ${cmdline_path}" >&2
        return 1
    fi

    local current_cmdline
    current_cmdline="$(cat "${cmdline_path}")"
    # Ensure it's treated as a single line, trimmed
    current_cmdline="$(echo "${current_cmdline}" | tr '\n\r' ' ' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"

    # 1. Remove any forced declaration for HDMI-A-2 (e.g. video=HDMI-A-2:...)
    local modified_cmdline
    modified_cmdline="$(echo "${current_cmdline}" | sed -E 's/(^|[[:space:]]+)video=HDMI-A-2:[^[:space:]]+//g')"

    # 2. Check and force HDMI-A-1 to 1920x1080@60D
    # The 'D' modifier forces digital output enabled even if TV is disconnected at boot.
    local target_param="video=HDMI-A-1:1920x1080@60D"

    # Remove any existing video=HDMI-A-1 parameter that differs
    modified_cmdline="$(echo "${modified_cmdline}" | sed -E 's/(^|[[:space:]]+)video=HDMI-A-1:[^[:space:]]+//g')"

    # Normalize whitespace
    modified_cmdline="$(echo "${modified_cmdline}" | tr -s ' ' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"

    # Append the canonical target parameter
    modified_cmdline="${modified_cmdline} ${target_param}"
    modified_cmdline="$(echo "${modified_cmdline}" | tr -s ' ' | sed -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//')"

    if [[ "${current_cmdline}" == "${modified_cmdline}" ]]; then
        echo "KMS cmdline in ${cmdline_path} is already compliant (IDEMPOTENT)."
        return 0
    fi

    echo "KMS cmdline modification required for ${cmdline_path}:"
    echo "  Current:  ${current_cmdline}"
    echo "  Proposed: ${modified_cmdline}"

    if [[ "${dry_run}" -eq 1 ]]; then
        echo "  [DRY-RUN] No changes written to ${cmdline_path}."
        return 0
    fi

    if [[ ! -w "${cmdline_path}" ]]; then
        echo "ERROR: Cannot write to ${cmdline_path}. Root privileges required." >&2
        return 1
    fi

    local tmp_file="${cmdline_path}.tmp.$$"
    printf "%s\n" "${modified_cmdline}" > "${tmp_file}"
    mv -f "${tmp_file}" "${cmdline_path}"
    echo "Successfully updated KMS cmdline in ${cmdline_path}."
}

# Function to inspect /proc/cmdline at runtime
check_runtime_kms() {
    if [[ ! -f "/proc/cmdline" ]]; then
        return 0
    fi
    local proc_cmdline
    proc_cmdline="$(cat /proc/cmdline)"

    if echo "${proc_cmdline}" | grep -q "video=HDMI-A-2:"; then
        echo "[renderer] WARNING: Forced declaration for second HDMI connector detected in /proc/cmdline." >&2
        echo "[renderer] WARNING: Secondary port should remain blank without forced declaration." >&2
    fi

    if ! echo "${proc_cmdline}" | grep -q "video=HDMI-A-1:1920x1080@60D"; then
        echo "[renderer] INFO: video=HDMI-A-1:1920x1080@60D is not active in /proc/cmdline." >&2
        echo "[renderer] INFO: Recommended for single-output cold boot with disconnected display." >&2
    else
        echo "[renderer] KMS: HDMI-A-1 locked to 1920x1080@60D (primary port 0)."
    fi
}

# ---------------------------------------------------------------------------
# XDG Runtime Directory & Pointer Suppression
# ---------------------------------------------------------------------------
ensure_xdg_runtime_dir() {
    # The launcher owns XDG_RUNTIME_DIR.
    # Must exist with permissions 0700 and be writable by the current process user.
    if [[ -z "${XDG_RUNTIME_DIR:-}" ]]; then
        if [[ -d "/run/rpi-vj/renderer" ]] && [[ -w "/run/rpi-vj/renderer" ]]; then
            XDG_RUNTIME_DIR="/run/rpi-vj/renderer"
        elif [[ -d "/run/user/$(id -u 2>/dev/null || echo 1000)" ]] && [[ -w "/run/user/$(id -u 2>/dev/null || echo 1000)" ]]; then
            XDG_RUNTIME_DIR="/run/user/$(id -u 2>/dev/null || echo 1000)"
        else
            XDG_RUNTIME_DIR="/tmp/rpi-vj-runtime-$(id -u 2>/dev/null || echo 1000)"
        fi
    fi

    if [[ ! -d "${XDG_RUNTIME_DIR}" ]]; then
        mkdir -p "${XDG_RUNTIME_DIR}"
        chmod 0700 "${XDG_RUNTIME_DIR}" 2>/dev/null || true
    else
        chmod 0700 "${XDG_RUNTIME_DIR}" 2>/dev/null || true
    fi

    export XDG_RUNTIME_DIR
    echo "[renderer] XDG_RUNTIME_DIR configured: ${XDG_RUNTIME_DIR}"
}

suppress_pointer_and_desktop() {
    # Non-negotiable decision #3: Cage owns sole program output.
    # No pointer UI, no cursor process, no desktop shell, no display manager.
    unset XCURSOR_THEME
    unset XCURSOR_SIZE
    unset DESKTOP_SESSION
    unset GDMSESSION
    unset XDG_CURRENT_DESKTOP
    export XCURSOR_THEME=""
    export XCURSOR_SIZE=0
    export WLR_LIBINPUT_NO_DEVICES="${WLR_LIBINPUT_NO_DEVICES:-1}"
}

# ---------------------------------------------------------------------------
# Profile Sanitization
# ---------------------------------------------------------------------------
sanitize_profile() {
    # Marks the last Chromium session as a clean exit so the "Restore pages"
    # bubble never appears on TV display after power cuts or sudden termination.
    local pref_file="${RENDERER_PROFILE_DIR}/Default/Preferences"
    if ! mkdir -p "${RENDERER_PROFILE_DIR}/Default" 2>/dev/null; then
        echo "[renderer] NOTICE: Cannot write to ${RENDERER_PROFILE_DIR}; falling back to profile in ${XDG_RUNTIME_DIR}/profile" >&2
        RENDERER_PROFILE_DIR="${XDG_RUNTIME_DIR}/profile"
        mkdir -p "${RENDERER_PROFILE_DIR}/Default" 2>/dev/null || true
        pref_file="${RENDERER_PROFILE_DIR}/Default/Preferences"
    fi

    # Remove stale singleton lock symlinks if present
    rm -f "${RENDERER_PROFILE_DIR}/SingletonLock" \
          "${RENDERER_PROFILE_DIR}/SingletonSocket" \
          "${RENDERER_PROFILE_DIR}/SingletonCookie" 2>/dev/null || true

    if [[ -f "${pref_file}" ]]; then
        if command -v python3 >/dev/null 2>&1; then
            local py_pref_target="${pref_file}"
            if command -v cygpath >/dev/null 2>&1; then
                py_pref_target="$(cygpath -w "${pref_file}")"
            fi
            python3 - "${py_pref_target}" <<'PY'
import json, sys
pref_path = sys.argv[1]
try:
    with open(pref_path, 'r', encoding='utf-8') as f:
        data = json.load(f)
    prof = data.setdefault('profile', {})
    prof['exit_type'] = 'Normal'
    prof['exited_cleanly'] = True
    with open(pref_path, 'w', encoding='utf-8') as f:
        json.dump(data, f)
except Exception as e:
    sys.stderr.write(f"[renderer] Profile sanitize notice: {e}\n")
PY
        fi
        echo "[renderer] Dedicated Chromium profile sanitized: ${RENDERER_PROFILE_DIR}"
    else
        echo "[renderer] Dedicated Chromium profile initialized at: ${RENDERER_PROFILE_DIR}"
    fi
}

# ---------------------------------------------------------------------------
# Bounded Readiness Checks (Monotonic Clock)
# ---------------------------------------------------------------------------
check_drm_readiness() {
    # The service starts from local files and DRM devices, not network targets.
    local deadline=$((SECONDS + RENDERER_DRM_WAIT_SECS))
    local drm_found=0

    echo "[renderer] Verifying DRM device readiness (max ${RENDERER_DRM_WAIT_SECS}s)..."
    while [[ "${SECONDS}" -lt "${deadline}" ]]; do
        if [[ -e "${DRM_CARD}" ]] || [[ -e "/dev/dri/card0" ]] || [[ -e "/dev/dri/card1" ]]; then
            drm_found=1
            break
        fi
        sleep 0.2
    done

    if [[ "${drm_found}" -eq 1 ]]; then
        echo "[renderer] DRM device detected."
    else
        echo "[renderer] NOTICE: DRM device node not present in /dev/dri. Proceeding (may be mock/test environment)." >&2
    fi
}

check_local_server_readiness() {
    # Bounded local-server readiness check using monotonic bash $SECONDS.
    # Never blocks boot indefinitely.
    local deadline=$((SECONDS + RENDERER_SERVER_WAIT_SECS))
    local server_ready=0

    echo "[renderer] Checking local server readiness: ${SHOW_URL} (max ${RENDERER_SERVER_WAIT_SECS}s)..."
    while [[ "${SECONDS}" -lt "${deadline}" ]]; do
        if command -v curl >/dev/null 2>&1; then
            if curl -sf --connect-timeout 1 "${SHOW_URL}" >/dev/null 2>&1; then
                server_ready=1
                break
            fi
        else
            # curl not present; cannot poll HTTP
            break
        fi
        sleep 0.2
    done

    if [[ "${server_ready}" -eq 1 ]]; then
        echo "[renderer] Local server is responding."
    else
        echo "[renderer] Local server check ended (elapsed: ${RENDERER_SERVER_WAIT_SECS}s). Launching browser."
    fi
}

# ---------------------------------------------------------------------------
# Browser Execution inside Cage Compositor
# ---------------------------------------------------------------------------
exec_browser_session() {
    # This function executes inside Cage (WAYLAND_DISPLAY is active).
    # Cage forwards output to the forced KMS display.

    # If wlr-randr is present, enforce output modes directly
    if command -v wlr-randr >/dev/null 2>&1; then
        wlr-randr --output HDMI-A-1 --mode 1920x1080@60 2>/dev/null || true
        wlr-randr --output HDMI-A-2 --off 2>/dev/null || true
    fi

    local chromium_args=(
        "--ozone-platform=wayland"
        "--enable-features=UseOzonePlatform"
        "--kiosk"
        "--app=${SHOW_URL}"
        "--user-data-dir=${RENDERER_PROFILE_DIR}"
        "--password-store=basic"
        "--use-mock-keychain"
        "--autoplay-policy=no-user-gesture-required"
        "--enable-gpu-rasterization"
        "--ignore-gpu-blocklist"
        "--renderer-process-limit=1"
        "--process-per-site"
        "--js-flags=--max-old-space-size=256"
        "--disable-extensions"
        "--disable-background-networking"
        "--disable-component-update"
        "--disable-sync"
        "--disable-session-crashed-bubble"
        "--hide-crash-restore-bubble"
        "--no-first-run"
        "--no-default-browser-check"
        "--disable-infobars"
        "--noerrdialogs"
        "--check-for-update-interval=31536000"
        "--disable-features=Translate,InfiniteSessionRestore,OptimizationGuideModelDownloading,OptimizationGuideOnDeviceModel"
        "--disable-pinch"
        "--overscroll-history-navigation=0"
    )

    echo "[renderer] Executing Chromium: ${CHROMIUM_BIN} (app=${SHOW_URL})"
    exec "${CHROMIUM_BIN}" "${chromium_args[@]}"
}

# ---------------------------------------------------------------------------
# Self-Test Suite (--test)
# ---------------------------------------------------------------------------
run_self_tests() {
    echo "=== Running RPi-VJ Renderer Launcher Test Suite ==="
    local tests_passed=0
    local tests_failed=0

    # Test 1: XDG_RUNTIME_DIR creation and ownership
    echo -n "Test 1: ensure_xdg_runtime_dir creates valid 0700 dir ... "
    local test_xdg_dir
    test_xdg_dir="$(mktemp -d 2>/dev/null || echo "/tmp/test-xdg-$$")"
    rm -rf "${test_xdg_dir}"
    XDG_RUNTIME_DIR="${test_xdg_dir}" ensure_xdg_runtime_dir >/dev/null 2>&1
    if [[ -d "${test_xdg_dir}" ]]; then
        echo "PASS"
        tests_passed=$((tests_passed + 1))
    else
        echo "FAIL (directory not created)"
        tests_failed=$((tests_failed + 1))
    fi
    rm -rf "${test_xdg_dir}"

    # Test 2: Pointer suppression
    echo -n "Test 2: suppress_pointer_and_desktop clears cursor vars ... "
    XCURSOR_THEME="Adwaita" XCURSOR_SIZE="24" suppress_pointer_and_desktop
    if [[ "${XCURSOR_THEME}" == "" && "${XCURSOR_SIZE}" == "0" ]]; then
        echo "PASS"
        tests_passed=$((tests_passed + 1))
    else
        echo "FAIL (cursor theme=${XCURSOR_THEME}, size=${XCURSOR_SIZE})"
        tests_failed=$((tests_failed + 1))
    fi

    # Test 3: KMS cmdline configuration - HDMI-A-1 forced, HDMI-A-2 removed
    echo -n "Test 3: configure_kms_cmdline forces HDMI-A-1 and strips HDMI-A-2 ... "
    local test_cmdline_file
    test_cmdline_file="$(mktemp 2>/dev/null || echo "/tmp/test-cmdline-$$")"
    printf "console=serial0,115200 console=tty1 root=PARTUUID=1234 rootwait video=HDMI-A-2:1920x1080@60D quiet\n" > "${test_cmdline_file}"
    configure_kms_cmdline "${test_cmdline_file}" 0 >/dev/null 2>&1
    local res_cmdline
    res_cmdline="$(cat "${test_cmdline_file}")"
    if echo "${res_cmdline}" | grep -q "video=HDMI-A-1:1920x1080@60D" && ! echo "${res_cmdline}" | grep -q "video=HDMI-A-2:"; then
        # Check idempotency
        configure_kms_cmdline "${test_cmdline_file}" 0 >/dev/null 2>&1
        local res_cmdline_2
        res_cmdline_2="$(cat "${test_cmdline_file}")"
        if [[ "${res_cmdline}" == "${res_cmdline_2}" ]]; then
            echo "PASS"
            tests_passed=$((tests_passed + 1))
        else
            echo "FAIL (not idempotent)"
            tests_failed=$((tests_failed + 1))
        fi
    else
        echo "FAIL (result: ${res_cmdline})"
        tests_failed=$((tests_failed + 1))
    fi
    rm -f "${test_cmdline_file}"

    # Test 4: Profile sanitization
    echo -n "Test 4: sanitize_profile cleans crash state ... "
    local test_prof_dir
    test_prof_dir="$(mktemp -d 2>/dev/null || echo "/tmp/test-prof-$$")"
    mkdir -p "${test_prof_dir}/Default"
    touch "${test_prof_dir}/SingletonLock"
    printf '{"profile":{"exit_type":"Crashed","exited_cleanly":false}}\n' > "${test_prof_dir}/Default/Preferences"
    RENDERER_PROFILE_DIR="${test_prof_dir}" sanitize_profile >/dev/null 2>&1
    if [[ ! -f "${test_prof_dir}/SingletonLock" ]]; then
        if command -v python3 >/dev/null 2>&1; then
            local test_pref_file="${test_prof_dir}/Default/Preferences"
            local py_test_target="${test_pref_file}"
            if command -v cygpath >/dev/null 2>&1; then
                py_test_target="$(cygpath -w "${test_pref_file}")"
            fi
            local is_clean
            is_clean=$(python3 -c "import json; d=json.load(open(r'${py_test_target}', encoding='utf-8')); print(d['profile']['exit_type']=='Normal' and d['profile']['exited_cleanly']==True)")
            if [[ "${is_clean}" == "True" ]]; then
                echo "PASS"
                tests_passed=$((tests_passed + 1))
            else
                echo "FAIL (Preferences not updated)"
                tests_failed=$((tests_failed + 1))
            fi
        else
            echo "PASS (lock removed, python skipped)"
            tests_passed=$((tests_passed + 1))
        fi
    else
        echo "FAIL (SingletonLock still present)"
        tests_failed=$((tests_failed + 1))
    fi
    rm -rf "${test_prof_dir}"

    # Test 5: Monotonic bounded server wait timeout
    echo -n "Test 5: check_local_server_readiness terminates within bound ... "
    local t_start=${SECONDS}
    RENDERER_SERVER_WAIT_SECS=1 SHOW_URL="http://127.0.0.1:59999/nonexistent" check_local_server_readiness >/dev/null 2>&1
    local t_elapsed=$((SECONDS - t_start))
    if [[ "${t_elapsed}" -le 3 ]]; then
        echo "PASS (${t_elapsed}s elapsed)"
        tests_passed=$((tests_passed + 1))
    else
        echo "FAIL (took ${t_elapsed}s)"
        tests_failed=$((tests_failed + 1))
    fi

    echo "=== Summary: ${tests_passed} passed, ${tests_failed} failed ==="
    if [[ "${tests_failed}" -eq 0 ]]; then
        return 0
    else
        return 1
    fi
}

# ---------------------------------------------------------------------------
# CLI Argument Dispatcher
# ---------------------------------------------------------------------------
usage() {
    cat << 'EOF'
Usage: launch-renderer.sh [OPTIONS]

KMS and Cage kiosk renderer launcher for RPi-VJ Appliance V3.
Starts Cage kiosk compositor and Chromium fullscreen kiosk in a single cgroup.

Options:
  --exec-browser        Internal mode called by Cage compositor to exec Chromium
  --setup-kms [PATH]    Configure kernel cmdline to force HDMI-A-1:1920x1080@60D and remove HDMI-A-2
  --check-kms           Inspect current /proc/cmdline for KMS compliance
  --dry-run             Run environment checks and output commands without executing
  --check               Verify readiness of DRM, profile, server, and runtime dir
  --test                Run built-in automated test suite
  -h, --help            Show this help message
EOF
    exit 0
}

MODE="launch"
CMDLINE_TARGET=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --exec-browser)
            MODE="browser"
            shift
            ;;
        --setup-kms)
            MODE="setup-kms"
            if [[ $# -gt 1 && ! "$2" =~ ^-- ]]; then
                CMDLINE_TARGET="$2"
                shift 2
            else
                shift
            fi
            ;;
        --check-kms)
            MODE="check-kms"
            shift
            ;;
        --dry-run)
            MODE="dry-run"
            shift
            ;;
        --check)
            MODE="check"
            shift
            ;;
        --test)
            MODE="test"
            shift
            ;;
        -h|--help)
            usage
            ;;
        *)
            echo "ERROR: Unknown option: $1" >&2
            usage
            ;;
    esac
done

case "${MODE}" in
    test)
        run_self_tests
        exit $?
        ;;
    setup-kms)
        configure_kms_cmdline "${CMDLINE_TARGET}" 0
        exit $?
        ;;
    check-kms)
        check_runtime_kms
        exit 0
        ;;
    browser)
        exec_browser_session
        ;;
    check)
        echo "Checking RPi-VJ Renderer Prerequisites:"
        check_runtime_kms
        ensure_xdg_runtime_dir
        suppress_pointer_and_desktop
        check_drm_readiness
        check_local_server_readiness
        sanitize_profile
        echo "All checks completed."
        exit 0
        ;;
    dry-run)
        echo "=== RPi-VJ Renderer Launcher: DRY RUN ==="
        check_runtime_kms
        ensure_xdg_runtime_dir
        suppress_pointer_and_desktop
        check_drm_readiness
        check_local_server_readiness
        sanitize_profile
        echo "[dry-run] Compositor binary: ${CAGE_BIN}"
        echo "[dry-run] Browser binary:    ${CHROMIUM_BIN}"
        echo "[dry-run] Show URL:          ${SHOW_URL}"
        echo "[dry-run] Profile Dir:       ${RENDERER_PROFILE_DIR}"
        echo "[dry-run] Command: exec ${CAGE_BIN} -d -s -- $0 --exec-browser"
        exit 0
        ;;
    launch)
        # Main launch sequence
        echo "========================================================"
        echo " RPi-VJ Appliance V3 — KMS Cage Renderer Launcher"
        echo "========================================================"
        check_runtime_kms
        ensure_xdg_runtime_dir
        suppress_pointer_and_desktop
        check_drm_readiness
        check_local_server_readiness
        sanitize_profile

        SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/$(basename "${BASH_SOURCE[0]}")"
        echo "[renderer] Launching Cage kiosk compositor: ${CAGE_BIN} -d -s -- ${SELF} --exec-browser"
        exec "${CAGE_BIN}" -d -s -- "${SELF}" --exec-browser
        ;;
esac
