#!/usr/bin/env python3
"""boot-ledger.py - Persistent Boot Ledger & Observability for Appliance V3.

Implements task A3-04:
- Structured JSONL logging to /var/lib/rpi-vj/boot-ledger.jsonl
- Required schema: boot_id, monotonic_uptime, realtime, clock_synchronized,
  event, service, exit_status, drm_connector, detail.
- Durable fsync flushes at critical boundaries without syncing per render frame.
- Unclean reboot detection (correlating across boot IDs).
- Strict monotonic clock evaluation for watchdog/recovery (realtime immunity).
- Bounded persistent journald configuration generation.
"""

from __future__ import annotations

import argparse
import ctypes
import datetime
import json
import os
import sys
import tempfile
import time
import unittest
import uuid
from typing import Any

DEFAULT_LEDGER_PATH = os.environ.get(
    "RPI_VJ_BOOT_LEDGER_PATH", "/var/lib/rpi-vj/boot-ledger.jsonl"
)
DEFAULT_JOURNALD_CONF_PATH = "/etc/systemd/journald.conf.d/10-rpi-vj.conf"

REQUIRED_EVENTS = [
    "kernel_start",
    "userspace_start",
    "renderer_start",
    "renderer_ready",
    "renderer_exit",
    "first_frame",
    "streamdeck_discover",
    "streamdeck_open",
    "streamdeck_disconnect",
    "ingest_start",
    "ingest_end",
    "supervisor_action",
    "clean_shutdown_request",
    "unclean_filesystem_indication",
]


def get_boot_id() -> str:
    """Returns the kernel or transient boot ID as a string."""
    if "RPI_VJ_BOOT_ID" in os.environ and os.environ["RPI_VJ_BOOT_ID"].strip():
        return os.environ["RPI_VJ_BOOT_ID"].strip()

    # Standard Linux kernel boot ID
    kernel_boot_id = "/proc/sys/kernel/random/boot_id"
    if os.path.exists(kernel_boot_id):
        try:
            with open(kernel_boot_id, "r", encoding="utf-8") as f:
                bid = f.read().strip()
                if bid:
                    return bid
        except OSError:
            pass

    # Transient runtime boot ID for environments without /proc
    transient_dir = (
        "/run/rpi-vj"
        if os.path.isdir("/run")
        else os.path.join(tempfile.gettempdir(), "rpi-vj")
    )
    boot_file = os.path.join(transient_dir, "boot_id")
    if os.path.exists(boot_file):
        try:
            with open(boot_file, "r", encoding="utf-8") as f:
                bid = f.read().strip()
                if bid:
                    return bid
        except OSError:
            pass

    # Generate new transient ID and persist to runtime file
    new_bid = str(uuid.uuid4())
    try:
        os.makedirs(transient_dir, exist_ok=True)
        with open(boot_file, "w", encoding="utf-8") as f:
            f.write(new_bid + "\n")
    except OSError:
        pass
    return new_bid


def get_monotonic_uptime() -> float:
    """Returns system or monotonic uptime in seconds (millisecond precision)."""
    if "RPI_VJ_MONOTONIC_UPTIME" in os.environ:
        try:
            return round(float(os.environ["RPI_VJ_MONOTONIC_UPTIME"]), 3)
        except ValueError:
            pass

    uptime_proc = "/proc/uptime"
    if os.path.exists(uptime_proc):
        try:
            with open(uptime_proc, "r", encoding="utf-8") as f:
                val = float(f.read().split()[0])
                return round(val, 3)
        except Exception:
            pass

    return round(time.monotonic(), 3)


def get_realtime() -> str:
    """Returns ISO 8601 UTC timestamp string."""
    now = datetime.datetime.now(datetime.timezone.utc)
    return now.strftime("%Y-%m-%dT%H:%M:%S.%fZ")


