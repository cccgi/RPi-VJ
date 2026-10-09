# RPi-VJ Desktop

A Windows VJ app that shares the Pi engine's audio analysis, beat prediction, react
modes and shader format. Phase 1 of five: one shader layer over one video layer.

## Run

```
cargo run --release -p rpi-vj-desktop
```

Run it from anywhere inside the repository. It reads the shader library from
`target/shader-bank/catalog.json` (set `RPI_VJ_ROOT` to point at another checkout).

Video clips need the mpv player library, `libmpv-2.dll`, which is not in the
repository. Put it in `target/desktop/mpv/`, beside the program, or in a folder named
by `RPI_VJ_MPV`. It comes from the `mpv-dev-x86_64` archive at
<https://github.com/shinchiro/mpv-winbuild-cmake/releases>. Without it the app runs
and says that video is off.

## What is there

- **Shader library:** every shader in the catalog, searchable by name, author or id.
  A click puts one live. Multipass shaders are listed greyed out; the engine is
  single-pass. Shaders that read input textures run, but see black where the texture
  would be.
- **Audio in:** any capture device, or an output device heard through loopback ("what
  is playing"). FFT gain, a level meter with clip warning, the tempo the tracker finds
  and how sure it is.
- **Beat prediction:** the Pi's predictor, with its lead as a slider in milliseconds.
- **React modes:** PUMP, STROBE, STEP, RIDE, TWIST and AUTO, as on the Pi.
- **Video layer:** one looping clip under the shader layer, with opacity and blend
  mode for the shader on top.
- **Preview and projection:** a preview in the window, and the same picture borderless
  on any monitor.

## Test options

| Option | Effect |
|---|---|
| `--compile-all` | Compile every runnable shader, write `target/desktop/compile-report.json`, exit |
| `--live <id>` | Start with this shader live |
| `--clip <file>` | Start with this clip playing |
| `--gain <x>` | Start with this FFT gain |
| `--output <n>` | Start projecting on monitor n |
| `--snapshot <png>` / `--ui-snapshot <png>` | Save the output picture or the window after `--frames <n>` frames, exit |
| `--exit-after <s>` | Exit after s seconds, printing frame rate, tempo and device state |
