# Media and show workflow

## Why media is absent

`gig/media/` is ignored except for its README. Video libraries are too large for ordinary Git, may have separate copyrights, and differ per operator. The historical `gig/show.json` is retained as a concrete configuration example, so a fresh clone will initially contain broken clip references until media is supplied or the show is regenerated.

## Supported workflow

1. Collect source clips in a staging directory, keeping originals outside the deploy directory.
2. Inspect and trim loops.
3. Normalize codecs, dimensions, pixel format, frame rate, fast-start metadata, and thumbnails.
4. Copy results below `gig/media/`.
5. Generate or edit Stream Deck clip pages in `gig/show.json`.
6. sync to the Pi and verify file hashes/paths.
7. Test cold launch and every key before the show.

## Tools

| Tool | Purpose |
|---|---|
| `gig/tools/add-clips.ps1` | Windows orchestrator: scan loops, prepare clips, add page(s), and optionally sync |
| `gig/tools/find-loops.py` | Analyze candidate loop points and write loop metadata |
| `gig/tools/loop-cut.py`, `loop-cut.sh` | Cut detected loop spans and transcode them |
| `gig/tools/transcode.ps1`, `transcode.sh` | Normalize clips and generate 96×96 Stream Deck thumbnails |
| `gig/tools/build-clip-bank.py` | Add/remove clip banks in `show.json` |
| `gig/tools/build-show.py` | Build the historical show configuration |
| `gig/tools/build-presets.py` | Build shader preset sequences |
| `gig/tools/sync.ps1` | Transfer code/config/media to the Pi and verify deployment |
| `gig/tools/test-hevc-support.sh` | Determine whether the installed Chromium build can decode HEVC |

Run any tool with `--help` or read its header before use; several scripts encode assumptions from the original operator's directory layout. Work on copies and review the resulting `show.json` diff.

## Encoding rules

The browser must decode the chosen codec. Raspberry Pi 5 hardware support does not guarantee that a particular Chromium package exposes that codec. Test HEVC on the exact deployed image. H.264 is broadly compatible but is CPU-decoded on Pi 5; HEVC may be hardware-decoded when Chromium and the OS expose it.

Regardless of codec:

- use 8-bit 4:2:0 (`yuv420p`);
- make the first frame independently decodable;
- use short, closed GOPs for predictable starts;
- move MP4 metadata to the front (`+faststart`);
- remove audio from visual clips;
- standardize resolution and frame rate;
- avoid variable-frame-rate sources;
- validate duration, dimensions, codec, and decode on the Pi.

The supplied transcode scripts contain the project's concrete FFmpeg arguments. A 96×96 still image with the same base filename is used for the Stream Deck key where configured.

## `show.json` model

The top-level ordered `banks` array becomes Stream Deck pages. Each bank contains a 4×8 grid. A cell may bind a clip, shader, video effect, shader effect, blend mode, or action. Runtime code overrides keys 24, 25, 30, and 31 on every page and may override 26–29 on pages 1–5; do not place irreplaceable clips in those slots.

Clip paths are URL paths relative to the served repository. Use forward slashes and exact case; Linux paths are case-sensitive. After editing, load the application locally, cycle through all pages, and inspect the browser console for configuration or missing-file errors.

## Transfer choices

- `sync.ps1` is the historical scripted path and performs deployment checks.
- `scp`/`rsync` are suitable for controlled updates with SSH keys.
- Samba makes drag-and-drop easier but should be restricted to a trusted LAN.

The future appliance plan adds a Stream Deck-driven USB ingest queue and local web portal. Those features are not implemented on `original`; see [`tasks/APPLIANCE-V3/A3-09-usb-discovery-copy.md`](../../tasks/APPLIANCE-V3/A3-09-usb-discovery-copy.md) through A3-12.
