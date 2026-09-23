#!/usr/bin/env python3
"""Small, bounded controller for an audio-only mpv/libmpv process.

The browser never decodes music.  mpv owns one audio decoder and is controlled
through its local Unix JSON IPC socket.  All playable files must resolve below
``gig/media/music``; the HTTP layer cannot turn this into an arbitrary-file
player.
"""
import json
import os
import shutil
import socket
import subprocess
import threading
import time


class MpvError(RuntimeError):
    pass


class MpvPlayer:
    def __init__(self, music_root, socket_path=None, executable=None):
        self.music_root = os.path.realpath(music_root)
        runtime = os.environ.get("XDG_RUNTIME_DIR") or "/tmp"
        self.socket_path = socket_path or os.path.join(runtime, "rpi-vj-mpv.sock")
        self.executable = executable or shutil.which("mpv") or "/usr/bin/mpv"
        self._process = None
        self._lock = threading.RLock()
        self._request_id = 0

    def available(self):
        return os.path.isfile(self.executable) and os.access(self.executable, os.X_OK)

    def _safe_path(self, src):
        if not isinstance(src, str) or not src.strip():
            raise MpvError("missing music source")
        # show.json stores paths relative to gig/, e.g. media/music/song.flac.
        rel = src.replace("\\", "/")
        if rel.startswith("media/music/"):
            rel = rel[len("media/music/"):]
        if os.path.isabs(rel):
            raise MpvError("absolute music paths are not allowed")
        path = os.path.realpath(os.path.join(self.music_root, rel))
        try:
            contained = os.path.commonpath([path, self.music_root]) == self.music_root
        except ValueError:
            contained = False
        if not contained:
            raise MpvError("music path escapes the library")
        if not os.path.isfile(path):
            raise MpvError("music file not found")
        return path

    def _start(self):
        if self._process is not None and self._process.poll() is None and os.path.exists(self.socket_path):
            return
        if not self.available():
            raise MpvError("mpv is not installed")
        if self._process is not None and self._process.poll() is not None:
            self._process = None
        try:
            if os.path.exists(self.socket_path):
                os.unlink(self.socket_path)
        except OSError:
            pass
        cmd = [
            self.executable,
            "--idle=yes", "--no-video", "--audio-display=no",
            "--keep-open=no", "--force-window=no", "--terminal=no",
            "--really-quiet", "--input-ipc-server=" + self.socket_path,
            "--audio-client-name=RPi-VJ Music",
        ]
        self._process = subprocess.Popen(
            cmd, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL, close_fds=True,
        )
        deadline = time.monotonic() + 3.0
        while time.monotonic() < deadline:
            if self._process.poll() is not None:
                raise MpvError("mpv exited during startup")
            if os.path.exists(self.socket_path):
                return
            time.sleep(0.025)
        raise MpvError("mpv IPC socket did not appear")

    def _command(self, *command):
        with self._lock:
            self._start()
            self._request_id += 1
            rid = self._request_id
            request = (json.dumps({"command": list(command), "request_id": rid}) + "\n").encode()
            try:
                with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
                    conn.settimeout(2.0)
                    conn.connect(self.socket_path)
                    conn.sendall(request)
                    buf = b""
                    while True:
                        chunk = conn.recv(65536)
                        if not chunk:
                            raise MpvError("mpv IPC closed without a response")
                        buf += chunk
                        while b"\n" in buf:
                            line, buf = buf.split(b"\n", 1)
                            if not line:
                                continue
                            response = json.loads(line.decode("utf-8", "replace"))
                            if response.get("request_id") != rid:
                                continue
                            if response.get("error") != "success":
                                raise MpvError("mpv: " + str(response.get("error")))
                            return response.get("data")
            except (OSError, ValueError, json.JSONDecodeError) as exc:
                raise MpvError("mpv IPC failure: %s" % exc) from exc

    def play(self, src):
        path = self._safe_path(src)
        self._command("loadfile", path, "replace")
        self._command("set_property", "pause", False)
        return self.status()

    def toggle_pause(self):
        self._command("cycle", "pause")
        return self.status()

    def toggle_mute(self):
        self._command("cycle", "mute")
        return self.status()

    def seek_forward(self, seconds):
        seconds = int(seconds)
        if seconds not in (30, 60, 120, 180):
            raise MpvError("unsupported seek interval")
        # Keep the same demuxer/decoder alive. relative+exact is accurate for
        # audio and avoids loadfile/reopen, so playback resumes at the target
        # without creating a second player or restarting the track.
        self._command("seek", seconds, "relative+exact")
        return {"ok": True, "seeked": seconds}

    def change_volume(self, delta):
        delta = int(delta)
        if delta not in (-5, 5):
            raise MpvError("unsupported volume step")
        self._command("add", "volume", delta)
        volume = self._property("volume", 100)
        return {"ok": True, "volume": round(float(volume))}

    def _property(self, name, default=None):
        try:
            value = self._command("get_property", name)
            return default if value is None else value
        except MpvError:
            return default

    def status(self):
        running = self._process is not None and self._process.poll() is None
        if not running:
            return {"available": self.available(), "running": False,
                    "playing": False, "paused": False, "muted": False,
                    "title": "", "path": ""}
        path = self._property("path", "") or ""
        return {
            "available": True,
            "running": True,
            "playing": bool(path) and not bool(self._property("pause", False)),
            "paused": bool(path) and bool(self._property("pause", False)),
            "muted": bool(self._property("mute", False)),
            "title": self._property("media-title", "") or "",
            "path": path,
            "time": self._property("time-pos", 0) or 0,
            "duration": self._property("duration", 0) or 0,
        }
