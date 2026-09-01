# TRACK B BOARD — Portable core (RTX 3090 PC + M2 Pro Mac)
**Status file for Track B only. `tasks/GIG-2026-08-29/BOARD.md` stays the gig's status file — do not merge them.**
Protocol: `AGENT_PROTOCOL_V2.md`. Roadmap: `../docs/v2/07-ROADMAP.md` (Phase B1, "GPU backend spike").
Specs: `specs/TRACKB/`. Results: `RESULTS-TRACKB.md`.
Target tree: everything below lives inside the existing `vj/` Cargo workspace. Gig code in `gig/` is untouched.

Per `docs/v2/07-ROADMAP.md` sequencing, B1 (this board) is the item that runs in parallel with A3 — it is a
separate machine (3090 PC, then Mac) and separate headspace from the Pi gig work, so it can start now.
History: this board started from zero `wgpu`/`winit`/`pollster` dependencies in `vj/`. TB-01..TB-07 closed on
2026-08-27 and their outcome is written up in `docs/v2/adr/ADR-100-render-backend.md`; TB-08..TB-12 (round 2)
are what Gate B still needs.

| ID | Task | File(s) | Model | Blocks | Status |
|---|---|---|---|---|---|
| **TB-01** | Add wgpu/winit/pollster deps, feature-gate by platform | `vj/Cargo.toml`, `vj/tools/gpu-spike/` | cheap | TB-02 | DONE |
| **TB-02** | wgpu device init + clear-color window on Windows/3090 | `vj/tools/gpu-spike/src/main.rs` | cheap | TB-03 | DONE |
| **TB-03** | Same spike binary, verified on macOS/M2 Pro (Metal) | same files as TB-02, platform run only | cheap | TB-04 | DONE |
| **TB-04** | Render one hardcoded triangle instead of clear-color | `vj/tools/gpu-spike/src/main.rs`, `shader.wgsl` | cheap | TB-05 | DONE |
| **TB-05** | Fullscreen quad + one ISF-lite fragment shader (static uniforms) | `vj/tools/gpu-spike/src/main.rs`, `quad.wgsl` | mid | TB-06 | DONE |
| **TB-06** | Live u_time/u_resolution uniforms + frame-time counter | `vj/tools/gpu-spike/src/main.rs` | mid | TB-07 | DONE |
| **TB-07** | CSV metrics log, run on both machines, feeds ADR-100 | `vj/tools/gpu-spike/src/metrics.rs` | mid | — | DONE |
| **ADR-100** | Backend write-up from TB-01..07 (architect task) | `docs/v2/adr/ADR-100-render-backend.md` | expensive | TB-08..12 | DONE (2026-08-27) |
| **TB-08** | gpu-spike first pixel on Pi 5 / V3DV — **not before 2026-08-29** | `vj/tools/gpu-spike/src/main.rs` | cheap | — | TODO |
| **TB-09** | Uncapped present mode + `cpu_ms` + p50/p99/p99.9 summary | `.../main.rs`, `.../metrics.rs` | mid | TB-10 | TODO |
| **TB-10** | 1080p offscreen target + Gate-B-weight shader + blend pass | `.../main.rs`, `heavy.wgsl` | mid | TB-11 | TODO |
| **TB-11** | Per-frame texture import and its copy cost (escape-hatch decider) | `.../main.rs`, `.../source.rs` | mid | TB-12 | TODO |
| **TB-12** | 30-min soak, external thermal/memory sampling, all three machines | none (run only) | cheap | — | TODO |

## Rules (same as AGENT_PROTOCOL_V2, repeated so nobody has to open that file mid-task)
1. One task = one conversation = one spec file in `specs/TRACKB/`. Worker opens the spec + at most the 1-2
   repo files it names — never the roadmap doc, never this board's rationale section.
2. Every task ends in an observable stated in the spec — not "it compiles."
3. Report ≤10 lines appended to `RESULTS-TRACKB.md`: files changed, the observable's actual output, surprises.
4. `DONE` = observed running on the named hardware (the 3090 box or the M2 Pro Mac). Compiling alone only earns
   `NEEDS-REVIEW` — say so in the report.
5. This board is the only Track B status file. Don't write a new summary doc, don't touch the gig board.

## Why this order (for the architect, not the workers)
TB-01→TB-04 is "does wgpu even come up on this box," split so a cheap model never debugs graphics-API setup and
shader logic in the same conversation. TB-03 is its own task, not folded into TB-02, because a Mac-side failure
is a different failure mode than a Windows-side one and shouldn't block the Windows worker from going DONE.
TB-05→TB-07 is what B1's actual exit criteria need (ADR-100 wants a measured backend comparison, not a green
build). GPU temperature/memory logging is explicitly deferred out of TB-07 — nvml/IOKit are both platform-only
APIs and belong in their own task once someone decides it's worth the added dependency surface.

## Round 2 — why TB-08..TB-12 exist (for the architect, not the workers)
ADR-100 is written (`docs/v2/adr/ADR-100-render-backend.md`) and it says wgpu is **accepted for the two desktop
tiers on portability evidence** — the same source, zero platform branches, Vulkan on the 3090 and Metal on the
M2 Pro, TB-03 needing 0 code changes. That was the cheaper half of B1's risk and it is closed.

Gate B itself is **not** closed, on any tier. TB-01..07 measured a 960x540 procedural shader under vsync with
nothing ever uploaded to the GPU, and never touched the Pi. So: TB-09 makes the timings mean something
(Fifo hides all headroom, and Gate B is a p99/p99.9 gate), TB-10 puts a Gate-B-weight workload behind them,
TB-11 produces the one number the GLES escape hatch actually turns on, TB-12 covers the 30-minute memory and
thermal conditions, and TB-08 visits the only platform the fallback was ever for.

Sequencing: TB-09 → TB-10 → TB-11 chain on the desktops (each adds CSV columns the next one keeps), TB-12 last
on the final binary. **TB-08 and the Pi leg of TB-11/TB-12 must not go near the Pi before the 2026-08-29 gig.**

B2 (ISF host) does not wait for any of this — it depends on the portability answer, which is settled. TB-10's
list of GLSL constructs that didn't port cleanly is direct input to B2's translator; ask for it before writing
those specs.
