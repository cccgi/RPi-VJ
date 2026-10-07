#!/usr/bin/env python3
"""Capture the local shader library as a native GLES slideshow under Xvfb."""
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import time


def frame(message):
    payload = json.dumps({"version": 1, "message": message}).encode()
    return struct.pack("<I", len(payload)) + payload


def main():
    output = Path(os.environ.get("SHOWCASE_OUTPUT", "/out"))
    output.mkdir(parents=True, exist_ok=True)
    shaders = sorted(Path("/work/shaders").glob("*.fs"))
    if not shaders:
        raise SystemExit("no shaders mounted")
    show = {
        "version": 2,
        "bpm": 128,
        "audio_device": None,
        "clips": [],
        "pages": [],
        "shaders": [{"label": p.stem, "path": str(p), "blend": "Additive", "opacity": 0.9} for p in shaders],
    }
    show_path = Path("/tmp/showcase-show.json")
    show_path.write_text(json.dumps(show), encoding="utf-8")
    socket_path = "/tmp/showcase-control.sock"
    try:
        os.unlink(socket_path)
    except FileNotFoundError:
        pass
    server = socket.socket(socket.AF_UNIX)
    server.bind(socket_path)
    server.listen(1)
    server.settimeout(30)
    xvfb = subprocess.Popen(["Xvfb", ":99", "-screen", "0", "1280x720x24"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    renderer = subprocess.Popen(
        ["target/debug/rpi-vj-native", str(show_path), "--windowed", "--allow-software", "--control-socket", socket_path],
        env={**os.environ, "DISPLAY": ":99", "SDL_VIDEODRIVER": "x11", "SDL_AUDIODRIVER": "dummy"},
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True,
    )
    recorder = subprocess.Popen(
        ["ffmpeg", "-y", "-loglevel", "error", "-f", "x11grab", "-video_size", "1280x720", "-framerate", "30", "-i", ":99.0", "-c:v", "libx264", "-pix_fmt", "yuv420p", str(output / "shaders.mp4")],
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
    )
    try:
        peer, _ = server.accept()
        peer.settimeout(1)
        base = {
            "clips": 0, "shaders": len(shaders), "pages": 1, "default_bpm": 128.0,
            "last_tap": None, "shader_presets": [["Additive", 0.9] for _ in shaders],
            "page": 0, "requested": None, "playing": None, "failed": None,
            "shader": True, "selected_shader": 0, "blackout": False,
            "blend": "Alpha", "opacity": 0.75, "generation": 0,
            "video_effect": 0, "shader_effect": 0, "video_effect_amount": 1.0,
            "shader_effect_amount": 1.0, "speed": 1.0, "bpm": 128.0,
            "hud": True, "failed_shaders": [],
        }
        for index, shader in enumerate(shaders):
            state = dict(base, selected_shader=index, generation=index + 1)
            peer.sendall(frame({"kind": "Snapshot", "session": 77, "revision": index + 1, "state": state}))
            print(f"{index + 1:02d}/{len(shaders)} {shader.name}", flush=True)
            time.sleep(2.0)
        time.sleep(1.0)
    finally:
        recorder.send_signal(subprocess.signal.SIGINT)
        try:
            recorder.wait(timeout=8)
        except subprocess.TimeoutExpired:
            recorder.kill()
        renderer.terminate()
        try:
            renderer.wait(timeout=5)
        except subprocess.TimeoutExpired:
            renderer.kill()
        xvfb.terminate()
        server.close()
    subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", str(output / "shaders.mp4"), "-vf", "select='eq(n,30)+eq(n,90)+eq(n,150)+eq(n,210)+eq(n,270)+eq(n,330)+eq(n,390)+eq(n,450)+eq(n,510)+eq(n,570)+eq(n,630)+eq(n,690)+eq(n,750)+eq(n,810)+eq(n,870)+eq(n,930)+eq(n,990)+eq(n,1050)'", "-vsync", "0", str(output / "frames-%02d.png")], check=True)
    print(f"SHOWCASE={output / 'shaders.mp4'}", flush=True)


if __name__ == "__main__":
    main()
