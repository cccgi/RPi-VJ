# This directory is DEAD. Do not use it.

`vj/web/` is the original canvas-2D mock (see `tasks/ENGINE_REALITY_CHECK.md`).
It uses `canvas.getContext("2d")`, has no WebGL, and never loaded a `.fs` file.

The real, working engine is **`gig/`** — WebGL2, Web Audio, WebHID, WebMIDI,
verified on the Pi 5 on 2026-08-25.

Opening `file:///.../vj/web/index.html` shows a mock, not the show.
The show is served at `http://localhost:8080/gig/` on the Pi.
