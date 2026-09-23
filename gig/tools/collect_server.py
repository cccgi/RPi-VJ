#!/usr/bin/env python3
"""Static file server for the show + a POST /collect endpoint that writes
probe results to ~/vj-results/<name>.log. Drop-in replacement for
`python3 -m http.server 8080`."""
import http.server, os, socketserver, urllib.parse, datetime, subprocess, threading, time, json, re
from mpv_player import MpvPlayer, MpvError

ROOT = os.getcwd()
OUT = os.path.expanduser("~/vj-results")
STATE_FILE = os.path.expanduser("~/vj-state.json")
MUSIC_ROOT = os.path.realpath(os.path.join(ROOT, "gig", "media", "music"))
MUSIC = MpvPlayer(MUSIC_ROOT)
os.makedirs(OUT, exist_ok=True)
_write_lock = threading.Lock()
_heartbeat_lock = threading.Lock()
_last_heartbeat = [None]
_watchdog_enabled = True
WATCHDOG_TIMEOUT_SEC = 15.0

def _restart_kiosk(reason="watchdog timeout"):
    logp = os.path.join(OUT, "watchdog.log")
    syslogp = os.path.join(OUT, "system.log")
    now_str = datetime.datetime.now().isoformat(timespec="seconds")
    lines = ["== %s  WATCHDOG RESTART: %s" % (now_str, reason)]
    variants = H.ACTIONS.get("restart-kiosk", [])
    ok = False
    for cmd in variants:
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=8)
            lines.append("   %s -> rc=%s %s%s" % (" ".join(cmd), r.returncode,
                         (r.stdout or "").strip()[:160], (r.stderr or "").strip()[:200]))
            if r.returncode == 0:
                ok = True
                break
        except FileNotFoundError as e:
            lines.append("   %s -> NOT FOUND (%s)" % (" ".join(cmd), e))
        except Exception as e:
            lines.append("   %s -> %s: %s" % (" ".join(cmd), type(e).__name__, e))
    lines.append("   RESULT: %s" % ("OK" if ok else "ALL VARIANTS FAILED"))
    log_text = "\n".join(lines) + "\n"
    with _write_lock:
        with open(logp, "a", encoding="utf-8") as f:
            f.write(log_text)
        with open(syslogp, "a", encoding="utf-8") as f:
            f.write(log_text)
    for l in lines:
        print(l, flush=True)

def _kiosk_is_active():
    """Separate probe so tests can model an active kiosk without systemd."""
    try:
        r = subprocess.run(["/usr/bin/systemctl", "--user", "is-active", "vj-kiosk"],
                           capture_output=True, text=True, timeout=5)
        return (r.stdout or "").strip() == "active"
    except Exception:
        return False

def watchdog_loop(timeout=WATCHDOG_TIMEOUT_SEC, check_interval=1.0):
    global _last_heartbeat, _watchdog_enabled
    startup_grace = 30.0
    start_time = time.time()
    was_enabled = True

    # CRITICAL: this loop must NEVER exit.
    #
    # It used to be `while _watchdog_enabled:`, so the first POST to
    # /system/watchdog-off ended the thread permanently. /system/watchdog-on
    # then set the flag back with no thread left to read it, and every probe
    # run therefore disarmed the watchdog for the rest of the server's life.
    # A Chromium crash after that point sat on screen forever -- which is
    # exactly the idle crash seen on the TV, with nothing left to recover it.
    while True:
        time.sleep(check_interval)

        if not _watchdog_enabled:
            was_enabled = False
            continue

        if not was_enabled:
            # Just re-armed: treat now as the baseline so a probe that ran for
            # several minutes does not trigger an instant restart.
            was_enabled = True
            start_time = time.time()
            with _heartbeat_lock:
                _last_heartbeat[0] = time.time()
            print("== watchdog re-armed", flush=True)
            continue

        now = time.time()
        with _heartbeat_lock:
            last = _last_heartbeat[0]
        if last is None:
            if (now - start_time) > startup_grace:
                _restart_kiosk("no initial heartbeat received after %.1fs startup grace" % startup_grace)
                with _heartbeat_lock:
                    _last_heartbeat[0] = time.time()
            continue

        elapsed = now - last
        if elapsed > timeout:
            # ONLY restart a kiosk that is SUPPOSED to be running. run-probe.sh
            # and setup-pair.sh stop vj-kiosk deliberately; without this check
            # the watchdog fought them, restarting the kiosk every ~20s while a
            # probe was running and leaving two Chromiums competing for the GPU.
            if not _kiosk_is_active():
                # deliberately stopped — stand down and stop counting
                with _heartbeat_lock:
                    _last_heartbeat[0] = time.time()
                continue
            _restart_kiosk("no heartbeat received for %.1fs (threshold %.1fs)" % (elapsed, timeout))
            with _heartbeat_lock:
                _last_heartbeat[0] = time.time()

