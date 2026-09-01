# RPi-VJ

RPi-VJ is an experimental Raspberry Pi 5 visual-performance system controlled primarily by an Elgato Stream Deck XL. It combines looping video, audio-reactive GLSL visuals, compositing, effects, beat controls, and a full-screen HDMI program output.

> **Branch status — `original`: historical baseline.** This branch preserves the system used and developed for the 29 August 2026 show. Its functional gig implementation uses Raspberry Pi OS Desktop, Chromium, WebGL2, Web Audio, and WebHID. It is feature-rich but is **not appliance-grade**: kiosk startup and browser/device permission recovery have known field failures. The planned desktop-free replacement is specified in [`tasks/APPLIANCE-V3/`](tasks/APPLIANCE-V3/README.md), but is not implemented on this branch.

## Start here

- [Features and project status](docs/original/FEATURES.md)
- [Hardware, software, installation, and first run](docs/original/INSTALLATION.md)
- [Stream Deck pages and every button](docs/original/STREAMDECK.md)
- [Media preparation and show configuration](docs/original/MEDIA.md)
- [Architecture and repository map](docs/original/ARCHITECTURE.md)
- [Operations and troubleshooting](gig/RUNBOOK.md)
- [Known appliance reliability problems](gig/APPLIANCE_RELIABILITY.md)

## What is usable today?

| Path | Purpose | Status |
|---|---|---|
| [`gig/`](gig/README.md) | Complete browser-based show engine and deployment scripts | Original working implementation; feature-complete enough for a show, but startup/device reliability is not production-grade |
| [`src/`](src) | Small Rust/mpv/Stream Deck proof of concept | Prototype; not the gig application |
| [`vj/`](vj/README.md) | Modular native Rust architecture (`vj-app`, surfaces, video, render, ingest) | Partially built scaffold; substantial backends are absent |
| [`tasks/APPLIANCE-V3/`](tasks/APPLIANCE-V3/README.md) | Desktop-free native appliance implementation plan | Design/tasks only on this branch |

## Fast local preview

The browser engine can be viewed on any machine with Python 3 and a WebGL2-capable Chromium-family browser:

```sh
cd gig
python3 -m http.server 8080
```

Open `http://localhost:8080`. The UI and shaders work without a Pi, but Stream Deck WebHID access, audio capture permissions, Pi deployment services, and hardware performance must be tested on the target device. Historical media is deliberately omitted from Git.

## License

Source code is provided under the [MIT License](LICENSE). Third-party media is not included or licensed by this repository.
