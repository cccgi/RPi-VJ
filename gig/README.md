# gig/ — Chromium/WebGL2 VJ engine for Sat 29 Aug 2026

Everything for the gig lives here. **Do not modify `../vj/` or `../src/`.**

Why a browser and not the Rust engine: `../tasks/ENGINE_REALITY_CHECK.md`
Plan and risks: `../tasks/GIG-2026-08-29/GIG_PLAN.md`
Status: `../tasks/GIG-2026-08-29/BOARD.md`

```
probe/    capability + codec tests — run these on the Pi FIRST
src/      gl.js isf.js audio.js deck.js mixer.js streamdeck.js show.js post.js choreo.js
shaders/  the 14 .fs files, copied from ../vj/shaders (originals untouched)
deploy/   udev rules, systemd kiosk unit
tools/    serve.sh, transcode.sh
```

Run: `tools/serve.sh` then open `http://localhost:8080/gig/`
(must be `http://localhost`, not `file://` — WebHID needs a secure context)
