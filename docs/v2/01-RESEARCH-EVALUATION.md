# Evaluation of the Four Prior Research Passes

Four independent research passes exist in this repository. They overlap heavily, disagree in six places, and each contributes something the others missed. This document names the contribution, names the error, and adjudicates every contradiction so the next implementer stops re-litigating them.

## 1. The four sets

| Set | Files | Character | Single best contribution |
|---|---|---|---|
| **A** | `RESEARCH.md` | Playback-first. mpv subprocess, `elgato-streamdeck`, PipeWire quantum tuning. Aspirational multi-crate scaffold. | The mpv DRM/KMS flag set and *why* subprocess beats libmpv for crash isolation. Concrete and correct. |
| **B** | `rpi-vj-architecture.md` | Shader-first. Engine survey, ISF adoption, audio uniform contract, DSP design. | The audio-reactive uniform contract and the **dual-rate EMA** anti-flicker technique. This is the best DSP thinking in the corpus and it is still correct. |
| **C** | `docs/HANDOFF.md`, `docs/RESEARCH_FINDINGS.md`, `docs/ARCHITECTURE.md`, `docs/IMPLEMENTATION_PLAN.md` | Skeptical/operational. Zero-copy verification discipline, state machine, service isolation. | The **three-tier caching taxonomy** (§3.5 below). This is the sharpest single insight in the whole corpus and it invalidates Set A's latency strategy. |
| **D** | `docs/handoff/*` | Contract-first. Typed commands/events with request lifecycle, two-deck prepared switching, ISF embedded profile, validation gates A–G. | The **`TriggerClip` → `CommandAccepted` → `ClipPrepared` → `ClipActivated` lifecycle**. Acknowledging a command is not the same as showing a frame, and only Set D models that. |

`STATUS.md` is not a research pass but is the most valuable document in the repo: it is an honest inventory of what was compiled versus what was assumed. Its discipline should be preserved — every claim in this new document set is marked *verified*, *reasoned*, or *validate on target*.

## 2. What each set got wrong

**Set A** treated page-cache warming as the answer to the <50 ms trigger budget. It is not (§3.5). It also has no shader host at all, and its "hard cuts are what VJs actually want" claim is a rationalization of a missing feature — you use crossfades in Resolume today.

**Set B** recommended a hand-rolled GLES 3.1 core on `glow`. That was defensible for a Pi-only appliance and is the wrong call the moment macOS enters scope: Metal has no GLES path, and you would be writing a second renderer. It also declared `im-knots/varda` and `DatanoiseTV/audiovis` nonexistent, which Set D contradicts (§3.2).

**Set C** is the most careful set and its main weakness is that its care never resolves into a decision — it correctly demands that zero-copy be proven and then leaves the backend open. Good discipline, no commitment.

**Set D** committed to GStreamer behind `VideoDeck` (ADR-004). The abstraction is right; naming GStreamer as *the* implementation is premature for a codebase that will need NVDEC on Windows and VideoToolbox on macOS. GStreamer becomes one backend among three, not the target.

All four sets share one blind spot: **every one of them models control input as "the Stream Deck."** Set D gets closest with `ControllerAdapter`, but its `ControllerFeedback` type still assumes bitmap keys. None of them anticipate velocity-palette LEDs, RGB SysEx grids, relative encoders with LED rings, or an OLED strip.

## 3. Adjudicated contradictions

### 3.1 Render API — GLES 3.1 custom core (B) vs. wgpu/Vulkan with GLES fallback (C, D)

**Resolution: wgpu everywhere.** ADR-100.

Set B's reasoning was sound for its stated scope and is invalidated by scope change, not by error. With Windows and macOS in scope, wgpu gives one render-graph implementation across Vulkan (Pi + PC) and Metal (Mac), and one shader toolchain (ISF/GLSL → SPIR-V via shaderc → naga → backend). The V3DV Vulkan 1.3 conformance both sets cite is the enabling fact.

The cost Set B correctly identified is real and is now a scheduled experiment rather than an objection: wgpu's ability to import decoded video frames without per-frame CPU copies is unproven on the Pi stack. That is Gate B in [`08-VALIDATION.md`](08-VALIDATION.md), and GLES/EGL remains the documented Pi-only escape hatch if it fails. If the escape hatch is taken, it is taken *only* for the Pi compositor, and it is recorded honestly rather than hidden.

### 3.2 Do `im-knots/varda` and `DatanoiseTV/audiovis` exist?

**Resolution: unresolved, and deliberately made not to matter.**

Set B searched and reported neither repository found. Set D cites both with URLs and describes their internals in specific detail (audiovis: DRM/KMS via GBM/EGL, ISF multipass, persistent buffers, MIDI/OSC; varda: Rust, Vulkan/wgpu, x86-64 Linux artifacts, GLSL 450 shader bodies). A direct search pass for this document also failed to surface either.

