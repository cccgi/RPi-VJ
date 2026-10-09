# RPi-VJ Desktop

A Windows VJ app that shares the Pi engine's audio analysis, beat prediction, react
modes and shader format. Phase 4 of five: a composition of layers filled from a
shader library and video clips.

## Run

```
cargo run --release -p rpi-vj-desktop
```

Run it from anywhere inside the repository. It reads the shader library from
`target/shader-bank/catalog.json` and the shaders' input textures from
`target/shader-bank/assets/` (set `RPI_VJ_ROOT` to point at another checkout).

Video clips need the mpv player library, `libmpv-2.dll`, which is not in the
repository. Put it in `target/desktop/mpv/`, beside the program, or in a folder named
by `RPI_VJ_MPV`. It comes from the `mpv-dev-x86_64` archive at
<https://github.com/shinchiro/mpv-winbuild-cmake/releases>. Without it a layer that is
asked to play a clip says that the library is missing.

The composition, folders and settings are kept in `%APPDATA%\RPi-VJ\desktop.json`,
written every 20 seconds and on exit.

## What is there

- **Library (left):** every shader in the catalog, searchable by name, author or id.
  "On the Pi" lists the shaders of the Pi show. Folders of your own: type a name and
  press "Add folder", then drag shaders onto it or use a shader's right-click menu.
  Click selects, Ctrl-click adds, Shift-click takes a range. Multipass shaders appear
  greyed out when "multipass" is ticked; the engine is single-pass.
- **Composition (middle):** layers as rows, cells as columns. Drag one or many shaders
  onto a cell and they fill the row from there, adding columns as needed.
  Double-clicking a library shader puts it on the selected layer and launches it.
  "Add clips to selected layer…" or dropping video files on the window adds clips.
  Click a cell to launch it; the live cell has a green frame. Right-click a cell to
  clear it, a layer's controls to clear or remove the layer.
- **Play times:** every cell has a play time (20 s unless changed; right-click the cell).
  A layer set to Sequence or Shuffle moves on by itself when the live cell's time is
  up, and with "on sections" ticked also when the song changes section. Shuffle plays
  every cell once before any comes round again. A cell can have its own react mode
  (right-click), shown on its corner; otherwise it takes the layer's.
- **Library autoplay:** tick shaders in the library's checkbox column ("all" ticks
  everything the list shows), then "Play checked". They play one after another on a
  layer of their own above the composition, in list order or shuffled, each for the
  time set, with the react mode chosen beside it, optionally also changing on song
  sections. The one playing is green in the list.
- **Library presets:** type a name and press "Save preset" to keep the ticked list
  together with shuffle, time, sections, react mode, opacity and blend. "Load preset"
  brings one back and starts playing it at once; its "Delete" submenu removes one.
- **MIDI:** every MIDI input is listened to. Press "MIDI learn", click a framed
  control or cell, then move the knob or press the pad for it. Knobs take FFT gain,
  beat lead, each layer's opacity and effect amount, and the autoplay opacity; pads
  take cells, each layer's Eject, and the autoplay's Play/Stop and Next. A bound
  control has a blue frame; right-click it in learn mode to forget the binding.
- **Composition files:** "Save as…" writes the layers to a file, "Open…" replaces the
  grid with a file's.
- **Per layer:** opacity, blend mode (Add, Screen, Mix, Multiply, Lighten, Difference),
  react mode (PUMP, STROBE, STEP, RIDE, TWIST, AUTO), one effect with an amount
  (Mirror, Pixelate, RGB split, Invert, Mono, Hue, Kaleido), Bypass and Eject. The top
  row is drawn last, over the rows below it.
- **Audio in (top):** any capture device, or an output device heard through loopback
  ("what is playing"). FFT gain, a level meter with clip warning, the tempo the tracker
  finds and how sure it is, and the beat predictor's lead in milliseconds.
- **Output (right, top bar):** a small preview, and the same picture borderless on any
  monitor.

## Test options

Any of these except `--state`, `--gain` and `--output` keeps the saved composition out
of the way.

| Option | Effect |
|---|---|
| `--compile-all` | Compile every runnable shader, write `target/desktop/compile-report.json`, exit |
| `--live <id>` | Put this shader on the second layer and launch it |
| `--fill <n>` | Put the first n shaders of the Pi show on the second layer |
| `--fx <n>` | Give the second layer effect number n |
| `--sequence` / `--autoplay <n>` | Play the second layer in sequence; tick and play n library shaders, a second each |
| `--open <file>` | Open this composition file at start |
| `--midi-test` | Bind and send a few MIDI messages and take the ticked list through a preset |
| `--clip <file>` | Put this clip on the first layer and launch it |
| `--gain <x>` | Start with this FFT gain |
| `--output <n>` | Start projecting on monitor n |
| `--state <file>` | Keep the composition in this file instead |
| `--snapshot <png>` / `--ui-snapshot <png>` | Save the output picture or the window after `--frames <n>` frames, exit |
| `--exit-after <s>` | Exit after s seconds, printing frame rate, tempo and layer state |
