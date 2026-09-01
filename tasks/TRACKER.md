# RPi-VJ Task Tracker

> ## ⚠️ READ FIRST — 2026-08-25
> **Active work has moved to [`GIG-2026-08-29/BOARD.md`](GIG-2026-08-29/BOARD.md).**
> There is a gig on **Sat 29 Aug**. That board is the only live status file.
>
> The `DONE` marks below are **not** what they appear to be. An audit
> ([`ENGINE_REALITY_CHECK.md`](ENGINE_REALITY_CHECK.md)) found the `vj/` workspace
> has *no* hardware dependencies at all — no cpal, hidapi, gstreamer, wgpu, tokio
> or Ableton Link anywhere. Tasks 06, 08, 09, 10, 12 and 13 are **logic models
> with the I/O boundary defined but never crossed**, not drivers. Nothing in the
> repo has ever compiled a `.fs` shader or opened a device.
> The status column below has been corrected to say so.
>
> Working protocol is now [`AGENT_PROTOCOL_V2.md`](AGENT_PROTOCOL_V2.md) —
> small self-contained specs for cheap models, one task per conversation.
> `ORCHESTRATION_WORKFLOW.md` is retired.
>
> **Track B (portable core, PC+Mac) now has its own board:** [`TRACK-B-BOARD.md`](TRACK-B-BOARD.md),
> broken into cheap/mid micro-specs in [`specs/TRACKB/`](specs/TRACKB/). Starts at TB-01 (2026-08-27).

## Legacy board — Rust engine (paused until after the gig)

`MODEL` = domain logic + tests, no I/O. `REAL` = verified against hardware.
Nothing in this table is `REAL`.

| Task ID | Task Name | Component / Crate | Assigned Model | Status | Review Log |
|---|---|---|---|---|---|
| **TASK-00** | Show Manifest & 4x8 Bank Schema | `vj-types` | Orchestrator | **DONE** | [`REVIEW-00-05.md`](reviews/REVIEW-00-05.md) |
| **TASK-01** | Automated Transcode & Ingest Engine | `vj-ingest` | Orchestrator | **DONE** | [`REVIEW-00-05.md`](reviews/REVIEW-00-05.md) |
| **TASK-02** | Resolume-Style Studio UI (4x8 Matrix) | `vj/web` | Orchestrator | **DONE** | [`REVIEW-00-05.md`](reviews/REVIEW-00-05.md) |
| **TASK-03** | 5-Band Audio DSP & Kick Detector | `vj-audio` | Orchestrator | **DONE** | [`REVIEW-00-05.md`](reviews/REVIEW-00-05.md) |
| **TASK-04** | Curated Audio-Reactive ISF Shaders | `vj/shaders` | Orchestrator | **DONE** | [`REVIEW-00-05.md`](reviews/REVIEW-00-05.md) |
| **TASK-05** | Pi 5 Appliance Boot & Packaging | `vj/deploy/pi` | Orchestrator | **DONE** | [`REVIEW-00-05.md`](reviews/REVIEW-00-05.md) |
| **TASK-06** | Stream Deck XL Real HID Driver & Icon Cache | `vj-surface-hid` | Worker | **MODEL** | [`REVIEW-00-06.md`](reviews/REVIEW-00-06.md) |
| **TASK-07** | Behringer X-Touch Mini MIDI Driver & Rings | `vj-surface-midi` | Worker | **MODEL** | [`REVIEW-00-06.md`](reviews/REVIEW-00-06.md) |
| **TASK-08** | Sound Blaster Play! 3 PipeWire Stream | `vj-audio` | Worker | **MODEL** | [`REVIEW-00-06.md`](reviews/REVIEW-00-06.md) |
| **TASK-09** | Dual-Deck GStreamer Hardware HEVC Loop | `vj-video` | Worker | **MODEL** | [`REVIEW-00-06.md`](reviews/REVIEW-00-06.md) |
| **TASK-10** | ISF Fragment Shader Host & Compositor | `vj-render` / `vj-isf` | Worker | **MODEL** | [`REVIEW-00-06.md`](reviews/REVIEW-00-06.md) |
| **TASK-11** | Main App End-to-End Wiring & Loop Integration | `vj-app` | Worker | **MODEL** | [`REVIEW-11-13.md`](reviews/REVIEW-11-13.md) |
| **TASK-12** | Operator Live Web Remote & Telemetry | `vj-api` | Worker | **MODEL** | [`REVIEW-11-13.md`](reviews/REVIEW-11-13.md) |
| **TASK-13** | Ableton Link Tempo & Phase Sync | `vj-tempo` | Worker | **MODEL** | [`REVIEW-11-13.md`](reviews/REVIEW-11-13.md) |
| **FEATURE** | HTML5 Drag & Drop Video Ingest & Theater Modal | `vj/web` | Worker | **DONE** | Integrated in `app.js` |
| **FEATURE** | Active Playing Status Halos & One-Click Stop | `vj/web` | Worker | **DONE** | Integrated in `style.css` & `app.js` |
| **FEATURE** | Master HDMI Live Preview (canvas-2D mock, not WebGL) | `vj/web` | Worker | **MOCK** | Integrated in `index.html` & `app.js` |
| **FEATURE** | 3D Raymarching ISF Shaders (never compiled — see G-04) | `vj/shaders` | Worker | **UNTESTED** | 5 New Shaders in `vj/shaders/` |
| **FEATURE** | USB Export & Pi Sync Tooling | `vj-ingest` | Worker | **DONE** | Added `export-usb` & `sync` CLI |

---

## Multi-Agent Collaboration Briefings (Claude Opus 5 & Antigravity)
*   📄 **Master Architecture Briefing:** [`CLAUDE_COLLABORATION_BRIEFING.md`](CLAUDE_COLLABORATION_BRIEFING.md)
*   📸 **System State & Codebase Snapshot:** [`SYSTEM_STATE_SNAPSHOT.md`](SYSTEM_STATE_SNAPSHOT.md)
*   🔄 **Collaboration Protocol:** [`ORCHESTRATION_WORKFLOW.md`](ORCHESTRATION_WORKFLOW.md)

---

## Phase 4 — deferred until after 2026-08-29

TASK-14 (Spout/Syphon), TASK-15 (more MIDI controllers) and TASK-16 (LFO matrix)
are **on hold**. All three add a layer on top of an I/O layer that does not exist
yet; building them makes the board longer and the gig no closer.

Phase 4 resumes after the gig, reordered to close the boundary first:
1. wgpu init + one triangle, on the 3090 box then on the Pi under DRM/KMS (Gate B)
2. one real `.fs` shader through `vj-isf` into that window with a live audio uniform
3. Stream Deck XL over `hidapi` on the PC
4. `gstreamer-rs` HEVC decode on the Pi, measured

The browser build shipped for the gig will by then have answered most of the
hardware questions these steps depend on — real fps numbers, the real codec,
the real Stream Deck report layout. That is the main reason it is worth doing
even setting the gig aside.

## Status Legend
*   `TODO` : Spec written and ready for worker to implement.
*   `IN_PROGRESS` : Worker currently editing code.
*   `IN_REVIEW` : Worker finished code + unit tests; awaiting review.
*   `REVISE` : Review found issues; worker needs to fix.
*   `DONE` : Reviewed, tests passed, code verified.