That level of specificity in Set D is not the shape of a hallucination, but neither is it confirmable from here. **Action:** open both URLs yourself — it is a two-minute check, and you may have a bookmark or a private fork Set B could not see. Either way, this is bounded risk, because neither project is on the dependency path in this plan. Both are classified as *reference reading* — study the DRM/EGL and ISF-modulation patterns, vendor nothing. No milestone blocks on either.

### 3.3 Video backend — mpv subprocess (A, C) vs. GStreamer behind `VideoDeck` (D)

**Resolution: `VideoDeck` trait with per-platform backends. mpv is Track A only and is a dead end by design.**

Set D's abstraction is right and its implementation choice is too narrow. The final shape:

| Platform | Backend | Status |
|---|---|---|
| Pi 5 (Track A appliance) | mpv subprocess + JSON IPC | transitional; ships the gig rig, never gains shaders |
| Pi 5 (Track B) | GStreamer `v4l2slh265dec` → DMA-BUF → Vulkan external memory | primary |
| Windows / RTX 3090 | ffmpeg + NVDEC (or D3D11VA) → CUDA/Vulkan interop | primary |
| macOS / M2 Pro | VideoToolbox → `CVPixelBuffer`/IOSurface → Metal texture | primary |
| Any | HAP (Snappy + BC1/BC3) CPU decode → GPU upload | desktop tiers only, see §3.6 |

Set A's argument for subprocess isolation over libmpv remains correct *for the mpv path specifically* and is why Track A keeps it. It does not generalize: the moment the compositor needs decoded textures rather than final scanout, subprocess isolation is exactly what you cannot have.

### 3.4 Stream Deck: raw `hidapi` (current `src/streamdeck.rs`, endorsed by C) vs. `elgato-streamdeck` crate (A, D)

**Resolution: `elgato-streamdeck` crate.**

Set C's "a fixed appliance benefits from a visible protocol" argument holds only while the target is one fixed appliance. Across three OSes and two XL revisions, per-model report framing, image format (JPEG vs BMP), key rotation, and page-report sizing are exactly the maintenance you do not want to own. The crate is hidapi-backed, so the cross-platform transport story is identical either way.

Keep from the current implementation: the reconnect behavior and cached-icon replay. Those are genuinely good and should be lifted into the generic `vj-surface` feedback cache, where every device gets them.

### 3.5 Trigger latency — page-cache warming (A) vs. the three-tier taxonomy (C)

**Resolution: Set C, unambiguously. This is the most important correction in this document.**

Set C's taxonomy:

1. **Page-cache warm** — file bytes are in RAM. Removes storage latency only. Demux and decoder startup remain.
2. **Demux warm** — container metadata parsed, initial GOP read. Decoder startup remains.
3. **Frame ready** — a live decoder context holds the first displayable frame as a DMA-BUF / EGLImage / Metal texture.

Set A's `posix_fadvise(WILLNEED)` bank warming achieves tier 1 and its mpv playlist pre-append trick achieves tier 2. **Neither meets a hard p95 <50 ms budget.** Only tier 3 does.

This drives real architecture: a bounded **hot decoder slot pool** (4–8 slots on Pi, 16–32 on the 3090), populated by explicit cueing plus a recent-use policy, with cold clips falling back to a slower path that reports a degraded-latency metric rather than pretending. The engine must never blank the output to service a cold trigger. This is also why Set D's two-deck model and the `ClipPrepared`/`ClipActivated` split exist — they are the control-plane expression of the same fact.

The retained per-clip encoding rules from A and C: `faststart` (moov first), start every clip on a closed random-access point, short closed GOP, normalized resolution/frame-rate/pixel-format/color metadata at ingest.

### 3.6 Codec standard — HEVC everywhere (all four sets)

**Resolution: correct for the Pi, insufficient for the program. Introduce media tiers.**

HEVC Main 8-bit 4:2:0 is right for the Pi because it is the only hardware decode path on BCM2712. It is a mediocre choice on a 3090 or an M2 Pro, where the real VJ answer is an intra-only GPU-friendly texture codec — HAP is what Resolume uses, and it is nearly free to decode: Snappy decompress, then upload already-compressed BC1/BC3 blocks straight to the GPU.

The reason this is a *tier* and not a global switch: **V3D has no S3TC/BC texture support.** Mesa's V3D driver supports ETC1/ETC2 and, more recently, native ASTC — not BC1/BC3. HAP on the Pi would require a CPU BC→RGBA transcode per frame, which is worse than HEVC hardware decode. Confidence: high on the driver fact, *validate on target* on the practical transcode cost if you ever want a HAP clip to play on the Pi at all.

Resulting ingest policy:

