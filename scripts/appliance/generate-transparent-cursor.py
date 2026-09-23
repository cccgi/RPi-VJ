#!/usr/bin/env python3
"""Generate a transparent XCursor theme so Cage/wlroots and Chromium render zero mouse pointer.

Writes real, correctly-offset Xcursor binary files (X.org Xcursor file
format): FILE_HEADER, one TOC entry per cursor image, then the image chunk
itself (CHUNK_HEADER + width/height/xhot/yhot/delay + a single fully
transparent ARGB32 pixel). Every offset is computed from the actual struct
sizes rather than hardcoded, so the file's table of contents always points
at the real chunk position.
"""

import os
import struct
import sys

XCURSOR_MAGIC = b"Xcur"
FILE_HEADER_SIZE = 16          # magic(4) + header(4) + version(4) + ntoc(4)
FILE_VERSION = 0x1_0000        # Xcursor file format version 1.0
TOC_ENTRY_SIZE = 12            # type(4) + subtype(4) + position(4)
IMAGE_CHUNK_HEADER_SIZE = 36   # header(4) + type(4) + subtype(4) + version(4)
                                # + width(4) + height(4) + xhot(4) + yhot(4) + delay(4)
CURSOR_IMAGE_TYPE = 0xfffd0002
IMAGE_CHUNK_VERSION = 1


# Compositors and toolkits don't reliably read XCURSOR_SIZE from the
# environment when creating their cursor manager (wlroots-based compositors
# in particular commonly request a hardcoded nominal size, e.g. 24, rather
# than consulting the env var). If a theme only supplies a single nominal
# size and that size isn't what gets requested, the lookup for that cursor
# name fails and the library falls back to its own built-in *visible*
# default arrow bitmap -- invisible in the file we ship, but not what
# actually gets drawn. So every cursor file here contains fully-transparent
# images at every commonly-requested size, not just one, eliminating the
# possibility of a fallback regardless of what size any given client asks
# for.
CURSOR_SIZES = (1, 8, 16, 18, 20, 22, 24, 28, 32, 36, 40, 48, 56, 64, 72, 96, 128)


def build_transparent_cursor_file(nominal_sizes=CURSOR_SIZES) -> bytes:
    """Build one Xcursor file containing a fully-transparent image at every
    size in `nominal_sizes`, so the file resolves correctly no matter which
    nominal size the compositor/toolkit actually requests."""
    ntoc = len(nominal_sizes)
    toc_position = FILE_HEADER_SIZE + ntoc * TOC_ENTRY_SIZE

    file_header = struct.pack("<4sIII", XCURSOR_MAGIC, FILE_HEADER_SIZE, FILE_VERSION, ntoc)

    tocs = b""
    chunks = b""
    position = toc_position
    for size in nominal_sizes:
        tocs += struct.pack("<III", CURSOR_IMAGE_TYPE, size, position)
        image_chunk_header = struct.pack(
            "<IIIIIIIII",
            IMAGE_CHUNK_HEADER_SIZE,
            CURSOR_IMAGE_TYPE,
            size,
            IMAGE_CHUNK_VERSION,
            size,  # width
            size,  # height
            0,      # xhot
            0,      # yhot
            0,      # delay (ms)
        )
        pixels = struct.pack("<I", 0x00000000) * (size * size)
        chunk = image_chunk_header + pixels
        chunks += chunk
        position += len(chunk)

    return file_header + tocs + chunks


def create_transparent_xcursor(output_dir: str):
    cursors_dir = os.path.join(output_dir, "cursors")
    os.makedirs(cursors_dir, exist_ok=True)

    names = [
        "default", "left_ptr", "right_ptr", "pointer", "wait", "crosshair",
        "text", "hand1", "hand2", "arrow", "top_left_arrow", "center_ptr",
        "cross", "dnd-none", "dnd-copy", "dnd-move", "grabbing", "grab",
        "not-allowed", "help", "progress", "alias", "copy", "move", "no-drop",
        "all-scroll", "col-resize", "row-resize", "n-resize", "e-resize",
        "s-resize", "w-resize", "ne-resize", "nw-resize", "se-resize",
        "sw-resize", "ew-resize", "ns-resize", "nesw-resize", "nwse-resize",
        "zoom-in", "zoom-out"
    ]

    cursor_bytes = build_transparent_cursor_file()
    for name in names:
        target = os.path.join(cursors_dir, name)
        with open(target, "wb") as f:
            f.write(cursor_bytes)

    # index.theme — standalone theme, no inheritance (this theme already
    # supplies every cursor name it needs; inheriting from itself, as a
    # previous version of this script did, is meaningless and risks a
    # resolver loop).
    index_theme = os.path.join(output_dir, "index.theme")
    with open(index_theme, "w", encoding="utf-8") as f:
        f.write("[Icon Theme]\nName=transparent\nComment=Transparent Cursor Theme for RPi-VJ\n")

    print(f"Generated transparent XCursor theme in {output_dir}")


if __name__ == "__main__":
    target_dir = sys.argv[1] if len(sys.argv) > 1 else "/opt/rpi-vj/theme/transparent"
    create_transparent_xcursor(target_dir)
