#!/usr/bin/env python3
"""Supervise `rpi-vj-native --bench` on the Pi (Python 3 standard library only).

The engine measures; this script keeps a long run alive and honest:

* refuses to start while the live services hold the display (unless --stop-services);
* relaunches the engine with --resume until every manifest entry has a result;
* no new results line for --hang-seconds: stop the engine and record `hang` for the
  entry whose `begin` has no `result`; an unexpected exit records `crash` the same way;
* stopping is SIGTERM first (the engine leaves at a frame boundary) and SIGKILL only
  after --term-seconds, because killing a process inside a V3D render job has produced
  a kernel oops on this platform; a new kernel oops aborts the run;
* an entry gets at most one supervisor result, and --max-strikes engine deaths in a
  row without a single engine-written result abort the run;
* pauses the engine between entries above --pause-above C until below --resume-below C;
* --restart-services starts the services again on every way out, including signals.

Detached run that survives the SSH session (root, so it may own DRM and systemctl):

  sudo systemd-run --unit=rpi-vj-bench --collect \
      -p StandardOutput=append:/var/lib/rpi-vj-bench/supervisor.log \
      -p StandardError=append:/var/lib/rpi-vj-bench/supervisor.log \
      /usr/bin/python3 /opt/rpi-vj-bench/pi_bench.py /var/lib/rpi-vj-bench/manifest.json \
      --stop-services --restart-services

  systemctl status rpi-vj-bench          # progress
  tail -f /var/lib/rpi-vj-bench/supervisor.log
  sudo systemctl stop rpi-vj-bench       # stop early; services are restored
"""
import argparse
import fcntl
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

SERVICES = ("rpi-vj-native", "rpi-vj-controller")
DEFAULT_BIN = "/opt/rpi-vj-bench/rpi-vj-native"


class Stop(Exception):
    """A termination signal arrived."""


def log(message):
    print(time.strftime("%H:%M:%S"), message, flush=True)


def systemctl(*args):
    command = ["systemctl", *args]
    if os.geteuid() != 0:
        command = ["sudo", "-n", *command]
    return subprocess.run(command, capture_output=True, text=True)


def active(service):
    return subprocess.run(["systemctl", "is-active", "--quiet", service]).returncode == 0


def vcgencmd(*args):
    try:
        done = subprocess.run(["vcgencmd", *args], capture_output=True, text=True, timeout=5)
        return done.stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return ""


def temperature():
    """SoC temperature in C, or None when neither source is readable."""
    text = vcgencmd("measure_temp")  # temp=54.9'C
    try:
        return float(text.split("=", 1)[1].split("'", 1)[0])
    except (IndexError, ValueError):
        pass
    try:
        return int(Path("/sys/class/thermal/thermal_zone0/temp").read_text()) / 1000.0
    except (OSError, ValueError):
        return None


def throttled():
    text = vcgencmd("get_throttled")  # throttled=0x0
    return text.split("=", 1)[1] if "=" in text else None


def read_results(path):
    """Return (keys with a result, key of the last begin without a result, engine result
    count, keys holding a supervisor result). Torn or foreign lines are ignored."""
    done, supervised, pending, engine_results = set(), set(), None, 0
    try:
        text = Path(path).read_text(errors="replace")
    except FileNotFoundError:
        return done, pending, engine_results, supervised
    for line in text.splitlines():
        try:
            row = json.loads(line)
            key = (row["id"], row["variant"])
            event = row["event"]
        except (ValueError, KeyError, TypeError):
            continue
        if event == "begin":
            pending = key
        elif event == "result":
            done.add(key)
            if row.get("status") in ("crash", "hang"):
                supervised.add(key)
            else:
                engine_results += 1
            if pending == key:
                pending = None
    if pending in done:
        pending = None
    return done, pending, engine_results, supervised


def supervisor_result(key, status, error):
    return json.dumps({
        "event": "result", "id": key[0], "variant": key[1], "status": status,
        "error": " ".join(error.split())[:400],
        "temp_c": temperature(), "throttled": throttled(),
    }, separators=(",", ":"))


def append_line(path, line):
    with open(path, "a+b") as handle:
        handle.seek(0, os.SEEK_END)
        if handle.tell():
            handle.seek(-1, os.SEEK_END)
            if handle.read(1) != b"\n":
                handle.write(b"\n")  # a line torn by the dead engine
        handle.write(line.encode() + b"\n")
        handle.flush()
        os.fsync(handle.fileno())


def last_phase(log_path):
    """Last engine stderr line, to say where a hang or crash happened."""
    try:
        with open(log_path, "rb") as handle:
            handle.seek(0, os.SEEK_END)
            handle.seek(max(0, handle.tell() - 2000))
            lines = handle.read().decode(errors="replace").strip().splitlines()
        return lines[-1].strip() if lines else ""
    except OSError:
        return ""


