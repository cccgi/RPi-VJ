#!/usr/bin/env bash
# record-boot-event.sh - Appliance V3 boot event recording helper
# Contract: A3-04 Persistent Observability
#
# Supported subcommands:
#   marker <A|B|...>
#   kernel-start
#   userspace-start
#   renderer-start
#   renderer-ready
#   renderer-exit [status_code]
#   first-frame
#   streamdeck-discover
#   streamdeck-open
#   streamdeck-disconnect
#   ingest-start [detail]
#   ingest-end [status_code] [detail]
#   supervisor-action <action>
#   clean-shutdown
#   check-unclean
#   setup-journald
#   read [limit]
#   record <event> [service] [exit_status] [drm_connector] [detail]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LEDGER_PATH="${RPI_VJ_BOOT_LEDGER_PATH:-/var/lib/rpi-vj/boot-ledger.jsonl}"

# Locate boot-ledger.py
LEDGER_PY="${LEDGER_PY:-}"
if [ -z "$LEDGER_PY" ]; then
    if [ -f "${SCRIPT_DIR}/boot-ledger.py" ]; then
        LEDGER_PY="${SCRIPT_DIR}/boot-ledger.py"
    elif [ -f "/opt/rpi-vj/scripts/appliance/boot-ledger.py" ]; then
        LEDGER_PY="/opt/rpi-vj/scripts/appliance/boot-ledger.py"
    fi
fi

# Locate Python
PYTHON="${PYTHON:-}"
if [ -z "$PYTHON" ]; then
    if command -v python3 >/dev/null 2>&1; then
        PYTHON="python3"
    elif command -v python >/dev/null 2>&1; then
        PYTHON="python"
    fi
fi

# Fallback logger if python or boot-ledger.py is unavailable
record_fallback_shell() {
    local event="$1"
    local service="${2:-}"
    local exit_status="${3:-}"
    local drm_connector="${4:-}"
    local detail="${5:-}"

    local boot_id="${RPI_VJ_BOOT_ID:-}"
    if [ -z "$boot_id" ] && [ -f /proc/sys/kernel/random/boot_id ]; then
        boot_id="$(cat /proc/sys/kernel/random/boot_id | tr -d ' \n\r')"
    fi
    if [ -z "$boot_id" ]; then
        boot_id="unknown-boot-id"
    fi

    local monotonic="0.0"
    if [ -f /proc/uptime ]; then
        monotonic="$(awk '{print $1}' /proc/uptime)"
    fi

    local realtime
    realtime="$(date -u +"%Y-%m-%dT%H:%M:%SZ" 2>/dev/null || date +"%Y-%m-%dT%H:%M:%SZ")"

    mkdir -p "$(dirname "$LEDGER_PATH")"

    # Escape JSON strings
    service_json="null"
    [ -n "$service" ] && service_json="\"$service\""

    exit_json="null"
    [ -n "$exit_status" ] && exit_json="$exit_status"

    drm_json="null"
    [ -n "$drm_connector" ] && drm_json="\"$drm_connector\""

    detail_json="null"
    [ -n "$detail" ] && detail_json="\"$detail\""

    local entry
    entry="{\"boot_id\":\"${boot_id}\",\"monotonic_uptime\":${monotonic},\"realtime\":\"${realtime}\",\"clock_synchronized\":false,\"event\":\"${event}\",\"service\":${service_json},\"exit_status\":${exit_json},\"drm_connector\":${drm_json},\"detail\":${detail_json}}"

    echo "$entry" >> "$LEDGER_PATH"
    sync -d "$LEDGER_PATH" 2>/dev/null || sync "$LEDGER_PATH" 2>/dev/null || true
    echo "$entry"
}

run_python_ledger() {
    if [ -n "$PYTHON" ] && [ -n "$LEDGER_PY" ] && [ -f "$LEDGER_PY" ]; then
        "$PYTHON" "$LEDGER_PY" "$@"
        return 0
    fi
    return 1
}

cmd="${1:-help}"
shift || true

