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

## Shader render tiers

A shader entry in the show manifest may carry `"render_height"`: 1080 (default), 720, 540 or 360. Width is derived at 16:9. The renderer keeps one offscreen target per tier used by the show, renders the selected shader at its tier with matching `u_resolution`/`RENDERSIZE`, and the compositor upsamples it to the 1080p output. Video stays 1080p. Manifests without the field load unchanged; any other value is rejected at load. `RPI_VJ_RENDER_WIDTH`/`RPI_VJ_RENDER_HEIGHT` remain a diagnostic override that takes precedence over every shader's tier. Both binaries parse the manifest strictly, so a manifest that uses `render_height` needs the renderer and the controller from the same build.

The shader-only diagnostic loop (a show without clips) counts late frames the same way as the show loop, a frame over 20 ms, instead of printing a constant zero.

## Shader input textures

A shader entry, in the show manifest and in the bench manifest, may carry up to four channel bindings for `iChannel0..3`:

```json
{"label": "XslGRr", "path": "XslGRr.fs", "render_height": 720,
 "channels": [
  {"channel": 0, "path": "assets/f735bee5....png", "kind": "2d", "filter": "mipmap", "wrap": "repeat", "vflip": true},
  {"channel": 2, "path": "assets/27012b4e....bin", "kind": "volume", "filter": "linear", "wrap": "repeat", "vflip": true},
  {"channel": 1, "path": "assets/94284d43....png", "kind": "cube", "filter": "linear", "wrap": "clamp", "vflip": false}]}
```

- `channel` (0-3, each at most once) and `path` are required. `kind` is `2d` (default), `cube` or `volume`; `filter` is `mipmap` (default, trilinear), `linear` or `nearest`; `wrap` is `repeat` (default) or `clamp`; `vflip` defaults to true. These are Shadertoy's settings and defaults. Show paths are relative to the manifest, bench paths are used as written. Manifests without `channels` load unchanged, and a shader without bindings serialises exactly as before; a manifest that uses them needs the renderer and the controller from the same build.
- A `channels` block that is wrong in any way (not a list, an unknown key or value, a channel outside 0-3 or bound twice, an empty path, more than four entries) disables that one shader and nothing else: the show still loads in the renderer and in the controller, the renderer reports the shader in `shader_errors` with `invalid channel bindings: ...` so its key shows as unavailable, and `Show::load` adds a warning. In a bench manifest the entry gets `prepare_error` and the run continues. The rejected block is written back unchanged if the manifest is serialised.
- `2d` is one PNG or JPEG, decoded to RGBA8 (grey replicated to RGB, missing alpha 1). `vflip` true puts the image's first row at the top (t = 1), as Shadertoy does. `cube` names the +X face; the others are `<stem>_1` to `<stem>_5` beside it, in the order -X, +Y, -Y, +Z, -Z, uploaded unflipped, six equal squares. `volume` is Shadertoy's `.bin`: `BIN\0`, u32 width/height/depth, u8 channels, u8 layout, u16 format, then packed 8-bit texels; one channel is uploaded as R8 (green and blue read 0, as on Shadertoy), four as RGBA8.
- Everything is decoded and uploaded at start-up, never on the show path. Each file is uploaded once however many shaders bind it; sampler settings are GLES 3 sampler objects, one per filter/wrap pair, so the same texture serves different settings. A 2D file bound both flipped and unflipped is two uploads. An upload carries mipmaps when any of its bindings asks for them.
- Texture memory is capped: 128 MiB by default, `RPI_VJ_TEXTURE_BUDGET_MB` (1-1024) overrides. Assets are taken in first-use order. The size of each comes from its file headers (image header, all six cube faces, the volume header checked against the file length) and is reserved against the cap before anything is decoded, so a file that does not fit or exceeds the limits is never read into memory: 2D images up to 2048 px per edge, cube faces up to 1024, volumes up to 256, files up to 32 MiB. The total is logged as `textures: N assets uploaded, ...`. All bindings of the bank's single-pass texture shaders need 100.3 MiB.
- A shader whose texture is missing, undecodable, of the wrong shape or over the cap is disabled with the reason (a `shader_errors` report in the show, `prepare_error` in the benchmark). It never renders with a wrong or absent input, and the renderer keeps running.
- The sampler type follows the binding: at prepare time a whole-line `uniform sampler2D iChannelN;` of a bound channel is rewritten to `samplerCube` or `sampler3D`, and `precision highp sampler3D;` is added where needed. After linking, a channel still declared with another type than its binding fails the shader instead of reading black. The lookup coordinates are the shader's business: the bank files that were converted with their cubemap and volume lookups cut to `.xy` do not compile against a cube or volume binding; `tools/shader_bank/adapt.py` writes their corrected source as the `tex` variant.
- In the shader pass each `iChannelN` the shader actually reads gets its own texture unit (0-3) with its texture and sampler object, or nothing; an unbound channel reads opaque black whatever ran before. `iChannelResolution[n]` is the texture size (depth in z for volumes), 0 when unbound. Shaders that read no channel pay nothing. A channel the shader reads but the manifest does not bind is logged once when the shader is compiled (`reads iChannel1 without a channel binding: sampled as opaque black`).

