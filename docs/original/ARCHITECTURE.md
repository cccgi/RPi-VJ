# Original implementation architecture

## Runtime data flow

```text
Stream Deck XL --USB HID/WebHID-->
                                  Chromium single-page application
USB line input --Web Audio------->  show manager -> mixer -> WebGL2 -> HDMI TV
local MP4 files --HTTP/video----->  dual video decks       |
show.json + shaders --HTTP------->                         diagnostics
                                                             |
systemd -> Python control server <---health/control HTTP------+
              |
              +--> journal + ~/vj-results + shutdown/restart/display actions
```

The HTTP origin is required: WebHID does not work from `file://`. The Python server serves `gig/`, exposes system-control endpoints, and records state. Chromium owns rendering, video decode, live audio capture, controller events, and controller image uploads.

## Browser modules

| File | Responsibility |
|---|---|
| `gig/index.html` | Application shell, orchestration, per-page system keys, safety gestures, diagnostics, and kiosk UI |
| `gig/src/gl.js` | WebGL2 context, textures, buffers, render targets, and shader utilities |
| `gig/src/deck.js` | HTML video deck loading, loop/play state, texture update, and recovery |
| `gig/src/mixer.js` | Deck composition, shader layer, blend modes, effects, resolution scaling, and blackout |
| `gig/src/audio.js` | Web Audio capture and real-time feature extraction |
| `gig/src/isf.js` | ISF-style shader parsing and uniform mapping |
| `gig/src/show.js` | `show.json` parsing, page/key bindings, active states, navigation, and tap tempo |
| `gig/src/streamdeck.js` | Stream Deck XL WebHID protocol, reports, LCD image transfer, reconnect, and key edges |
| `gig/src/xtouch.js` | Optional X-Touch Mini WebMIDI input and LED feedback |
| `gig/src/choreo.js` | Beat clock, onset lock, modulation, and choreography |
| `gig/src/post.js` | Optional post-processing passes |

## Configuration and state

- `gig/show.json`: ordered Stream Deck pages and each clip/shader/effect binding.
- `gig/presets.json`: named multi-shader preset sequences used by optional X-Touch controls.
- `gig/shaders/*.fs`: fragment shader programs and metadata.
- `~/.config/chromium`: persistent WebHID and audio grants. Deleting it requires interactive pairing again.
- `~/vj-results`: runtime logs, health/state snapshots, and diagnostic artifacts on the Pi.
- systemd user services: `vj-server.service` starts the local server; `vj-kiosk.service` starts Chromium.

## Deployment model and weak points

The historical design launches the server and kiosk inside a Raspberry Pi OS graphical user session. Its supervision can restart a failed process, but it cannot guarantee that the display session, Chromium profile lock, GPU context, WebHID grant, audio grant, or media decode path is healthy. A process being “active” is therefore weaker than the appliance being show-ready. This mismatch is the reason the next architecture introduces an explicit readiness gate and removes Chromium, the desktop, WebHID, and browser profiles.

## Repository map

| Directory | Meaning |
|---|---|
| `gig/` | Historical complete show implementation |
| `gig/deploy/` | Pi installation, systemd, udev, repair, diagnostics, and retired patches |
| `gig/probe/` | Browser/GPU/codec/audio/shader test pages |
| `gig/tools/` | Media, show-generation, sync, logging, and test utilities |
| `config/`, `scripts/`, `src/`, `native/` | Earlier native mpv/PipeWire prototype |
| `vj/` | Partially built modular Rust engine and ingest workspace |
| `docs/v2/`, `docs/handoff/` | Architecture research and handoffs for native directions |
| `tasks/GIG-2026-08-29/` | Historical task specifications and results for the gig stack |
| `tasks/APPLIANCE-V3/` | Future appliance implementation work packages |

Historical backup and `_superseded` files are retained to preserve investigation context. They are not installation entry points.