def kernel_oopses():
    """Number of kernel oopses since boot, or None when the kernel log is not readable."""
    try:
        done = subprocess.run(["dmesg"], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return None
    if done.returncode != 0:
        return None
    return sum(1 for line in done.stdout.splitlines() if "Internal error: Oops" in line)


def stop_process(process, grace):
    """SIGTERM, then SIGKILL after `grace` seconds. Returns True when SIGKILL was needed."""
    if process.poll() is not None:
        return False
    process.terminate()
    try:
        process.wait(timeout=grace)
        return False
    except subprocess.TimeoutExpired:
        process.kill()  # blocked inside one GL call for the whole grace period
        process.wait()
        return True


def missing_textures(manifest):
    """Channel texture files the manifest names that are not on disk, sorted. The engine
    fails just those entries with `prepare_error`; saying so up front saves a wasted run.
    A cubemap binding names its first face; the others are `<stem>_1` .. `<stem>_5`."""
    missing = set()
    for entry in manifest.get("entries", []):
        for channel in entry.get("channels") or []:
            first = Path(channel["path"])
            faces = [first]
            if channel.get("kind") == "cube":
                faces += [first.with_name(f"{first.stem}_{face}{first.suffix}") for face in range(1, 6)]
            missing.update(str(path) for path in faces if not path.is_file())
    return sorted(missing)


class Supervisor:
    def __init__(self, args, manifest):
        self.args = args
        self.out = Path(manifest["out"])
        self.keys = [(e["id"], e["variant"]) for e in manifest["entries"]]
        self.pause_file = Path(str(self.out) + ".pause")
        self.engine_log = Path(args.engine_log or str(self.out) + ".engine.log")
        self.process = None
        self.reported = 0
        self.oopses = kernel_oopses()

    def remaining(self):
        done = read_results(self.out)[0]
        return [key for key in self.keys if key not in done]

    def report_progress(self):
        done = read_results(self.out)[0]
        count = sum(1 for key in self.keys if key in done)
        if count != self.reported:
            self.reported = count
            log(f"progress {count}/{len(self.keys)} temp={temperature()}C throttled={throttled()}")

    def thermal(self):
        temp = temperature()
        if temp is None:
            return
        if not self.pause_file.exists() and temp > self.args.pause_above:
            self.pause_file.touch()
            log(f"temperature {temp:.1f} C above {self.args.pause_above}: pausing between entries")
        elif self.pause_file.exists() and temp < self.args.resume_below:
            self.pause_file.unlink()
            log(f"temperature {temp:.1f} C below {self.args.resume_below}: resuming")

    def launch_once(self):
        """Run the engine until it exits or stalls. Returns (cause, detail); cause is
        None for a clean exit, else 'hang' or 'crash'."""
        environment = dict(os.environ)
        environment.setdefault("SDL_VIDEODRIVER", "kmsdrm")
        environment.setdefault("SDL_AUDIODRIVER", "dummy")
        with open(self.engine_log, "ab") as engine_log:
            self.process = subprocess.Popen(
                [self.args.bin, "--bench", self.args.manifest, "--resume"],
                stdin=subprocess.DEVNULL, stdout=engine_log, stderr=engine_log, env=environment)
        size = self.out.stat().st_size if self.out.exists() else 0
        started = False  # the engine wrote its first line after start-up calibration
        activity = time.monotonic()
        thermal_check = 0.0
        while True:
            time.sleep(0.25)
            now = time.monotonic()
            current = self.out.stat().st_size if self.out.exists() else 0
            if current != size:
                # A lone newline is the engine terminating a torn line, not an entry.
                started = started or current - size > 1
                size, activity = current, now
                self.report_progress()
            if now - thermal_check >= 5:
                thermal_check = now
                self.thermal()
            code = self.process.poll()
            if code is not None:
                self.process = None
                if code == 0:
                    return None, ""
                return "crash", f"engine exit status {code}; last: {last_phase(self.engine_log)}"
            if self.pause_file.exists() and read_results(self.out)[1] is None:
                activity = now  # idle on purpose while cooling down
                continue
            limit = self.args.hang_seconds if started else self.args.startup_seconds
            if now - activity > limit:
                phase = last_phase(self.engine_log)  # before the engine logs its exit
                killed = stop_process(self.process, self.args.term_seconds)
                self.process = None
                if killed:
                    log("warning: engine ignored SIGTERM and was SIGKILLed; if a render job "
                        "was in flight the kernel may misbehave (see README)")
                    time.sleep(5)  # let the GPU scheduler retire whatever was queued
                return "hang", f"no output for {limit:.0f} s; last: {phase}"

    def run(self):
        strikes = 0
        launches = 0
        while True:
            todo = self.remaining()
            if not todo:
                return 0
            launches += 1
            before = read_results(self.out)[2]
            log(f"launch {launches}: {len(todo)} of {len(self.keys)} entries to measure")
            cause, detail = self.launch_once()
            _, pending, after, supervised = read_results(self.out)
            if cause and pending and pending not in supervised:
                append_line(self.out, supervisor_result(pending, cause, detail))
                log(f"{cause}: {pending[0]} {pending[1]} ({detail})")
            elif cause:
                log(f"engine {cause} outside an entry ({detail})")
            self.report_progress()
            oopses = kernel_oopses()
            if oopses is not None and self.oopses is not None and oopses > self.oopses:
                log("abort: the kernel logged an oops during this run (dmesg); results so far "
                    "are kept, reboot before continuing")
                return 4
            if after > before:
                strikes = 0
            else:
                strikes += 1
                if not self.remaining():
                    return 0
                if strikes >= self.args.max_strikes:
                    log(f"abort: engine died {strikes} times in a row without measuring anything; "
                        f"see {self.engine_log}")
                    return 3
                time.sleep(2)

    def cleanup(self):
        if self.process is not None:
            stop_process(self.process, self.args.term_seconds)
            self.process = None
        try:
            self.pause_file.unlink()
        except FileNotFoundError:
            pass


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("manifest", help="bench manifest (plan section 3.1)")
    parser.add_argument("--bin", default=DEFAULT_BIN, help=f"bench engine binary (default {DEFAULT_BIN})")
    parser.add_argument("--stop-services", action="store_true",
                        help="stop the live renderer/controller instead of refusing to run")
    parser.add_argument("--restart-services", action="store_true",
                        help="start the live services again on exit, error or signal")
    parser.add_argument("--fresh", action="store_true",
                        help="move an existing results file aside instead of resuming it")
    parser.add_argument("--hang-seconds", type=float, default=60.0)
    parser.add_argument("--startup-seconds", type=float, default=300.0,
                        help="allowance for engine start-up, which waits for an idle GPU")
    parser.add_argument("--term-seconds", type=float, default=30.0,
                        help="wait after SIGTERM for the engine to leave at a frame boundary")
    parser.add_argument("--max-strikes", type=int, default=3)
    parser.add_argument("--pause-above", type=float, default=80.0)
    parser.add_argument("--resume-below", type=float, default=70.0)
    parser.add_argument("--engine-log", help="engine stderr log (default <out>.engine.log)")
    args = parser.parse_args()

    with open(args.manifest) as handle:
        manifest = json.load(handle)
    if not os.access(args.bin, os.X_OK):
        raise SystemExit(f"missing bench binary {args.bin}")
    running = [service for service in SERVICES if active(service)]
    if running and not args.stop_services:
        raise SystemExit(f"refusing: {', '.join(running)} active (they own the display); "
                         "stop them or pass --stop-services")
    absent = missing_textures(manifest)
    if absent:
        log(f"warning: {len(absent)} channel texture files are missing; entries that bind them will be "
            f"prepare_error: {', '.join(absent[:5])}{' ...' if len(absent) > 5 else ''}")
    supervisor = Supervisor(args, manifest)
    supervisor.out.parent.mkdir(parents=True, exist_ok=True)
    lock = open(str(supervisor.out) + ".lock", "w")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        raise SystemExit(f"refusing: another supervisor holds {lock.name}")

    def on_signal(number, _frame):
        raise Stop(signal.Signals(number).name)

    for number in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
        signal.signal(number, on_signal)
    code = 1
    try:
        if running:
            log(f"stopping {' '.join(running)}")
            done = systemctl("stop", *running)
            if done.returncode != 0:
                raise SystemExit(f"cannot stop services: {done.stderr.strip()}")
        if args.fresh and supervisor.out.exists():
            aside = f"{supervisor.out}.{time.strftime('%Y%m%d-%H%M%S')}.bak"
            supervisor.out.rename(aside)
            log(f"previous results moved to {aside}")
        supervisor.cleanup()  # a stale pause file would stall the engine
        code = supervisor.run()
        done, _, _, _ = read_results(supervisor.out)
        log(f"finished: {sum(1 for key in supervisor.keys if key in done)}/{len(supervisor.keys)} "
            f"entries have results in {supervisor.out}")
    except Stop as stop:
        log(f"stopped by {stop}")
        code = 130
    finally:
        # Block further signals so the restore below always completes.
        for number in (signal.SIGTERM, signal.SIGINT, signal.SIGHUP):
            signal.signal(number, signal.SIG_IGN)
        supervisor.cleanup()
        if args.restart_services:
            done = systemctl("start", *SERVICES)
            state = " ".join(f"{s}={'active' if active(s) else 'INACTIVE'}" for s in SERVICES)
            log(f"services restarted: {state} {done.stderr.strip()}")
    return code


if __name__ == "__main__":
    sys.exit(main())
