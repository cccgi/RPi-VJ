#!/usr/bin/env python3
"""Generate a transparent XCursor theme so Cage/wlroots and Chromium render zero mouse pointer."""

import os
import struct
import sys

def create_transparent_xcursor(output_dir: str):
    magic = b"Xcur"
    header_size = 16
    version = 1
    ntoc = 1
    header = struct.pack("<4sIII", magic, header_size, version, ntoc)
    toc = struct.pack("<III", 0xfffd0002, 1, 36)
    img_hdr = struct.pack("<IIIIIIIII", 36, 0xfffd0002, 1, 1, 1, 1, 0, 0, 0)
    pixel = struct.pack("<I", 0)  # transparent 0x00000000 RGBA
    cursor_bytes = header + toc + img_hdr + pixel

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

    for name in names:
        target = os.path.join(cursors_dir, name)
        with open(target, "wb") as f:
            f.write(cursor_bytes)

    # index.theme
    index_theme = os.path.join(output_dir, "index.theme")
    with open(index_theme, "w", encoding="utf-8") as f:
        f.write("[Icon Theme]\nName=transparent\nComment=Transparent Cursor Theme for RPi-VJ\nInherits=transparent\n")

    print(f"Generated transparent XCursor theme in {output_dir}")

if __name__ == "__main__":
    target_dir = sys.argv[1] if len(sys.argv) > 1 else "/opt/rpi-vj/theme/transparent"
    create_transparent_xcursor(target_dir)
