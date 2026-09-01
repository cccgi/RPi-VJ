# Cross-Platform VJ Engine — Program Overview

**Status:** authoritative as of 2026-08-22. Supersedes `RESEARCH.md`, `rpi-vj-architecture.md`, `docs/*.md`, and `docs/handoff/*.md` where they disagree. Those remain valid as background research; see [`01-RESEARCH-EVALUATION.md`](01-RESEARCH-EVALUATION.md) for what survives from each.

## What changed since the earlier research

Three things, and they all widen the scope:

1. **Control surfaces are now plural.** The prior research assumed Stream Deck XL was the control surface. The real requirement is six heterogeneous devices — one USB HID device and five class-compliant USB MIDI devices, with three different LED feedback models and two different encoder models between them. That is not a feature bolted onto the Stream Deck code path; it is a subsystem.
2. **The Pi is no longer the target — it is the *first* target.** The system must also run on an i-series/RTX 3090 Windows workstation and an M2 Pro MacBook Pro. That invalidates the GLES-3.1-only render decision and forces the video decode path behind an abstraction.
3. **The DJ software is now in scope as a sync source.** rekordbox and Serato both speak Ableton Link. Audio-only beat detection stops being the only tempo authority.

## Product definition

A single Rust engine, one codebase, three hardware branches, that:

- plays and cuts local video loops across multiple decks with beat-quantized triggering;
- renders audio-reactive ISF/GLSL shader layers;
- composites video and shader layers with real blend and key modes;
- is driven by any combination of Stream Deck XL, APC Key 25, Akai Fire, MPK mini, Launchpad Mini MK3, X-Touch Mini — hot-pluggable, simultaneously, with correct LED/display feedback on each;
- takes tempo and phase from audio analysis *and* Ableton Link, with an explicit authority policy;
- runs headless as a Pi 5 appliance, and as a windowed desktop application on Windows and macOS, from one show file.

The near-term goal is your mobile rig. The architecture's job is to make sure the near-term goal doesn't foreclose the long-term one.

## The four decisions that define the program

| Decision | Choice | Rationale | ADR |
|---|---|---|---|
| Render API | **wgpu everywhere** — Vulkan on Pi and PC, Metal on Mac | One compositor, one shader toolchain, three backends. GLES/EGL survives only as a Pi escape hatch behind a measured gate. | ADR-100 |
| Control model | **Virtual control surface with named regions** | Devices project their physical topology onto abstract regions (`clip.grid`, `layer.*.opacity`, …). Adding a seventh controller becomes a TOML profile, not an engine change. | ADR-101 |
| Show portability | **Capability tiers with declared degradation** | One show file. `Appliance` (Pi), `Portable` (M2 Pro), `Workstation` (3090). The show declares what it needs; the engine degrades layer count and render scale predictably rather than failing. | ADR-102 |
| Delivery | **Two parallel tracks over shared crates** | Track A hardens the Pi rig for your gigs on the existing mpv slice. Track B builds the portable core. `vj-types`, `vj-surface`, and `vj-devices` are written once and consumed by both — Track A's controller work *is* Track B's controller work. | ADR-105 |

## Document set

| Document | What it settles |
|---|---|
| [`01-RESEARCH-EVALUATION.md`](01-RESEARCH-EVALUATION.md) | What each of the four prior research passes got right, what to discard, and adjudication of the six direct contradictions between them. |
| [`02-TARGET-ARCHITECTURE.md`](02-TARGET-ARCHITECTURE.md) | Component boundaries, thread ownership, data flow, render graph, failure behavior. |
| [`03-CONTROL-SURFACES.md`](03-CONTROL-SURFACES.md) | The virtual surface model, device profile schema, and per-device protocol research for all six controllers. |
| [`04-PLATFORM-BRANCHES.md`](04-PLATFORM-BRANCHES.md) | Pi 5 / RTX 3090 / M2 Pro divergence: decode, import, present, audio, capability tiers. |
| [`05-TEMPO-AND-SYNC.md`](05-TEMPO-AND-SYNC.md) | Tempo authority model, Ableton Link integration, quantized launch scheduler. |
| [`06-REPO-LAYOUT.md`](06-REPO-LAYOUT.md) | Cargo workspace, crate responsibilities, dependency rules, key type contracts. |
| [`07-ROADMAP.md`](07-ROADMAP.md) | Track A and Track B phases, shared work, exit criteria, effort estimates. |
| [`08-VALIDATION.md`](08-VALIDATION.md) | Gates A–J, per platform, with pass conditions. |
| [`adr/README.md`](adr/README.md) | ADR-100 through ADR-108. |

## Non-negotiable constraints carried forward

These came out of the earlier research and remain true:

- Pi 5 hardware-decodes HEVC only. H.264 is software (~50–60% CPU at 1080p60). Never design a Pi H.264 zero-copy path.
- A 16.67 ms render frame budget means no disk I/O, no shader compilation, no image encoding, no blocking IPC, no device enumeration, and no unbounded allocation on the render thread.
- Never allocate, lock, log, or touch the filesystem inside an audio capture callback.
- "Page cache warm" is not "frame ready." Only a live decoder holding a presentable frame satisfies a sub-50 ms trigger. See [`01`](01-RESEARCH-EVALUATION.md) §3.5.
- Zero-copy must be *proven* from negotiated buffer types and DRM/API state, never inferred from an option named `hwdec`.
- On the Pi, DRM/KMS ownership excludes a desktop compositor.

## New constraints introduced by this document

- No control surface adapter may call GPU APIs, decode video, extract thumbnails, or encode images. Feedback images are produced at index time and by a dedicated feedback worker, never on the HID/MIDI thread.
- No engine component may name a specific controller. Devices exist only as profiles and drivers behind `ControllerAdapter`.
- Shader source is compiled at index/startup/explicit-reload time only. A live trigger never invokes a compiler on any platform.
- Platform-specific code lives only in `vj-video`, `vj-audio`, `vj-render`'s surface setup, and `vj-surface-*` transports. Everything else must build for all three targets unchanged.
