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
- **Shader library extras:** hovering a shader shows its picture ("Make all pictures"
  draws them all once; they are kept in `target/desktop/thumbs/`). Double-click runs
  it in the preview only, with "To selected layer" and "Close" under the picture.
  Right-click to favourite it or remove it from the library; "Favourites" and
  "Removed" are in the group list, and a removed shader can be restored from there.
  The group list also holds groups worked out from each shader's tags, code and
  author (Fractals, Raymarched 3D, Tunnels, Tiny code, Space, Nature and so on, then
  one per author with ten or more shaders).
- **Shader editor:** under the preview while a shader is being tried out. Transform
  (zoom, turn, move), motion (speed, and how far it follows the music), colour (hue,
  saturation, brightness, contrast, gamma, invert), and a slider for each number the
  shader declares up front (`#define NAME 1.5` or `const float NAME = 1.5;`). "Edit
  code" opens the shader's code; "Apply" compiles it and, if it compiles, keeps it as
  an edited copy in `target/desktop/user-shaders/` (the library file is never
  changed; "Back to the original" drops the copy). Everything set here stays with
  the shader and applies wherever it plays.
- **Download from Shadertoy:** "Download…" beside the tabs. Paste links or ids and
  press Download. It runs the repository's own tools (`fetch_shadertoy_user.py`,
  `make_native_shadertoy.py`, `shader_bank/catalog.py`, `shader_bank/fetch_assets.py`)
  with `python`, which needs Playwright and opens a browser window while it works.
  New shaders are checked for compiling and collected in the group "New, to review";
  right-click one and choose "Mark as reviewed" to take it out.
- **Assets tab:** browse the computer for video clips and pictures (Up, Drive,
  Choose…; double-click a folder to enter it). Files are picked like shaders (click,
  Ctrl-click, Shift-click) and dragged onto a cell, or double-clicked to launch on
  the selected layer. Pictures show as stills.
- **Collections:** in the Assets tab. Name one and press "Add collection", then drag
  files onto it or use a file's right-click menu; a collection can hold files from
  anywhere. Click a collection to see its files; drag the collection itself onto a
  cell to place everything in it from there on.
- **Transitions:** each layer has a transition and a time (the last line of its
  controls): Cut, Fade, Wipe, Wipe down, Iris, Zoom, Dissolve, Push, Dip to black,
  Flash. It plays whenever the layer goes from one cell to the next, by hand, in
  sequence, shuffled or from the library's autoplay. The outgoing picture is held
  still while it gives way.
- **Effects on cells:** an effect double-clicked in the Effects tab goes to the
  highlighted cells (the picked ones, else the one clicked last), or to the layer
  when the layer's name was clicked last. Dropping an effect on a cell puts it on
  that cell; on a layer's controls, on the layer. A cell's effects run before its
  layer's, and a cell with effects shows "FX" and their number.
- **Effects tab:** beside "Shaders" at the top of the left panel. Drag an effect onto
  a layer (its controls or any of its cells) or onto the "Composition effects" box on
  the right; double-click adds it to the selected layer. Each layer and the
  composition has a chain, worked through from the top: every effect has an on/off
  box, an amount, "up"/"dn" to reorder and "x" to remove. The selected layer's chain
  and the composition's are shown under the preview.
- **Preview:** shows the output, or a library shader being tried out. "Detach" puts it
  in a window of its own that can be resized and moved to another monitor; closing
  that window puts it back. Attached, drag the panel's left edge to resize it.
- **Library presets:** type a name and press "Save preset" to keep the ticked list
  together with shuffle, time, sections, react mode, opacity and blend. "Load preset"
  brings one back and starts playing it at once; its "Delete" submenu removes one.
- **MIDI:** every MIDI input is listened to. Press "MIDI learn", click a framed
  control or cell, then move the knob or press the pad for it. Knobs take FFT gain,
  beat lead, each layer's opacity and effect amount, and the autoplay opacity; pads
  take cells, each layer's Eject, and the autoplay's Play/Stop and Next. A bound
  control has a blue frame; right-click it in learn mode to forget the binding.
- **MIDI lights:** every MIDI output is opened. A bound pad shows whether its cell is
  empty, filled or playing; a bound button whether it is on; a bound knob gets its
  value back, for encoders with a light ring. The values sent are in the "MIDI" menu
  and start with the connected controller's colours.
- **APC Key 25:** when one is connected its layout is on (the "MIDI" menu turns it
  off). The pad grid is the composition: bottom row the first layer, eight columns.
  Arrows move the grid a bank of eight columns left or right and one layer up or
  down, and are lit while there is more that way. The buttons beside the rows eject
  that layer; "stop all clips" ejects all. Knobs 1 to 5 are the opacities of the five
  layers shown, 6 the autoplay opacity, 7 the beat lead, 8 the FFT gain.
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
| `--tab <n>` / `--preview <id>` / `--detach` / `--thumbs` | Open the Effects tab; preview a shader; detach the preview; draw every hover picture |
| `--clip <file>` | Put this clip on the first layer and launch it |
| `--gain <x>` | Start with this FFT gain |
| `--output <n>` | Start projecting on monitor n |
| `--state <file>` | Keep the composition in this file instead |
| `--snapshot <png>` / `--ui-snapshot <png>` | Save the output picture or the window after `--frames <n>` frames, exit |
| `--exit-after <s>` | Exit after s seconds, printing frame rate, tempo and layer state |