| Tier | Primary format | Fallback | Notes |
|---|---|---|---|
| `Appliance` (Pi 5) | MP4 / HEVC Main 8-bit 4:2:0, closed GOP 15–30, faststart | H.264 flagged software-decode in the index | Main10 only after the full decode→import→display path is measured |
| `Portable` (M2 Pro) | HAP / HAP Q in MOV | HEVC via VideoToolbox | ProRes also viable; VideoToolbox handles it |
| `Workstation` (RTX 3090) | HAP / HAP Q | HEVC or H.264 via NVDEC | NVDEC makes codec choice nearly irrelevant; HAP still wins on seek/loop determinism |

The ingest tool (`vj-ingest`) produces every tier a show declares from one source master, and the clip index records which tiers exist for each clip. A show authored on the 3090 with HAP masters plays on the Pi from the HEVC renditions of the same clips, or refuses to load with a clear diagnostic naming the missing rendition.

## 4. What is carried forward verbatim

These survive unchanged and should not be re-derived:

- **Set B's audio uniform contract** — `u_bass`/`u_mid`/`u_treble`/`u_volume`/`u_beat_onset`/`u_beat_confidence`/`u_bpm` plus `u_spectrum`/`u_waveform` textures, extended by Set D with `u_beat_phase` and `u_fft_bins`. Adopted as the shader ABI in [`02-TARGET-ARCHITECTURE.md`](02-TARGET-ARCHITECTURE.md).
- **Set B's dual-rate EMA** — fast attack (~10–20 ms), slow release (~150–300 ms). A single symmetric EMA is the classic mistake and produces exactly the flicker you are trying to avoid. Combined with Set D's rate-independent form `alpha = 1 - exp(-dt/tau)`.
- **Set B/D's spectral-flux onset detection** with an adaptive threshold (rolling median + MAD per D, rather than mean × 1.5 per B — D's is more robust to a loud sustained passage) and a 100–160 ms refractory interval.
- **Set D's "do not normalize each spectrum by its own maximum"** — that makes silence look active. Use noise-floor and upper-envelope tracking instead. This is a real bug avoided.
- **Set D's typed-internal / JSON-at-boundaries rule** (ADR-006) and its fixed-layout `AudioFeatures` snapshot with no per-update allocation.
- **Set D's thread ownership table and prohibited-work column.** Extended in this plan with control-surface threads.
- **Set C's zero-copy verification evidence list** — decoder output caps showing DMA-BUF/DRM PRIME, no `videoconvert` or mapped frame in the hot path, DRM plane state showing expected formats/modifiers, CPU profiles free of per-frame memcpy, stable page-flip timing.
- **Set C's operational findings** — 5 A supply, active cooling, discover DRM connector and PipeWire node identities at startup rather than assuming numeric IDs, unprivileged service user with a narrow device allowlist, treat clip paths as untrusted input.
- **Set A's PipeWire quantum analysis** — 256 @ 48 kHz = 5.33 ms per period; 128 is achievable on paper and xruns in practice on the Pi's xHCI with class-compliant USB audio. Lock min = default = max on the appliance image so adaptive quantum growth cannot surprise you mid-set.
- **Set D's validation gates A–G.** Extended to J here.

## 5. What is discarded

- The multi-crate scaffold described in `RESEARCH.md` §4.1 (`vj-core`/`vj-video`/`vj-streamdeck`/`vj-audio`). It never matched the repository and the crate boundaries are wrong for the new scope. Superseded by [`06-REPO-LAYOUT.md`](06-REPO-LAYOUT.md).
- `vj-video/src/raw_drm.rs` as a concept. A hand-rolled ffmpeg + DRM atomic path is not justified on any of the three platforms and is not preserved even as a skeleton.
- "Hard-cut triggering only, by design." Crossfade and the other transitions are requirements, delivered by the two-deck model.
- Page-cache warming as a latency strategy (kept as a cheap tier-1 optimization, demoted from a headline claim).
- Any statement that the Pi supports GLES 3.2. It is conformant at 3.1.

## 6. Open items inherited and still open

| Item | Owner phase | Notes |
|---|---|---|
| `varda` / `audiovis` existence | You, 2 minutes | §3.2. No milestone depends on the answer. |
| Whether wgpu can import decoded frames on the Pi stack without per-frame copies | Gate B | The single highest-risk unknown in the program. |
| Whether V3DV Vulkan is stable enough for sustained 60 fps compositing | Gate B | GLES fallback exists precisely for this. |
| Button press/release edge diffing in `src/streamdeck.rs` | Track A, Phase A1 | Currently re-fires on every poll tick a key reads held. Known-broken. |
| PipeWire buffer alignment assumption in `native/pw_capture.c` | Gate D | `bytemuck` cast alignment is unverified against real negotiated buffers. |
| Real HID-to-photon latency | Gate C | Every <50 ms figure in the corpus is a reasoned budget, never a measurement. |