case "$cmd" in
    marker)
        marker_name="${1:-A}"
        if run_python_ledger record --event "marker" --detail "$marker_name" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "marker" "" "" "" "$marker_name"
        fi
        ;;

    kernel-start)
        if run_python_ledger record --event "kernel_start" --service "kernel" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "kernel_start" "kernel"
        fi
        ;;

    userspace-start)
        if run_python_ledger record --event "userspace_start" --service "systemd" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "userspace_start" "systemd"
        fi
        ;;

    renderer-start)
        if run_python_ledger record --event "renderer_start" --service "rpi-vj-renderer" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "renderer_start" "rpi-vj-renderer"
        fi
        ;;

    renderer-ready)
        if run_python_ledger record --event "renderer_ready" --service "rpi-vj-renderer" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "renderer_ready" "rpi-vj-renderer"
        fi
        ;;

    renderer-exit)
        status_code="${1:-0}"
        if run_python_ledger record --event "renderer_exit" --service "rpi-vj-renderer" --exit-status "$status_code" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "renderer_exit" "rpi-vj-renderer" "$status_code"
        fi
        ;;

    first-frame)
        if run_python_ledger record --event "first_frame" --service "rpi-vj-renderer" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "first_frame" "rpi-vj-renderer"
        fi
        ;;

    streamdeck-discover)
        if run_python_ledger record --event "streamdeck_discover" --service "rpi-vj-controller" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "streamdeck_discover" "rpi-vj-controller"
        fi
        ;;

    streamdeck-open)
        if run_python_ledger record --event "streamdeck_open" --service "rpi-vj-controller" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "streamdeck_open" "rpi-vj-controller"
        fi
        ;;

    streamdeck-disconnect)
        if run_python_ledger record --event "streamdeck_disconnect" --service "rpi-vj-controller" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "streamdeck_disconnect" "rpi-vj-controller"
        fi
        ;;

    ingest-start)
        detail="${1:-}"
        if run_python_ledger record --event "ingest_start" --service "rpi-vj-ingest" --detail "$detail" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "ingest_start" "rpi-vj-ingest" "" "" "$detail"
        fi
        ;;

    ingest-end)
        status_code="${1:-0}"
        detail="${2:-}"
        if run_python_ledger record --event "ingest_end" --service "rpi-vj-ingest" --exit-status "$status_code" --detail "$detail" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "ingest_end" "rpi-vj-ingest" "$status_code" "" "$detail"
        fi
        ;;

    supervisor-action)
        action="${1:-unknown}"
        if run_python_ledger record --event "supervisor_action" --service "rpi-vj-supervisor" --detail "$action" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "supervisor_action" "rpi-vj-supervisor" "" "" "$action"
        fi
        ;;

    clean-shutdown)
        if run_python_ledger record --event "clean_shutdown_request" --service "rpi-vj-supervisor" --path "$LEDGER_PATH"; then
            exit 0
        else
            record_fallback_shell "clean_shutdown_request" "rpi-vj-supervisor"
        fi
        ;;

    check-unclean)
        if run_python_ledger check-unclean --path "$LEDGER_PATH"; then
            exit 0
        else
            exit 1
        fi
        ;;

    setup-journald)
        target="${1:-/etc/systemd/journald.conf.d/10-rpi-vj.conf}"
        if run_python_ledger journald-conf --install --target "$target"; then
            exit 0
        else
            mkdir -p "$(dirname "$target")"
            cat << 'EOF' > "$target"
# /etc/systemd/journald.conf.d/10-rpi-vj.conf
# Managed for Appliance V3 Persistent Observability (A3-04)
[Journal]
Storage=persistent
SystemMaxUse=200M
SystemKeepFree=15%
RuntimeMaxUse=50M
MaxRetentionSec=1month
ForwardToSyslog=no
EOF
            echo "Installed journald configuration to $target (shell fallback)"
        fi
        ;;

    read|dump)
        limit="${1:-}"
        if [ -n "$limit" ]; then
            run_python_ledger read --path "$LEDGER_PATH" --limit "$limit"
        else
            run_python_ledger read --path "$LEDGER_PATH"
        fi
        ;;

    record)
        event="${1:-}"
        service="${2:-}"
        exit_status="${3:-}"
        drm_conn="${4:-}"
        detail="${5:-}"
        if [ -z "$event" ]; then
            echo "Error: record requires <event>" >&2
            exit 2
        fi
        args=(record --event "$event" --path "$LEDGER_PATH")
        [ -n "$service" ] && args+=(--service "$service")
        [ -n "$exit_status" ] && args+=(--exit-status "$exit_status")
        [ -n "$drm_conn" ] && args+=(--drm-connector "$drm_conn")
        [ -n "$detail" ] && args+=(--detail "$detail")
        if run_python_ledger "${args[@]}"; then
            exit 0
        else
            record_fallback_shell "$event" "$service" "$exit_status" "$drm_conn" "$detail"
        fi
        ;;

    help|--help|-h)
        echo "Usage: $0 <command> [arguments...]"
        echo "Commands:"
        echo "  marker <A|B|...>                Record test/verification marker"
        echo "  kernel-start                    Record kernel_start event"
        echo "  userspace-start                 Record userspace_start event"
        echo "  renderer-start                  Record renderer_start event"
        echo "  renderer-ready                  Record renderer_ready event"
        echo "  renderer-exit [code]            Record renderer_exit event"
        echo "  first-frame                     Record first_frame event"
        echo "  streamdeck-discover             Record streamdeck_discover event"
        echo "  streamdeck-open                 Record streamdeck_open event"
        echo "  streamdeck-disconnect           Record streamdeck_disconnect event"
        echo "  ingest-start [detail]           Record ingest_start event"
        echo "  ingest-end [code] [detail]      Record ingest_end event"
        echo "  supervisor-action <action>      Record supervisor_action event"
        echo "  clean-shutdown                  Record clean_shutdown_request event"
        echo "  check-unclean                   Check prior boot for unclean shutdown"
        echo "  setup-journald [target]         Install bounded journald configuration"
        echo "  read [limit]                    Read recorded events from ledger"
        echo "  record <ev> [svc] [code] [drm]  Record arbitrary event"
        ;;

    *)
        echo "Unknown command: $cmd" >&2
        echo "Run '$0 help' for usage." >&2
        exit 1
        ;;
esac