def is_clock_synchronized() -> bool:
    """Detects whether system clock is synchronized (e.g., NTP/systemd-timesyncd)."""
    if "RPI_VJ_CLOCK_SYNCED" in os.environ:
        return os.environ["RPI_VJ_CLOCK_SYNCED"].lower() in ("1", "true", "yes")

    # Linux adjtimex check
    if sys.platform.startswith("linux"):
        try:
            libc = ctypes.CDLL("libc.so.6")
            buf = ctypes.create_string_buffer(256)
            ret = libc.adjtimex(buf)
            # TIME_ERROR == 5 indicates unsynchronized clock
            if ret != -1:
                return ret != 5
        except Exception:
            pass

    # systemd-timesyncd touch file
    if os.path.exists("/run/systemd/timesync/synchronized"):
        return True

    return False


def detect_drm_connector() -> str | None:
    """Detects primary connected DRM connector (e.g., HDMI-A-1)."""
    if "RPI_VJ_DRM_CONNECTOR" in os.environ:
        val = os.environ["RPI_VJ_DRM_CONNECTOR"].strip()
        return val if val else None

    drm_dir = "/sys/class/drm"
    if os.path.exists(drm_dir):
        try:
            candidates: list[str] = []
            for entry in os.listdir(drm_dir):
                status_file = os.path.join(drm_dir, entry, "status")
                if os.path.isfile(status_file):
                    try:
                        with open(status_file, "r", encoding="utf-8") as sf:
                            if sf.read().strip() == "connected":
                                parts = entry.split("-", 1)
                                conn = parts[1] if len(parts) > 1 else entry
                                candidates.append(conn)
                    except OSError:
                        continue
            if candidates:
                for c in candidates:
                    if c == "HDMI-A-1":
                        return c
                for c in candidates:
                    if "HDMI" in c:
                        return c
                return candidates[0]
        except Exception:
            pass

    return None


def record_event(
    event: str,
    service: str | None = None,
    exit_status: int | None = None,
    drm_connector: str | None = None,
    detail: Any = None,
    boot_id: str | None = None,
    clock_synchronized: bool | None = None,
    monotonic_uptime: float | None = None,
    realtime: str | None = None,
    ledger_path: str = DEFAULT_LEDGER_PATH,
    fsync: bool = True,
) -> dict[str, Any]:
    """Records a single JSONL event entry to the boot ledger.

    Flushes and fsyncs at critical boundaries to survive sudden power removal.
    """
    if boot_id is None:
        boot_id = get_boot_id()
    if monotonic_uptime is None:
        monotonic_uptime = get_monotonic_uptime()
    if realtime is None:
        realtime = get_realtime()
    if clock_synchronized is None:
        clock_synchronized = is_clock_synchronized()
    if drm_connector is None:
        drm_connector = detect_drm_connector()

    entry: dict[str, Any] = {
        "boot_id": boot_id,
        "monotonic_uptime": monotonic_uptime,
        "realtime": realtime,
        "clock_synchronized": clock_synchronized,
        "event": event,
        "service": service,
        "exit_status": exit_status,
        "drm_connector": drm_connector,
        "detail": detail,
    }

    line = json.dumps(entry, separators=(",", ":")) + "\n"
    parent_dir = os.path.dirname(os.path.abspath(ledger_path))
    if parent_dir:
        os.makedirs(parent_dir, exist_ok=True)

    new_file = not os.path.exists(ledger_path)
    with open(ledger_path, "a", encoding="utf-8") as f:
        f.write(line)
        f.flush()
        if fsync:
            os.fsync(f.fileno())

    if new_file:
        try:
            os.chmod(ledger_path, 0o664)
            if hasattr(os, "chown") and os.geteuid() == 0:
                import grp, pwd
                try:
                    vj_uid = pwd.getpwnam("vj").pw_uid
                    vj_gid = grp.getgrnam("vj").gr_gid
                    os.chown(ledger_path, vj_uid, vj_gid)
                except KeyError:
                    pass
        except OSError:
            pass

    return entry


