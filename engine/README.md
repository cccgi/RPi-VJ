# Native VJ engine

Browser-free Rust performance runtime on `codex/native-mpv-core`. Two independently supervised executables isolate the physical controller from graphics/decoder failures. No browser, web server, desktop shell, Stream Deck application or network service is required.

This is a development implementation. Pi 5 hardware decode, USB behavior, direct KMS boot and sustained 1080p60 remain release qualification gates.

## Processes and controls

`rpi-vj-controller` opens Stream Deck XL directly through HIDAPI's Linux hidraw backend. Separate USB and image workers keep JPEG preparation out of button polling. The controller owns page/clip/effect state, persists checkpoints, and sends bounded versioned snapshots over a private Unix socket. The renderer reports actual presentation; a load request alone never turns a key green. Disconnection paints OFFLINE while preserving navigation and desired state. Keys retain their previous action until the new image's last USB packet completes.

`rpi-vj-native` embeds two libmpv instances, owns the GLES compositor and hosts native GLSL shaders. It keeps the current clip while loading a candidate and warms one subsequent clip. Invalid candidates preserve current output. An active decoder failure exits for service recovery. Controller reconnection replays state. Audio capture retries after loss and decays to silence while disconnected.

The migrated `examples/legacy-show.json` contains nine visual pages: CLIPS, MIX, SYS, FX, FX-LAYER, QUADMIRROR, 50TH, 720p and NEON. It retains 113 clip entries and 18 original shader presets. The legacy MUSIC bank is outside the visual launcher. Keys 24/25 navigate, 30 resets and 31 blacks out. Pages include speed, BPM/tap, HUD, opacity, eight blend modes and 24 independent video/shader effects. Thumbnails are prepared stills; key images use native text rendering.

## Build

On Raspberry Pi OS Bookworm Lite, install the native development libraries:

```sh
sudo apt-get install build-essential pkg-config libsdl2-dev libmpv-dev libudev-dev libasound2-dev ffmpeg
cargo build --release --locked --features native
```

The controller can be built alone with `--features controller --bin rpi-vj-controller`; it has no SDL, GL or libmpv dependency. The renderer alone uses `--features renderer --bin rpi-vj-native`.

Package installation does not prove Pi HEVC acceleration. The renderer requires a non-software `hwdec-current` before promoting each candidate. `--allow-software` is solely a diagnostic option. The exact Pi libmpv/FFmpeg build and concurrent decoder/render path must pass hardware tests.

## Content preparation

```sh
python engine/tools/import_legacy_show.py --help
python engine/tools/prepare_show.py engine/examples/legacy-show.json prepared/native-hevc
```

Preparation preserves pages and originals, produces HEVC Main 8-bit 1080p60 MP4s and 96px thumbnails, fully decodes each output for validation, and publishes the final show only after all assets succeed. Interrupted work resumes from a manifest/source-stamped progress record. Workstation NVIDIA encoding is optional with `--encoder hevc_nvenc --ffmpeg /path/to/ffmpeg`. Preparation does not create additional motion or repair loop boundaries.

Shadertoy imports are offline and API/file based. See [Shadertoy ingestion](../docs/native/SHADERTOY.md) and `tools/import_shadertoy.py`; the runtime never contacts Shadertoy or launches a browser.

## Run and deploy

Install both binaries in `/opt/rpi-vj-native`, the prepared show under `/srv/rpi-vj-native`, both units from `deploy/`, and the supplied HID udev rule. Create a `vj` service user with video/render/audio/input access. Enable both services independently. Neither service requires the other to start or stay alive.

The renderer uses SDL KMSDRM on tty1 and ALSA for capture. Set `audio_device` in the manifest to the exact SDL capture-device name; null disables capture. Verify HDMI 1920x1080@60, DRM ownership and permissions on the target image. Desktop diagnostics use `--windowed --allow-software`.

Controller readiness follows a complete key upload; missing USB hardware keeps it retrying without stopping the renderer. Its watchdog checks both worker progress counters. Renderer readiness follows its first swap, and its watchdog follows frame-loop progress. Readiness is not proof of physical HDMI/LCD output. Checkpoints live in the controller StateDirectory and are rejected if the show changes.

Before enabling, inspect and disable historical system and user kiosk/broker/controller units and any display manager. Preserve the SD image for rollback. Historical deployment scripts must not be used for this branch. No Pi installation has been performed by this change.

## Verification

```sh
cargo test --locked --features controller --lib
cargo clippy --locked --all-targets --features native -- -D warnings
cargo fmt --all -- --check
python -m unittest discover -s engine/tools -p 'test_*.py'
docker build -f engine/tools/Dockerfile.check -t rpi-vj-native-check .
docker run --rm rpi-vj-native-check xvfb-run -a -s '-screen 0 1920x1080x24' python3 engine/tools/native_smoke.py
```

The container checks Linux native linking, Clippy and all migrated shader sources. Software Mesa playback checks do not substitute for Pi GPU, USB, audio or boot qualification. See `docs/native/QUALIFICATION.md`.
