#!/usr/bin/env python3
"""Prepare a complete native show as HEVC while preserving its pages and bindings."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time
from prepare_media import encode_command, verify


def publish(path, data):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(data, indent=2) + '\n', encoding='utf-8')
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('show', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--ffmpeg', default='ffmpeg')
    parser.add_argument('--encoder', choices=['libx265', 'hevc_nvenc'], default='libx265')
    args = parser.parse_args()
    source = args.show.resolve()
    output = args.output.resolve()
    original = source.read_bytes()
    signature = hashlib.sha256(original).hexdigest()
    show = json.loads(original)
    progress_path = output / 'preparation.json'
    if output.exists():
        if not progress_path.is_file():
            parser.error('existing output is not a resumable prepared show')
        progress = json.loads(progress_path.read_text(encoding='utf-8'))
        if progress['source_manifest_sha256'] != signature:
            parser.error('source manifest differs from interrupted preparation')
    else:
        output.mkdir(parents=True)
        progress = dict(source_manifest_sha256=signature, complete={})
        publish(progress_path, progress)
    (output / 'media').mkdir(exist_ok=True)
    (output / 'shaders').mkdir(exist_ok=True)
    started = time.monotonic()
    for index, clip in enumerate(show['clips']):
        path = (source.parent / clip['path']).resolve()
        if not path.is_file():
            raise ValueError(f'missing source media: {path}')
        # Refuse any overlap even when a caller chooses an output inside legacy media.
        if output == path.parent or output in path.parents:
            raise ValueError('output overlaps a source asset')
        stamp = [str(path), path.stat().st_size, path.stat().st_mtime_ns]
        destination = output / 'media' / f'{index:04d}.mp4'
        thumbnail = destination.with_suffix('.jpg')
        entry = progress['complete'].get(str(index))
        if not (entry and entry['source'] == stamp and destination.is_file()
                and entry['bytes'] == destination.stat().st_size and thumbnail.is_file()):
            if destination.exists():
                raise ValueError(f'prepared asset differs from progress record: {destination}')
            if shutil.disk_usage(output).free < 2 * 1024**3:
                raise ValueError('less than 2 GiB free; preparation paused')
            temporary = destination.with_name(destination.stem + '.encoding.mp4')
            # Only our own incomplete temporary may be replaced when resuming.
            if temporary.exists():
                temporary.unlink()
            args_ff = encode_command(path, temporary)
            args_ff[0] = args.ffmpeg
            if args.encoder == 'hevc_nvenc':
                start, end = args_ff.index('-c:v'), args_ff.index('-tag:v')
                args_ff[start:end] = ['-c:v', 'hevc_nvenc', '-preset', 'p5', '-tune', 'hq',
                                     '-rc', 'vbr', '-cq', '20', '-b:v', '0', '-g', '30',
                                     '-bf', '0', '-forced-idr', '1', '-no-scenecut', '1',
                                     '-profile:v', 'main', '-pix_fmt', 'yuv420p']
            else:
                offset = args_ff.index('-x265-params') + 1
                args_ff[offset] += ':pools=8:frame-threads=3:log-level=error'
            subprocess.run(args_ff, check=True)
            verify(temporary)
            subprocess.run(['ffmpeg', '-nostdin', '-v', 'error', '-y', '-i', str(temporary),
                            '-frames:v', '1', '-vf', 'scale=96:96', str(thumbnail)], check=True)
            temporary.rename(destination)
            progress['complete'][str(index)] = dict(source=stamp, bytes=destination.stat().st_size,
                                                    encoder=args.encoder)
            publish(progress_path, progress)
        clip['path'] = destination.relative_to(output).as_posix()
        clip['thumbnail'] = thumbnail.relative_to(output).as_posix()
        print(f'HEVC {index+1}/{len(show["clips"])} {clip["label"]} elapsed={time.monotonic()-started:.0f}s', flush=True)
    for index, shader in enumerate(show['shaders']):
        source_shader = (source.parent / shader['path']).resolve()
        target = output / 'shaders' / f'{index:02d}-{source_shader.name}'
        if target.exists() and target.read_bytes() != source_shader.read_bytes():
            raise ValueError(f'conflicting shader asset {target}')
        if not target.exists():
            shutil.copyfile(source_shader, target)
        shader['path'] = target.relative_to(output).as_posix()
    publish(output / 'show.json', show)
    print(f'COMPLETE {output / "show.json"}', flush=True)


if __name__ == '__main__':
    main()