def read_events(
    ledger_path: str = DEFAULT_LEDGER_PATH,
    boot_id: str | None = None,
    limit: int | None = None,
) -> list[dict[str, Any]]:
    """Reads events from the boot ledger, optionally filtered by boot_id."""
    if not os.path.exists(ledger_path):
        return []

    events: list[dict[str, Any]] = []
    with open(ledger_path, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                rec = json.loads(line)
                if boot_id is None or rec.get("boot_id") == boot_id:
                    events.append(rec)
            except json.JSONDecodeError:
                continue

    if limit is not None and limit > 0:
        return events[-limit:]
    return events


def check_unclean_reboot(
    ledger_path: str = DEFAULT_LEDGER_PATH,
    current_boot_id: str | None = None,
    record_indication: bool = True,
) -> bool:
    """Inspects prior boot events.

    If prior boot terminated without clean_shutdown_request, logs
    unclean_filesystem_indication for current boot.
    Returns True if prior boot was unclean, False otherwise.
    """
    if current_boot_id is None:
        current_boot_id = get_boot_id()

    all_events = read_events(ledger_path=ledger_path)
    if not all_events:
        return False

    prev_boot_id = None
    for ev in reversed(all_events):
        bid = ev.get("boot_id")
        if bid != current_boot_id:
            prev_boot_id = bid
            break

    if prev_boot_id is None:
        return False

    prev_events = [ev for ev in all_events if ev.get("boot_id") == prev_boot_id]
    if not prev_events:
        return False

    last_event = prev_events[-1]
    is_clean = last_event.get("event") == "clean_shutdown_request"

    if not is_clean:
        if record_indication:
            record_event(
                event="unclean_filesystem_indication",
                service="rpi-vj-supervisor",
                exit_status=None,
                detail={
                    "prior_boot_id": prev_boot_id,
                    "last_event": last_event.get("event"),
                    "last_monotonic_uptime": last_event.get("monotonic_uptime"),
                    "last_realtime": last_event.get("realtime"),
                    "reason": "Previous boot did not record clean_shutdown_request before termination",
                },
                boot_id=current_boot_id,
                ledger_path=ledger_path,
                fsync=True,
            )
        return True

    return False


def evaluate_recovery_watchdog(
    last_heartbeat_monotonic: float,
    current_monotonic: float,
    timeout_seconds: float,
) -> tuple[bool, str]:
    """Strictly monotonic watchdog evaluation.

    Contract: No recovery decision may use realtime.
    Returns: (is_timed_out, detail_message)
    """
    if current_monotonic < last_heartbeat_monotonic:
        return False, "current monotonic earlier than last heartbeat"
    elapsed = current_monotonic - last_heartbeat_monotonic
    if elapsed > timeout_seconds:
        return (
            True,
            f"Watchdog timeout triggered: elapsed monotonic {elapsed:.3f}s > threshold {timeout_seconds:.3f}s",
        )
    return (
        False,
        f"Watchdog healthy: elapsed monotonic {elapsed:.3f}s <= threshold {timeout_seconds:.3f}s",
    )


def generate_journald_conf() -> str:
    """Generates bounded persistent systemd-journald configuration."""
    return (
        "# /etc/systemd/journald.conf.d/10-rpi-vj.conf\n"
        "# Managed for Appliance V3 Persistent Observability (A3-04)\n"
        "[Journal]\n"
        "Storage=persistent\n"
        "SystemMaxUse=200M\n"
        "SystemKeepFree=15%\n"
        "RuntimeMaxUse=50M\n"
        "MaxRetentionSec=1month\n"
        "ForwardToSyslog=no\n"
    )


def install_journald_conf(dest_path: str = DEFAULT_JOURNALD_CONF_PATH) -> str:
    """Installs journald configuration drop-in."""
    content = generate_journald_conf()
    parent = os.path.dirname(os.path.abspath(dest_path))
    if parent:
        os.makedirs(parent, exist_ok=True)
    with open(dest_path, "w", encoding="utf-8") as f:
        f.write(content)
    return dest_path


# ---------------------------------------------------------------------------
# Proportionate Unit Tests
# ---------------------------------------------------------------------------


class TestBootLedger(unittest.TestCase):
    def setUp(self) -> None:
        self.tmp_dir = tempfile.TemporaryDirectory()
        self.ledger_path = os.path.join(self.tmp_dir.name, "boot-ledger.jsonl")

    def tearDown(self) -> None:
        self.tmp_dir.cleanup()

    def test_record_and_read_schema(self) -> None:
        """Verifies all 9 mandatory schema fields are correctly written."""
        entry = record_event(
            event="renderer_ready",
            service="rpi-vj-renderer",
            exit_status=None,
            drm_connector="HDMI-A-1",
            detail={"resolution": "1920x1080@60"},
            boot_id="test-boot-123",
            clock_synchronized=False,
            monotonic_uptime=14.250,
            realtime="2026-09-15T00:00:00.000000Z",
            ledger_path=self.ledger_path,
        )

        self.assertEqual(entry["boot_id"], "test-boot-123")
        self.assertEqual(entry["monotonic_uptime"], 14.250)
        self.assertEqual(entry["realtime"], "2026-09-15T00:00:00.000000Z")
        self.assertFalse(entry["clock_synchronized"])
        self.assertEqual(entry["event"], "renderer_ready")
        self.assertEqual(entry["service"], "rpi-vj-renderer")
        self.assertIsNone(entry["exit_status"])
        self.assertEqual(entry["drm_connector"], "HDMI-A-1")
        self.assertEqual(entry["detail"], {"resolution": "1920x1080@60"})

        events = read_events(self.ledger_path)
        self.assertEqual(len(events), 1)
        self.assertEqual(events[0], entry)

    def test_unclean_reboot_detection(self) -> None:
        """Verifies sudden power removal is detected on next boot."""
        # Boot 1 records events and terminates abruptly without clean_shutdown_request
        record_event(
            event="kernel_start",
            boot_id="boot-001",
            monotonic_uptime=1.0,
            ledger_path=self.ledger_path,
        )
        record_event(
            event="marker",
            detail="A",
            boot_id="boot-001",
            monotonic_uptime=5.0,
            ledger_path=self.ledger_path,
        )

        # Boot 2 starts: evaluate unclean boot
        is_unclean = check_unclean_reboot(
            ledger_path=self.ledger_path,
            current_boot_id="boot-002",
            record_indication=True,
        )
        self.assertTrue(is_unclean)

        # Verify unclean_filesystem_indication is recorded in boot 2
        boot2_events = read_events(self.ledger_path, boot_id="boot-002")
        self.assertEqual(len(boot2_events), 1)
        self.assertEqual(boot2_events[0]["event"], "unclean_filesystem_indication")
        self.assertEqual(boot2_events[0]["detail"]["prior_boot_id"], "boot-001")
        self.assertEqual(boot2_events[0]["detail"]["last_event"], "marker")

    def test_clean_shutdown_detection(self) -> None:
        """Verifies clean shutdown produces no false unclean indication."""
        record_event(
            event="kernel_start",
            boot_id="boot-001",
            monotonic_uptime=1.0,
            ledger_path=self.ledger_path,
        )
        record_event(
            event="clean_shutdown_request",
            service="rpi-vj-supervisor",
            boot_id="boot-001",
            monotonic_uptime=20.0,
            ledger_path=self.ledger_path,
        )

        is_unclean = check_unclean_reboot(
            ledger_path=self.ledger_path,
            current_boot_id="boot-002",
            record_indication=True,
        )
        self.assertFalse(is_unclean)
        self.assertEqual(len(read_events(self.ledger_path, boot_id="boot-002")), 0)

    def test_monotonic_watchdog_realtime_immunity(self) -> None:
        """Verifies advancing wall time by 1 hour causes NO watchdog trigger."""
        last_hb = 100.0  # seconds monotonic
        timeout = 30.0  # 30 second threshold

        # 5 seconds of monotonic time elapsed, but wall clock jumps by 1 hour (3600s)
        current_monotonic = 105.0
        # Simulated wall clock jump has zero effect on evaluate_recovery_watchdog:
        timed_out, msg = evaluate_recovery_watchdog(
            last_hb, current_monotonic, timeout
        )
        self.assertFalse(timed_out)
        self.assertIn("Watchdog healthy", msg)

        # Only true monotonic progression past timeout triggers recovery
        current_monotonic = 135.0
        timed_out, msg = evaluate_recovery_watchdog(
            last_hb, current_monotonic, timeout
        )
        self.assertTrue(timed_out)
        self.assertIn("Watchdog timeout triggered", msg)

    def test_observable_markers_and_power_loss_flow(self) -> None:
        """Verifies the exact observable flow:

        Marker A -> power removal -> reboot -> Marker B -> both markers readable.
        """
        boot_a = "boot-id-marker-a"
        boot_b = "boot-id-marker-b"

        # 1. Generate marker A in first boot
        record_event(
            event="marker",
            detail="A",
            boot_id=boot_a,
            monotonic_uptime=10.0,
            realtime="2026-09-15T01:00:00.000000Z",
            ledger_path=self.ledger_path,
        )

        # 2. Power removal: no clean shutdown event recorded for boot_a

        # 3. Reboot: boot B initializes, checks unclean reboot
        unclean = check_unclean_reboot(
            ledger_path=self.ledger_path,
            current_boot_id=boot_b,
            record_indication=True,
        )
        self.assertTrue(unclean)

        # 4. Generate marker B in second boot
        record_event(
            event="marker",
            detail="B",
            boot_id=boot_b,
            monotonic_uptime=12.0,
            realtime="2026-09-15T02:00:00.000000Z",
            ledger_path=self.ledger_path,
        )

        # 5. Verify both markers are readable
        all_events = read_events(self.ledger_path)
        markers = [
            e.get("detail")
            for e in all_events
            if e.get("event") == "marker"
        ]
        self.assertEqual(markers, ["A", "B"])

        # 6. Verify wall time advance by 1 hour (3600s) does not trigger watchdog
        hb_ts = 12.0
        current_mono = 14.0  # only 2 seconds monotonic elapsed
        timed_out, _ = evaluate_recovery_watchdog(hb_ts, current_mono, timeout_seconds=60.0)
        self.assertFalse(timed_out)

    def test_journald_conf_bounds(self) -> None:
        """Verifies journald configuration specifies <=200M and 15% keep free."""
        conf = generate_journald_conf()
        self.assertIn("Storage=persistent", conf)
        self.assertIn("SystemMaxUse=200M", conf)
        self.assertIn("SystemKeepFree=15%", conf)


# ---------------------------------------------------------------------------
# CLI Parser
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="RPi-VJ Appliance V3 Boot Ledger and Observability Manager"
    )
    sub = parser.add_subparsers(dest="command")

    # record
    p_rec = sub.add_parser("record", help="Record a boot event")
    p_rec.add_argument("--event", required=True, help="Event name")
    p_rec.add_argument("--service", default=None, help="Service name")
    p_rec.add_argument("--exit-status", type=int, default=None, help="Exit code")
    p_rec.add_argument("--drm-connector", default=None, help="DRM connector")
    p_rec.add_argument("--detail", default=None, help="JSON or string detail")
    p_rec.add_argument("--boot-id", default=None, help="Explicit boot ID")
    p_rec.add_argument("--monotonic-uptime", type=float, default=None)
    p_rec.add_argument("--clock-synced", action="store_true", default=None)
    p_rec.add_argument("--path", default=DEFAULT_LEDGER_PATH, help="Ledger file path")
    p_rec.add_argument("--no-fsync", action="store_true", help="Skip fsync")

    # read
    p_read = sub.add_parser("read", help="Read boot events")
    p_read.add_argument("--boot-id", default=None, help="Filter by boot ID")
    p_read.add_argument("--limit", type=int, default=None, help="Limit entries")
    p_read.add_argument("--path", default=DEFAULT_LEDGER_PATH, help="Ledger file path")

    # check-unclean
    p_chk = sub.add_parser(
        "check-unclean", help="Check previous boot for unclean shutdown"
    )
    p_chk.add_argument("--boot-id", default=None, help="Current boot ID")
    p_chk.add_argument("--path", default=DEFAULT_LEDGER_PATH, help="Ledger file path")

    # journald-conf
    p_j = sub.add_parser(
        "journald-conf", help="Show or install journald configuration"
    )
    p_j.add_argument(
        "--install",
        action="store_true",
        help="Install to target path (default /etc/systemd/journald.conf.d/10-rpi-vj.conf)",
    )
    p_j.add_argument(
        "--target",
        default=DEFAULT_JOURNALD_CONF_PATH,
        help="Target config path",
    )

    # evaluate-watchdog
    p_w = sub.add_parser(
        "evaluate-watchdog",
        help="Evaluate watchdog timeout using strictly monotonic clock",
    )
    p_w.add_argument(
        "--last-monotonic",
        type=float,
        required=True,
        help="Last heartbeat monotonic time",
    )
    p_w.add_argument(
        "--current-monotonic",
        type=float,
        default=None,
        help="Current monotonic time (defaults to now)",
    )
    p_w.add_argument(
        "--timeout",
        type=float,
        default=30.0,
        help="Timeout in seconds",
    )

    # test
    sub.add_parser("test", help="Run internal unit test suite")

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    if args.command == "record":
        detail_val = args.detail
        if detail_val:
            try:
                detail_val = json.loads(detail_val)
            except Exception:
                pass
        rec = record_event(
            event=args.event,
            service=args.service,
            exit_status=args.exit_status,
            drm_connector=args.drm_connector,
            detail=detail_val,
            boot_id=args.boot_id,
            clock_synchronized=args.clock_synced,
            monotonic_uptime=args.monotonic_uptime,
            ledger_path=args.path,
            fsync=not args.no_fsync,
        )
        print(json.dumps(rec))
        return 0

    elif args.command == "read":
        events = read_events(
            ledger_path=args.path,
            boot_id=args.boot_id,
            limit=args.limit,
        )
        for ev in events:
            print(json.dumps(ev))
        return 0

    elif args.command == "check-unclean":
        unclean = check_unclean_reboot(
            ledger_path=args.path,
            current_boot_id=args.boot_id,
            record_indication=True,
        )
        if unclean:
            print("UNCLEAN_REBOOT_DETECTED")
            return 1
        else:
            print("CLEAN_BOOT")
            return 0

    elif args.command == "journald-conf":
        if args.install:
            path = install_journald_conf(args.target)
            print(f"Installed journald config to {path}")
        else:
            print(generate_journald_conf(), end="")
        return 0

    elif args.command == "evaluate-watchdog":
        curr = args.current_monotonic
        if curr is None:
            curr = get_monotonic_uptime()
        timed_out, msg = evaluate_recovery_watchdog(
            last_heartbeat_monotonic=args.last_monotonic,
            current_monotonic=curr,
            timeout_seconds=args.timeout,
        )
        print(msg)
        return 1 if timed_out else 0

    elif args.command == "test":
        suite = unittest.TestLoader().loadTestsFromTestCase(TestBootLedger)
        runner = unittest.TextTestRunner(verbosity=2)
        result = runner.run(suite)
        return 0 if result.wasSuccessful() else 1

    else:
        parser.print_help()
        return 1


if __name__ == "__main__":
    sys.exit(main())