### `u_video`

A shader that declares `uniform sampler2D u_video;` gets the active deck's video frame on texture unit 4 during the shader pass: the 1080p target libmpv just rendered, origin bottom-left, linear, clamped. While no clip is playing the sampler reads black. Other shaders are unaffected.

`u_video` is the decoded frame as stored. For a `quad_mirror` clip that is the quarter-frame source, not the unfolded picture the compositor shows; the engine does not unfold it for the shader. A shader that cares can declare `uniform int u_video_quad_mirror;`, which is 1 while the playing clip is a `quad_mirror` clip and 0 otherwise, and unfold itself. Stem clips are not meant to be `quad_mirror` clips. The benchmark binds a synthetic 1920x1080 test pattern in its place, so stem shaders (`studio/stem-demo/stems_quad.fs`) can be measured without a decoder; that figure excludes decode cost.

## Shader benchmark

`rpi-vj-native --bench MANIFEST.json [--resume]` measures shader cost in one KMS/GLES session. Manifest, result lines and measurement rules are specified in `docs/native/SHADER_QUALIFICATION_PLAN.md` section 3. Per entry it writes a `begin` line, prepares and compiles the shader, renders the same shader pass the show uses into an offscreen target at each tier with `glFinish` after every frame, and appends one `result` line. Both lines are synced to disk. `--resume` skips `(id, variant)` keys that already hold a `result`.

Details that the plan leaves to the implementation:

- Time and audio are synthetic. Frame `i` uses `u_time = 40 * fract((i + 0.5) * 0.618034)`, so any frame count covers the 0-40 s span; audio uniforms follow a fixed 128 BPM pattern. The same sequence is used at every tier.
- `compile_ms` is compile, link and one 64x36 warm-up frame, with Mesa's disk shader cache disabled, so it is the cold cost and `first_ms` is render cost. Percentiles are nearest-rank. A tier stops after 90 frames or 2.5 s of measured frame time, with at least 3 frames.
- Pixel statistics come from three full readbacks of the first tier at 4, 20 and 36 s, box-filtered to 96x54: mean luma, mean per-sample spatial standard deviation of luma, mean absolute luma difference between consecutive samples. The 20 s sample is the PPM thumbnail. A `mean_luma` of 0 means the shader rendered black.
- An entry with `channels` is measured with those textures bound (see "Shader input textures") and its result line then carries `"channels": {"0": "2d", "1": "cube"}`, an addition to the plan's result fields that `report.py` uses to tell a texture shader measured as designed from one measured unbound. Without bindings every channel sampler reads black, as in the show, and the line is unchanged. Textures of all entries still to measure are uploaded before the first measurement, so `compile_ms` holds no texture work.
- `too_slow` has three causes, named in `error`: the first frame of the first tier took over 1500 ms; a 64x36 probe frame predicts more than four times that, in which case the full-size job is never submitted; or the kernel's V3D hang watchdog reset the GPU. The watchdog kills a render job that makes no tile progress for about a second, after which `glFinish` returns normally and the frame would look like a 1024 ms measurement. Frames of 450 ms or more are therefore re-rendered over a sentinel fill and read back; a tier with untouched tiles is reported as `{"skipped":"gpu_reset"}`.
- `RPI_VJ_BENCH_SYNC=readback|fence` swaps `glFinish` for a 1x1 readback or a fence sync. On the Pi 5 (V3D 7.1, Mesa 26.2) all three agree to 0.02 ms and track the kernel's render-queue accounting in `gpu_stats`, which the engine prints per entry on stderr.
- SIGTERM and SIGINT stop the engine between frames; the interrupted entry gets no `result`.

