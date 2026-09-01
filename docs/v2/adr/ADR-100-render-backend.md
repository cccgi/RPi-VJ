# ADR-100 — Render backend: wgpu everywhere, GLES as a measured Pi fallback

**Supersedes** the inline ADR-100 stub in `docs/v2/adr/README.md` (that entry now points here).
**Gate:** Gate B, `docs/v2/08-VALIDATION.md`.
**Evidence:** `tasks/RESULTS-TRACKB.md` (TB-01 … TB-07), `vj/tools/gpu-spike/gpu-spike-results-pc.csv`,
`vj/tools/gpu-spike/gpu-spike-results-mac.csv`.
**Written:** 2026-08-27, after Track B TB-01–TB-07 closed on the RTX 3090 PC and the M2 Pro MacBook.

---

## Status

| Tier | Backend | Status |
|---|---|---|
| `Workstation` (RTX 3090 / Windows) | wgpu → Vulkan | **Accepted.** wgpu comes up, renders, presents. Gate B numbers not yet taken. |
| `Portable` (M2 Pro / macOS) | wgpu → Metal | **Accepted.** Same binary, zero platform `cfg`, zero source changes. |
| `Appliance` (Pi 5 / V3DV) | wgpu → Vulkan, GLES/EGL retained | **Provisional. No measurement exists.** |

Gate B is **not** closed on any tier. What TB-01–TB-07 closed is the *portability* question, which was the
cheaper half of the risk. The *performance* question is untouched — see "What the evidence does not show".

## Decision

1. **One render core on wgpu.** Confirmed for the two desktop tiers by evidence, not by argument. B2 (ISF host)
   is unblocked and starts against wgpu/naga.
2. **The GLES/EGL escape hatch stays open, and stays Pi-only.** It is not retired by desktop results — it was
   never for the desktop. It closes when, and only when, TB-08/TB-11 show V3DV carrying the Gate B workload
   including frame import.
3. **No second renderer is authorised for any desktop tier**, on any grounds, without a measured Gate B failure.
4. **No GPU-telemetry crates** (`nvml-wrapper`, IOKit/IOReport). Temperature and VRAM are sampled by external
   processes running alongside the spike (`nvidia-smi --query-gpu`, `powermetrics`, `vcgencmd measure_temp`)
   and joined on wall time. This keeps the deferred TB-07 field from becoming three platform-specific
   dependencies inside the render crate.

## What the evidence shows

- The same `gpu-spike` source, with `wgpu::Backends::PRIMARY` and no platform branches, selected Vulkan on the
  3090 and Metal on the M2 Pro. TB-03 required **0 code changes** on macOS. That is the single most valuable
  result on this board: the naga/wgpu abstraction held across two backends without a fork.
- A hand-ported ISF-lite fragment shader (`quad.wgsl`, a port of `gig/shaders/cyber_grid.fs`) compiles and
  renders correctly on both, with live `u_time`/`u_resolution` uniforms, resize handling, and 0 validation
  errors. The GLSL→WGSL semantic gaps that appeared (UV origin flip, `vec2` 16-byte alignment padding) are
  exactly the class B2's translator has to handle, and they were small.
- Both machines sustain the vsync cadence (~60.2 fps over 36.0 s / 2166 frames on the PC; ~59.8 fps over 40.2 s
  / 2404 frames on the Mac) with no crash, stall, or surface loss.

## What the evidence does **not** show

These four gaps are why the ADR is not final. Each maps to a dispatchable task below.

1. **The frame times are vsync intervals, not render cost.** `present_mode: Fifo` caps presentation at the
   display refresh, and `frame_ms` is measured start-of-frame to start-of-frame, so it includes the vsync wait.
   The steady ~16.67 ms says "the GPU finished in time," not how much headroom was left. Gate B's p99 < 16.0 ms
   and p99.9 < 16.67 ms cannot be evaluated from a vsync-locked wall-clock series at all. → **TB-09**
2. **The workload is roughly two orders of magnitude below Gate B's.** Gate B is one decoded 1080p60 source
   plus a moderate full-screen shader plus a blend/colour pass. The spike is one procedural shader at
   **960×540** with no video, no blend, no colour conversion. → **TB-10**
3. **Nothing has ever been imported to the GPU.** The entire escape-hatch clause in ADR-100 — "selected only if
   wgpu cannot import decoded frames without per-frame CPU copies" — turns on a measurement that does not
   exist. Gate B's "no per-frame CPU readback" is currently unverifiable. → **TB-11**
4. **The Pi has not been touched.** V3DV's Vulkan 1.3 conformance is a claim from research, not an observation
   from our hardware, and the Appliance tier is the only tier the GLES fallback was ever for. → **TB-08**

Additionally: the 36 s and 40 s runs say nothing about memory growth or thermal throttling, both of which are
explicit Gate B pass conditions over 30 minutes. → **TB-12**

## Consequences

- **B2 (ISF host) may start now.** It depends on the portability answer, which is settled, not on the
  performance answer, which is not. The shaderc → SPIR-V → naga pipeline is the committed path.
- **B1 stays open** until TB-08–TB-12 report. This ADR is revised in place with their numbers, not replaced.
- If TB-11 shows a per-frame CPU copy on the Pi's import path, the consequence recorded in the original
  ADR-100 applies as written: ISF assets need both SPIR-V and GLSL-ES compilation paths in the cache, and that
  cost is paid visibly. It is not yet known whether that bill arrives.
- The pre-existing `vj-types` `DeckId: Default` compile error (TB-01, confirmed in review) means
  `cargo build --workspace` still fails; `cargo build -p gpu-spike` is what Track B has been building. That is
  a legacy defect, out of Track B's scope, and needs its own ticket before B2 consumes `vj-types`.
- `wgpu` 0.20.1 / `winit` 0.29.15 are pinned **for the spike only**. Whether B2 bumps them is a B2 decision,
  taken against ISF-host needs, not inherited silently from this tool.

## Follow-on tasks (Track B board, `tasks/TRACK-B-BOARD.md`)

| ID | Closes | Model | Machine |
|---|---|---|---|
| TB-08 | gap 4 — Pi 5 / V3DV first pixel | cheap | Pi 5 — **after the 2026-08-29 gig** |
| TB-09 | gap 1 — uncapped GPU frame time + percentiles | mid | PC, then Mac |
| TB-10 | gap 2 — 1080p, Gate-B-weight shader | mid | PC, then Mac |
| TB-11 | gap 3 — texture import path and its copy cost | mid | PC, then Mac, then Pi |
| TB-12 | soak — 30 min, memory and thermals | cheap | all three |

TB-09 and TB-10 are desktop-only and can start immediately. TB-08 must not go near the gig Pi before
2026-08-29. TB-11 is the one that decides the escape hatch and is the highest-value task on the list.
