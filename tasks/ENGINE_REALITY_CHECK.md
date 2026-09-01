# Engine Reality Check — Audit of the `vj/` Rust Workspace
**Auditor:** Claude Opus 5 · **Date:** 2026-08-25
**Status:** AUTHORITATIVE. Do not re-litigate. Supersedes status claims in
`CLAUDE_COLLABORATION_BRIEFING.md` §3 and the `DONE` column of the old TRACKER.

## Method
Read every `Cargo.toml` in `vj/crates/*` and `vj/tools/*`, plus the head of each
implementation module and all of `vj/web/app.js`'s render path.

## Finding 1 — the workspace has no I/O dependencies at all
Every crate in `vj/` depends on exactly four external crates: `serde`,
`serde_json`, `toml`, `thiserror`. There is no `cpal`, `pipewire`, `hidapi`,
`midir`, `gstreamer`, `wgpu`, `glow`, `tokio`, `warp`, or Ableton Link binding
anywhere in the tree.

Therefore the following are **models with the I/O boundary defined but not
crossed** — not drivers, servers or engines:

| Claimed | Actually |
|---|---|
| TASK-06 Stream Deck XL HID driver | VID/PID consts + a hash-diffed icon cache. No USB write. |
| TASK-08 PipeWire/CPAL capture | An `AudioStreamConfig` struct + a ring buffer. No device is opened. |
| TASK-09 GStreamer hardware HEVC | `GST_PIPELINE_TEMPLATE` is a `&str`. Frames are an `AtomicU64` counter. |
| TASK-10 ISF host & compositor | `vj-render` has no GPU dependency. It composites nothing. |
| TASK-12 Warp WebSocket server | Serializable summary structs. No server binds a port. |
| TASK-13 Ableton Link | A `LinkSession` struct with a `bpm: f64`. No session on the network. |

The 62 tests are real tests of real logic, but they test the model against
itself. None can fail because a hardware assumption is wrong.

## Finding 2 — `vj/web/app.js` is a canvas-2D mock, not a shader compositor
`app.js` calls `canvas.getContext("2d")` and `drawImage`. There is no
`getContext("webgl2")`, no `createShader`, and **no code path anywhere in the
repo that reads a `.fs` file**. The 14 shaders in `vj/shaders/` have never been
compiled or executed by anything.

## Finding 3 — what IS real and valuable
- `vj-types`, `vj-core`, `vj-surface`: clean domain model, good trait
  boundaries, genuinely reusable.
- `vj-audio/analyzer.rs`: the 5-band crossover + EMA + kick-onset logic is
  correct and portable. **Port the band edges and cooldown, don't redesign.**
- `vj-surface-hid/device.rs`: the Stream Deck XL report constants
  (1024-byte reports, 8-byte header, 96×96) are correct and reusable.
- `vj/shaders/*.fs`: 14 real GLSL files. The most portable asset in the repo.
- `src/` (root, non-workspace): the older mpv + hidapi Pi player. This one has
  **real** `hidapi` and mpv-IPC dependencies. It is the only code in the repo
  that has ever tried to touch hardware.
- `STATUS.md` was honest all along and says most of this. It was not carried
  forward into the newer briefings.

## Consequence for planning
The Rust engine is 3–6 weeks from first pixel, not 4 days. It is not the gig
path. See `GIG-2026-08-29/GIG_PLAN.md` for the pivot and its rationale.
Nothing here is deleted — the model layer is the spec for the eventual engine.