class H(http.server.SimpleHTTPRequestHandler):
    def send_head(self):
        """Serve a single HTTP byte range for seekable local media.

        Chromium requires a 206 response and Content-Range to seek without
        downloading a multi-hundred-megabyte MP3 from byte zero. Python's
        SimpleHTTPRequestHandler returns 200 for Range requests, so provide
        the bounded response here while retaining its normal path handling.
        """
        range_header = self.headers.get("Range")
        if not range_header:
            return super().send_head()

        path = self.translate_path(urllib.parse.urlparse(self.path).path)
        if not os.path.isfile(path):
            return super().send_head()
        try:
            size = os.path.getsize(path)
            match = re.fullmatch(r"bytes=(\d*)-(\d*)", range_header.strip())
            if not match or (not match.group(1) and not match.group(2)):
                raise ValueError("unsupported byte range")
            if match.group(1):
                start = int(match.group(1))
                end = int(match.group(2)) if match.group(2) else size - 1
            else:
                suffix = int(match.group(2))
                if suffix <= 0:
                    raise ValueError("invalid suffix range")
                start = max(0, size - suffix)
                end = size - 1
            if start >= size or start < 0 or end < start:
                self.send_response(416)
                self.send_header("Content-Range", "bytes */%d" % size)
                self.send_header("Content-Length", "0")
                self.end_headers()
                return None
            end = min(end, size - 1)
            source = open(path, "rb")
            source.seek(start)
            self._range_remaining = end - start + 1
            self.send_response(206)
            self.send_header("Content-Type", self.guess_type(path))
            self.send_header("Accept-Ranges", "bytes")
            self.send_header("Content-Range", "bytes %d-%d/%d" % (start, end, size))
            self.send_header("Content-Length", str(self._range_remaining))
            self.send_header("Last-Modified", self.date_time_string(os.path.getmtime(path)))
            self.end_headers()
            return source
        except ValueError:
            self.send_response(416)
            self.send_header("Content-Range", "bytes */%d" % os.path.getsize(path))
            self.send_header("Content-Length", "0")
            self.end_headers()
            return None
        except OSError:
            self.send_error(404, "File not found")
            return None

    def copyfile(self, source, outputfile):
        remaining = getattr(self, "_range_remaining", None)
        if remaining is None:
            return super().copyfile(source, outputfile)
        try:
            while remaining > 0:
                chunk = source.read(min(128 * 1024, remaining))
                if not chunk:
                    break
                outputfile.write(chunk)
                remaining -= len(chunk)
        finally:
            self._range_remaining = None

    def end_headers(self):
        # NO CACHING, EVER.
        #
        # SimpleHTTPRequestHandler sends Last-Modified and no Cache-Control, so
        # Chromium applies heuristic caching to the ES modules. The Pi could
        # therefore hold files that hash-verify as current while the kiosk kept
        # executing an older mixer.js / audio.js -- a fix that is deployed,
        # verified, and still not running. That cost several rounds of chasing
        # phantom bugs. The show is served from localhost; caching buys nothing.
        self.send_header("Cache-Control", "no-store, no-cache, must-revalidate, max-age=0")
        self.send_header("Pragma", "no-cache")
        self.send_header("Expires", "0")
        self.send_header("Accept-Ranges", "bytes")
        super().end_headers()

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/state":
            return self._get_state()
        # Readable with a plain `curl` -- a health check you need a -X POST to
        # reach is a health check nobody runs.
        if parsed.path == "/system/watchdog-status":
            return self._watchdog_status()
        if parsed.path == "/system/telemetry":
            return self._telemetry()
        if parsed.path == "/api/music/status":
            return self._music_status()
        return super().do_GET()

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        if parsed.path == "/system/watchdog-off":
            return self._watchdog_toggle(False)
        if parsed.path == "/system/watchdog-on":
            return self._watchdog_toggle(True)
        if parsed.path == "/system/watchdog-status":
            return self._watchdog_status()
        if parsed.path.startswith("/system/"):
            return self._system(parsed.path.rsplit("/", 1)[-1])
        if parsed.path == "/heartbeat":
            return self._heartbeat()
        if parsed.path == "/state":
            return self._save_state()
        if parsed.path == "/api/music/play":
            return self._music_play()
        if parsed.path == "/api/music/toggle-pause":
            return self._music_control("pause")
        if parsed.path == "/api/music/toggle-mute":
            return self._music_control("mute")
        if parsed.path == "/api/music/seek":
            return self._music_seek()
        if parsed.path == "/api/music/volume":
            return self._music_volume()
        if parsed.path != "/collect":
            self.send_error(404); return
        qs = urllib.parse.parse_qs(parsed.query)
        name = (qs.get("name", ["probe"])[0] or "probe")
        name = "".join(c for c in name if c.isalnum() or c in "-_.")[:64] or "probe"
        n = int(self.headers.get("Content-Length", 0) or 0)
        body = self.rfile.read(n).decode("utf-8", "replace")
        path = os.path.join(OUT, name + ".log")
        with _write_lock:
            with open(path, "w", encoding="utf-8") as f:
                f.write("# collected %s\n%s\n" % (datetime.datetime.now().isoformat(timespec="seconds"), body))
        self.send_response(204); self.end_headers()
        print("== wrote %s (%d bytes)" % (path, len(body)), flush=True)

    def _json_response(self, status, data):
        body = json.dumps(data).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _read_json(self, limit=8192):
        n = int(self.headers.get("Content-Length", 0) or 0)
        if n <= 0 or n > limit:
            raise ValueError("invalid request size")
        return json.loads(self.rfile.read(n).decode("utf-8", "replace"))

    def _music_status(self):
        try:
            return self._json_response(200, MUSIC.status())
        except Exception as exc:
            return self._json_response(503, {"error": str(exc)})

    def _music_play(self):
        try:
            data = self._read_json()
            return self._json_response(200, MUSIC.play(data.get("src")))
        except (ValueError, json.JSONDecodeError, MpvError) as exc:
            return self._json_response(400, {"error": str(exc)})
        except Exception as exc:
            return self._json_response(503, {"error": str(exc)})

    def _music_control(self, action):
        try:
            state = MUSIC.toggle_pause() if action == "pause" else MUSIC.toggle_mute()
            return self._json_response(200, state)
        except MpvError as exc:
            return self._json_response(503, {"error": str(exc)})

    def _music_seek(self):
        try:
            data = self._read_json()
            return self._json_response(200, MUSIC.seek_forward(data.get("seconds")))
        except (ValueError, TypeError, json.JSONDecodeError, MpvError) as exc:
            return self._json_response(400, {"error": str(exc)})

    def _music_volume(self):
        try:
            data = self._read_json()
            return self._json_response(200, MUSIC.change_volume(data.get("delta")))
        except (ValueError, TypeError, json.JSONDecodeError, MpvError) as exc:
            return self._json_response(400, {"error": str(exc)})

    def _heartbeat(self):
        global _last_heartbeat
        now = time.time()
        with _heartbeat_lock:
            _last_heartbeat[0] = now
        n = int(self.headers.get("Content-Length", 0) or 0)
        if n > 0:
            try:
                body = self.rfile.read(n).decode("utf-8", "replace")
                data = json.loads(body)
                if isinstance(data, dict):
                    with _write_lock:
                        with open(STATE_FILE, "w", encoding="utf-8") as f:
                            json.dump(data, f, indent=2)
            except Exception:
                pass
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(b'{"status":"ok"}\n')

    def _save_state(self):
        n = int(self.headers.get("Content-Length", 0) or 0)
        if n > 0:
            try:
                body = self.rfile.read(n).decode("utf-8", "replace")
                data = json.loads(body)
                with _write_lock:
                    with open(STATE_FILE, "w", encoding="utf-8") as f:
                        json.dump(data, f, indent=2)
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(b'{"status":"saved"}\n')
                return
            except Exception as e:
                self.send_error(400, "invalid json: %s" % e)
                return
        self.send_error(400, "empty body")

    def _get_state(self):
        data = "{}"
        if os.path.exists(STATE_FILE):
            try:
                with open(STATE_FILE, "r", encoding="utf-8") as f:
                    data = f.read()
            except Exception:
                data = "{}"
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(data.encode("utf-8"))

    # 2026-08-27: bottom-HUD telemetry, added chasing the idle/Streamdeck-press
    # OOM crashes. All /sys/class/thermal zones (Pi 5 exposes several -- CPU
    # is not the only one) plus PMIC current/voltage via vcgencmd, which is
    # how ADR-100 said to get this data (no nvml-wrapper/IOKit-style crate,
    # shell out and join on wall time -- same policy applies here to vcgencmd).
    # GET, not POST: read-only, and the HUD polls this every couple seconds.
    def _telemetry(self):
        zones = []
        try:
            base = "/sys/class/thermal"
            for name in sorted(os.listdir(base)):
                if not name.startswith("thermal_zone"):
                    continue
                zdir = os.path.join(base, name)
                try:
                    with open(os.path.join(zdir, "temp")) as f:
                        milli_c = int(f.read().strip())
                    with open(os.path.join(zdir, "type")) as f:
                        ztype = f.read().strip()
                    zones.append({"zone": name, "type": ztype, "temp_c": round(milli_c / 1000.0, 1)})
                except Exception:
                    continue
        except Exception:
            pass

        power = {"volts": None, "amps": None, "watts": None, "raw": None}
        try:
            # vcgencmd pmic_read_adc dumps every rail; EXT5V is the one that
            # reflects total board draw on a Pi 5. Format per line:
            # "EXT5V_V volt(0)=5.10380000"  /  "EXT5V_A current(0)=0.83223000"
            r = subprocess.run(["/usr/bin/vcgencmd", "pmic_read_adc"],
                                capture_output=True, text=True, timeout=5)
            raw = (r.stdout or "").strip()
            power["raw"] = raw
            v = a = None
            for line in raw.splitlines():
                line = line.strip()
                if line.startswith("EXT5V_V") and "=" in line:
                    v = float(line.split("=", 1)[1])
                elif line.startswith("EXT5V_A") and "=" in line:
                    a = float(line.split("=", 1)[1])
            if v is not None:
                power["volts"] = round(v, 3)
            if a is not None:
                power["amps"] = round(a, 3)
            if v is not None and a is not None:
                power["watts"] = round(v * a, 2)
        except Exception as e:
            power["error"] = str(e)

        throttled = None
        try:
            r = subprocess.run(["/usr/bin/vcgencmd", "get_throttled"],
                                capture_output=True, text=True, timeout=5)
            throttled = (r.stdout or "").strip()
        except Exception:
            pass

        body = json.dumps({
            "ts": datetime.datetime.now().isoformat(timespec="seconds"),
            "zones": zones,
            "power": power,
            "throttled": throttled,
        })
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(body.encode())

    # Only these three actions, never a shell string. Bound to 127.0.0.1 only.
    # ABSOLUTE paths: a systemd --user service has a minimal PATH, so a bare
    # "sudo" or "systemctl" may not resolve and Popen raises FileNotFoundError.
    def _watchdog_status(self):
        """Is the watchdog armed, is its thread alive, and how stale is the
        heartbeat? A watchdog you cannot inspect is a watchdog you cannot
        trust -- this one silently died and nobody noticed for days."""
        with _heartbeat_lock:
            last = _last_heartbeat[0]
        alive = any(t.name == "vj-watchdog" and t.is_alive()
                    for t in threading.enumerate())
        age = None if last is None else round(time.time() - last, 1)
        fresh = last is not None and age is not None and age <= WATCHDOG_TIMEOUT_SEC
        body = json.dumps({
            "enabled": bool(_watchdog_enabled),
            "thread_alive": alive,
            "heartbeat_age_sec": age,
            "heartbeat_fresh": fresh,
            "healthy": bool(_watchdog_enabled and alive and fresh),
        })
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(body.encode())

    def _watchdog_toggle(self, on):
        global _watchdog_enabled
        _watchdog_enabled = bool(on)
        with _heartbeat_lock:
            _last_heartbeat[0] = time.time()
        print("== watchdog %s" % ("ENABLED" if on else "SUSPENDED"), flush=True)
        self.send_response(200); self.end_headers()
        self.wfile.write(b"OK")

    ACTIONS = {
        "shutdown": [["/usr/bin/sudo", "-n", "/sbin/shutdown", "-h", "now"],
                     ["/usr/bin/sudo", "-n", "/usr/sbin/shutdown", "-h", "now"],
                     ["/usr/bin/systemctl", "poweroff"]],
        "reboot":   [["/usr/bin/sudo", "-n", "/sbin/reboot"],
                     ["/usr/bin/sudo", "-n", "/usr/sbin/reboot"],
                     ["/usr/bin/systemctl", "reboot"]],
        "restart-kiosk": [["/usr/bin/systemctl", "--user", "restart", "vj-kiosk"]],
        # Force HDMI output to 1080p60. Pi5 Wayland uses wlr-randr; X11 fallback uses xrandr.
        # Output name is HDMI-A-1 on most Pi5 Wayland configs; try -A-2 as secondary fallback.
        "hdmi-1080p": [
            ["/usr/bin/wlr-randr", "--output", "HDMI-A-1", "--mode", "1920x1080@60"],
            ["/usr/bin/wlr-randr", "--output", "HDMI-A-2", "--mode", "1920x1080@60"],
            ["/usr/bin/xrandr", "--output", "HDMI-1", "--mode", "1920x1080"],
        ],
        # 2026-08-27: the former default. Now opt-in only, via the button that
        # used to say 1080p -- see index.html act:'hdmi4k'. Do not make this
        # the boot default again without re-benching idle memory on the 2GB
        # Pi; that is the leading suspect behind the idle/Streamdeck-press
        # OOM crashes (see vj-kiosk.service's 1080p ExecStartPre comment).
        "hdmi-4k": [
            ["/usr/bin/wlr-randr", "--output", "HDMI-A-1", "--mode", "3840x2160@60"],
            ["/usr/bin/wlr-randr", "--output", "HDMI-A-2", "--mode", "3840x2160@60"],
            ["/usr/bin/xrandr", "--output", "HDMI-1", "--mode", "3840x2160"],
        ],
    }

    def _system(self, action):
        variants = self.ACTIONS.get(action)
        if not variants:
            self.send_error(400, "unknown action"); return

        logp = os.path.join(OUT, "system.log")
        lines = ["== %s  action=%s" % (datetime.datetime.now().isoformat(timespec="seconds"), action)]
        ok = False
        for cmd in variants:
            try:
                r = subprocess.run(cmd, capture_output=True, text=True, timeout=8)
                lines.append("   %s -> rc=%s %s%s" % (" ".join(cmd), r.returncode,
                             (r.stdout or "").strip()[:160], (r.stderr or "").strip()[:200]))
                if r.returncode == 0:
                    ok = True
                    break
            except FileNotFoundError as e:
                lines.append("   %s -> NOT FOUND (%s)" % (" ".join(cmd), e))
            except Exception as e:
                lines.append("   %s -> %s: %s" % (" ".join(cmd), type(e).__name__, e))
        lines.append("   RESULT: %s" % ("OK" if ok else "ALL VARIANTS FAILED"))
        with _write_lock:
            with open(logp, "a", encoding="utf-8") as f:
                f.write("\n".join(lines) + "\n")
        for l in lines:
            print(l, flush=True)

        try:
            self.send_response(200 if ok else 500)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(("OK" if ok else "FAILED - see ~/vj-results/system.log").encode())
        except Exception:
            pass

    def log_message(self, *a):  # keep the journal quiet
        pass

class ThreadedServer(socketserver.ThreadingMixIn, http.server.HTTPServer):
    """MUST be threaded.

    A <video> element holds a long-lived streaming connection. With a
    single-threaded server that one connection occupies the only worker, so
    every shader fetch, thumbnail and diagnostic POST queues behind it --
    presses appeared to do nothing, then all fired at once when the clip
    changed and released the socket.
    """
    daemon_threads = True
    allow_reuse_address = True


if __name__ == "__main__":
    wb_thread = threading.Thread(target=watchdog_loop, daemon=True, name="vj-watchdog")
    wb_thread.start()
    srv = ThreadedServer(("127.0.0.1", 8080), H)
    print("== serving %s on 8080 (threaded), results -> %s, state -> %s" % (ROOT, OUT, STATE_FILE), flush=True)
    srv.serve_forever()