`tools/pi_bench.py` supervises a long run on the Pi (Python 3 standard library). It refuses to start while `rpi-vj-native` or `rpi-vj-controller` is active unless `--stop-services` is given, always launches with `--resume`, records `hang` (no new results line for `--hang-seconds`, default 60) or `crash` for the entry whose `begin` has no `result`, relaunches until every entry has a result, pauses between entries above 80 C until below 70 C, and with `--restart-services` starts both services again on every way out, including errors and signals. An entry receives at most one supervisor result. The run aborts after `--max-strikes` (default 3) consecutive launches in which the engine wrote no result, and after any new kernel oops. `--fresh` moves an existing results file aside; without it a rerun of the same command resumes.

Install the bench build beside, not over, the live one, and start it detached so it survives the SSH session:

```sh
sudo install -D target/release/rpi-vj-native /opt/rpi-vj-bench/rpi-vj-native
sudo install -D engine/tools/pi_bench.py /opt/rpi-vj-bench/pi_bench.py
sudo systemd-run --unit=rpi-vj-bench --collect \
  -p StandardOutput=append:/var/lib/rpi-vj-bench/supervisor.log \
  -p StandardError=append:/var/lib/rpi-vj-bench/supervisor.log \
  /usr/bin/python3 /opt/rpi-vj-bench/pi_bench.py /var/lib/rpi-vj-bench/manifest.json \
  --stop-services --restart-services
systemctl status rpi-vj-bench                  # running or finished
tail -f /var/lib/rpi-vj-bench/supervisor.log   # one progress line per result
sudo systemctl stop rpi-vj-bench               # stop early; services are restored
```

Engine stderr goes to `<out>.engine.log`. The benchmark needs DRM master, which is why the live services must be stopped for its duration.

Known hazard: on kernel 6.18.34+rpt, the one time the engine was killed with SIGKILL while a V3D render job was in flight, the next engine process hit a kernel oops in `drm_gem_put_pages` when it exited. This was seen once and not reproduced on purpose. The supervisor therefore sends SIGTERM and waits `--term-seconds` (default 30) for the engine to leave at a frame boundary before it resorts to SIGKILL, and the probe frame keeps single jobs short. Reboot the Pi after any oops.

`tools/benchmark_native_shaders.py` is the superseded per-process benchmark; its FPS and late-frame figures are not cost measurements.

## Verification

The lockfile needs Rust 1.88 or newer (`image` 0.25.10); on the Pi that is the rustup toolchain in `~/.cargo/bin`, not the distribution's cargo 1.85.

```sh
cargo test --locked --features controller --lib
cargo clippy --locked --all-targets --features native -- -D warnings
cargo fmt --all -- --check
python -m unittest discover -s engine/tools -p 'test_*.py'
docker build -f engine/tools/Dockerfile.check -t rpi-vj-native-check .
docker run --rm rpi-vj-native-check xvfb-run -a -s '-screen 0 1920x1080x24' python3 engine/tools/native_smoke.py
```

The container checks Linux native linking, Clippy and all migrated shader sources. Software Mesa playback checks do not substitute for Pi GPU, USB, audio or boot qualification. See `docs/native/QUALIFICATION.md`.
