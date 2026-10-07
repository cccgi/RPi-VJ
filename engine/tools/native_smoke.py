#!/usr/bin/env python3
"""Linux integration smoke under xvfb-run. Uses software decode only for CI."""
import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import tempfile
import time


def receive(peer):
    def exact(count):
        data = b''
        while len(data) < count:
            block = peer.recv(count - len(data))
            if not block:
                raise RuntimeError('renderer disconnected')
            data += block
        return data
    size, = struct.unpack('<I', exact(4))
    assert size <= 65536
    return json.loads(exact(size))['message']


def main():
    root = Path('/work')
    with tempfile.TemporaryDirectory() as temporary:
        work = Path(temporary)
        for index, color in enumerate(['red', 'blue']):
            subprocess.run(['ffmpeg', '-v', 'error', '-f', 'lavfi', '-i',
                            f'color={color}:s=1920x1080:r=60', '-t', '2', '-c:v', 'libx264',
                            '-preset', 'ultrafast', str(work / f'{index}.mp4')], check=True)
        original = json.loads((root / 'engine/examples/legacy-show.json').read_text())
        show = dict(version=2, bpm=128, audio_device=None, pages=[],
                    clips=[dict(label=str(i), path=str(work / f'{i}.mp4')) for i in range(3)],
                    shaders=original['shaders'])
        for shader in show['shaders']:
            shader['path'] = str(root / 'engine/examples' / shader['path'])
        path = work / 'show.json'
        path.write_text(json.dumps(show))
        state = json.loads(subprocess.check_output([
            'cargo', 'run', '--quiet', '--locked', '--example', 'state_for_show', '--', str(path)]))
        server = socket.socket(socket.AF_UNIX)
        address = str(work / 'control.sock')
        server.bind(address)
        server.listen(1)
        server.settimeout(45)
        log = open('/tmp/native-smoke.log', 'w+')
        process = subprocess.Popen(['target/debug/rpi-vj-native', str(path), '--windowed',
                                    '--allow-software', '--control-socket', address],
                                   stdout=log, stderr=log, env={**os.environ, 'SDL_AUDIODRIVER': 'dummy'})
        try:
            peer, _ = server.accept()
            peer.settimeout(20)
            revision = 0
            def send(**changes):
                nonlocal revision
                revision += 1
                state.update(changes)
                data = json.dumps(dict(version=1, message=dict(kind='Snapshot', session=17,
                                  revision=revision, state=state))).encode()
                peer.sendall(struct.pack('<I', len(data)) + data)
            def until(predicate):
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline:
                    report = receive(peer)
                    assert not report['shader_errors'], report['shader_errors']
                    if predicate(report):
                        return report
                raise AssertionError('report condition timed out')
            send(requested=0, generation=1)
            until(lambda r: r['playing'] == dict(clip=0, generation=1))
            print('PASS first clip presented', flush=True)
            time.sleep(2)
            send(requested=1, generation=2, shader=True, selected_shader=0, hud=True,
                 video_effect=1, shader_effect=2)
            until(lambda r: r['playing'] == dict(clip=1, generation=2))
            print('PASS warm clip switch and shader/effect/HUD rendering', flush=True)
            send(requested=2, generation=3)
            report = until(lambda r: r['failed'] and r['failed']['generation'] == 3)
            assert report['playing']['clip'] == 1, report
            print('PASS bad candidate preserves active clip', flush=True)
            peer.close()
            peer, _ = server.accept()
            peer.settimeout(20)
            send(requested=0, generation=4)
            until(lambda r: r['playing'] == dict(clip=0, generation=4))
            print('PASS controller socket reconnect and state replay', flush=True)
            send(requested=None, generation=5, shader=False, selected_shader=None)
            until(lambda r: r['playing'] is None)
            print('PASS reset cancels presentation', flush=True)
        finally:
            process.terminate()
            try:
                process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            log.seek(0)
            print(log.read()[-12000:])
            server.close()


if __name__ == '__main__':
    main()
