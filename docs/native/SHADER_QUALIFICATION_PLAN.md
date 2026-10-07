# Shader bank qualification plan — 2026-10-07

Owner: project orchestrator. Scope: make the large Shadertoy-derived shader
bank run reliably on the Pi 5 (2 GB), benchmark every shader at 1280x720 and
1920x1080, and document what works. This plan supersedes item 3 of
`HANDOFF_2026-10-07.md` and revises the blueprint where noted.

## 1. Audit findings (verified 2026-10-07)

| # | Finding | Evidence | Consequence |
|---|---|---|---|
| F1 | The live service renders shaders at 1080p although the deployed bank was qualified at 720p. | `rpi-vj-native.service` sets no `RPI_VJ_RENDER_WIDTH/HEIGHT`; `render_size()` defaults to 1920x1080. Journal: ~47.5 frames/s, `late` rising ~14/s. | The "36-shader 720p bank" is not holding 60 FPS on stage. |
| F2 | The old benchmark's "zero late frames" criterion is meaningless. | `run_shader_only` prints a hard-coded `late=0`. | Only the FPS figure carried information. |
| F3 | A shader that fails to compile is benchmarked as a perfect 60 FPS black frame. | `compile_selected` stores the error and `draw` skips the shader pass; the loop keeps swapping. | The 15/36 "qualified" lists may contain non-rendering shaders. |
| F4 | 911 of 1,340 old measurements were `no_frame_sample`. | `shader-bench.log`. Cause: one KMS process per test, lazy compile inside the 8 s window, and a need for two 1 Hz status lines. Anything slower than ~0.3 FPS or slow to compile was lost. | Two thirds of the bank is unmeasured, not unusable. |
| F5 | vsync-capped FPS cannot express headroom. | Every passing shader reads "60". | No way to rank shaders or know how much budget is left for video. |
| F6 | `iChannel0-3` samplers are declared but never bound; buffer passes are never rendered. | `render.rs` binds no channel textures; `make_native_shadertoy.py` keeps only the Image pass. | Of 1,164 bank shaders: 791 are self-contained, ~222 need textures, 151 are multipass. The last two classes render incorrectly today regardless of speed. |
| F7 | Shader compilation happens on the render thread at first trigger. | `compile_selected` is called from `draw`. | A V3D compile stalls video and the whole output for its duration. |
| F8 | All native engine work is uncommitted (`?? engine/`). | `git status`. | A snapshot was taken at `target/snapshots/engine-pre-bench-2026-10-07.tar.gz`; a commit is recommended. |
| F9 | The native engine feeds the 18 originals fewer audio signals than they were written for. | `render.rs`: `u_sub` = bass, `u_low_mid` = `u_high_mid` = mid, `u_locked` = 0. Reported by WP-C. | The five-band originals react on three bands; phase-locked terms are off. Phase 3 item. |
| F10 | The generic audio wrapper on bank shaders does not match the originals' conventions and costs two per-pixel trig calls plus a vec3 `pow`. | `make_native_shadertoy.py`; WP-C section 5 of `SHADER_ADAPTATION_RULES.md`. | Wrapper cost is included in every bank measurement; a lighter, consistent wrapper is a WP-E candidate. |

## 2. Blueprint revisions

R1. **Per-shader render tier.** Each shader entry in the show manifest gains an
optional `render_height` (1080, 720, 540 or 360; default 1080). The renderer
sizes the shader target to that tier and the existing compositor upscales to
1080p output. Video stays 1080p. This is the primary mechanism for making heavy
shaders usable; the global `RPI_VJ_RENDER_*` environment override remains for
diagnostics only.

R2. **Benchmark is an engine mode, not a process loop.** `rpi-vj-native --bench
<manifest>` holds one KMS session, compiles and measures each shader offscreen
with `glFinish`-bounded GPU frame times, and appends JSONL. A supervisor
restarts it after a crash or hang and records the offending shader.

R3. **Qualification is measured cost against a budget**, not vsync FPS.
Budgets for the shader pass (p95 GPU ms): `SOLID` <= 10.0, `OK` <= 13.0,
`MARGINAL` <= 16.0, otherwise `FAIL` at that tier. A shader's assigned tier is
the highest resolution at which it is `SOLID` or `OK`. Final acceptance is an
in-situ vsync run of the chosen bank with a clip playing.

R4. **Content classes.** `selfcontained` shaders are first-class. `needs_texture`
shaders are benchmarked but flagged visually unverified until channel textures
are implemented. `multipass` shaders are excluded until a buffer graph exists.

R5. **No compile on the show path.** The gig bank is precompiled at startup
(bounded bank size); compile time is recorded per shader by the benchmark so
the startup budget can be planned. Implemented after qualification.

R6. **Output mode switch (owner request, 2026-10-07).** The appliance gets two
output modes: true 1080p and true 720p, where the HDMI mode itself is
1280x720@60 and nothing is upscaled by the compositor. A shader's
`render_height` is then capped by the output height. The 720p and 1080p
benchmark tiers map directly onto the two modes. Open design points: whether
the switch is a Stream Deck SYS key that restarts the renderer or a boot-time
setting, and how 1080p clips are presented in 720p mode (decoder downscale).
Scheduled after WP-A is reviewed.

R7. **Channel textures are in scope now**, not Phase 3: the owner asked for the
missing Shadertoy textures to be fetched (WP-G) so texture shaders render as
designed. The engine binding work (WP-H) follows WP-A.

R8. **Third content pipeline: stems.** See `THIRD_PIPELINE_STEMS.md`.

## 3. Data contracts

### 3.1 Bench manifest (input, JSON)

```json
{"version": 1,
 "out": "/var/lib/rpi-vj-bench/results.jsonl",
 "thumb_dir": "/var/lib/rpi-vj-bench/thumbs",
 "tiers": [[1280, 720], [1920, 1080]],
 "entries": [{"id": "33cGDj", "variant": "base", "path": "/srv/rpi-vj-bench/shaders/base/33cGDj.fs"}]}
```

`(id, variant)` is the unique key. `tiers` are measured in the listed order.

### 3.2 Bench results (output, JSONL, append-only, flushed per line)

```json
{"event":"begin","id":"33cGDj","variant":"base"}
{"event":"result","id":"33cGDj","variant":"base","status":"ok","compile_ms":412.0,
 "tiers":{"720":{"frames":90,"mean_ms":4.1,"p50_ms":4.0,"p95_ms":4.6,"max_ms":5.2,"first_ms":9.8},
          "1080":{"frames":90,"mean_ms":9.0,"p50_ms":8.9,"p95_ms":9.9,"max_ms":11.0,"first_ms":12.1}},
 "pixels":{"mean_luma":0.31,"spatial_std":0.22,"temporal_diff":0.08},
 "thumb":"33cGDj.base.ppm","temp_c":61.2,"throttled":"0x0"}
```

`status` is one of `ok`, `prepare_error`, `compile_error`, `too_slow`,
`gl_error`, `crash`, `hang`. Error statuses carry `error` (<= 400 chars).
`crash` and `hang` are written by the supervisor, never by the engine. A tier
that was not attempted is `{"skipped":"reason"}`. Tier keys are the height as a
string.

### 3.3 Measurement rules

- Offscreen target at the tier size, swap interval irrelevant; each frame is
  `draw` + `glFinish`, timed with a monotonic clock. Present to the screen at
  most every 10th frame so the operator can see progress.
- Time is synthetic and deterministic: frames sample `u_time` across a 0-40 s
  span so camera/scene phases are covered. Audio uniforms follow a fixed
  128 BPM synthetic pattern so reactive branches are exercised.
- Per tier: discard the first frame (reported as `first_ms`), then up to 90
  frames or 2.5 s, whichever comes first, minimum 3 frames.
- 720p is measured first. If its first frame exceeds 1,500 ms the shader is
  `too_slow` and 1080p is skipped. Otherwise 1080p is always measured.
- Pixel statistics come from a 96x54 downsample read back at three time
  points: mean luma, spatial standard deviation, mean absolute temporal
  difference. The middle sample is saved as a binary PPM thumbnail.
- The supervisor samples `vcgencmd measure_temp` / `get_throttled` per entry
  and pauses above 80 C until below 70 C.

## 4. Work packages

Rules for every agent: the canonical checkout is `E:\__Projects\RPi-VJ`; git
needs `-c safe.directory=E:/__Projects/RPi-VJ`; do not commit, reset, clean or
check out; stay inside the files assigned to the package; never write the Pi
credentials into the repository; report measured facts and state plainly what
was not verified.

### Phase 1 (parallel)

**WP-A Engine: bench mode, render tiers, supervisor.** Files: `engine/src/**`,
`engine/tools/pi_bench.py`, `engine/deploy/**`, `engine/README.md`.
1. `--bench <manifest>` per sections 3.1-3.3, with `--resume` that skips keys
   already holding a `result` line.
2. `render_height` per shader (R1) in `show.rs` and `render.rs`, backward
   compatible with existing manifests; shader-only path gets a real late-frame
   counter.
3. `engine/tools/pi_bench.py`: runs on the Pi, refuses to run while the
   services are active unless `--stop-services`, launches the bench, detects
   hang (no output for 60 s) and crash, appends the supervisor result, resumes,
   logs temperature, optionally restarts services at the end.
4. Build on the Pi (`~/RPi-VJ-native`, `cargo build --release --features
   native`), install the bench binary under `/opt/rpi-vj-bench/` without
   replacing the live binary, and prove it on the 18 original shaders.
Acceptance: unit tests and clippy pass; a results file for the 18 originals
exists with both tiers; a deliberately broken shader yields `compile_error`;
an infinite-loop shader yields `hang` or `too_slow` and the run continues.

**WP-B Catalog and adaptation tooling.** Files: `engine/tools/shader_bank/**`,
outputs under `target/shader-bank/`.
1. `catalog.py`: scan `target/native-13-shaders`, `target/shadertoy-fractals`,
   and `engine/examples/shaders`; dedupe by id and content hash; join
   `target/shadertoy-offline/<id>/shader.json`; classify per R4; extract cost
   features (loop count and literal bounds, AA/quality defines, texture and
   buffer inputs, mouse/keyboard/audio inputs).
2. `adapt.py`: emit variants `base` (unchanged) and `lite` (quality defines
   lowered: `HW_PERFORMANCE 0`, `AA`/supersampling to 1; nothing that changes
   the look beyond anti-aliasing). Record every transform applied per shader.
3. `make_manifest.py`: build a section 3.1 manifest for a chosen class/variant
   set with Pi-side paths, plus the staging directory to upload.
4. `report.py`: join results JSONL with the catalog, apply R3, and write
   `docs/native/SHADER_BENCH_RESULTS.md`, `target/shader-bank/qualified.json`
   and a show manifest with `render_height` per shader.
Acceptance: unit tests on fixtures; catalog covers every bank file; report
runs against a synthetic results file.

**WP-C Original-18 study.** Output: `docs/native/SHADER_ADAPTATION_RULES.md`.
Read the 18 shaders in `engine/examples/shaders`, the retired pair in
`gig/shaders/_retired`, and any benchmark notes in `docs/`, `gig/`, `tasks/`.
Extract the concrete budget rules they obey (loop bounds, octave counts,
raymarch step counts, texture use, branch style, precision) and why the two
retired shaders were retired. Deliver numbered, mechanically checkable rules
that WP-B can turn into `lite`/`pi` transforms and that a human can apply.

### Phase 2 (sequential, after Phase 1)

**WP-D Baseline.** Run the bench on the 18 originals; they define the reference
cost envelope. Then run the full `selfcontained` class (`base`), then
`needs_texture` (`base`, flagged). Detached on the Pi; several hours.

**WP-E Adaptation pass.** For shaders that miss 1080p or 720p: assign the
lower tier (540/360) by measuring it; bench the `lite` variant where one
exists; apply WP-C rules to the near misses. Re-bench only changed entries.

**WP-F Results and bank.** Generate the results document, the qualified list
and a tiered show manifest; deploy it with `render_height` set; run the in-situ
vsync acceptance with a clip playing; update `pi5-qualified-shaders.md`.

### Added 2026-10-07

**WP-G Texture acquisition** (running). Establish how assets were fetched,
fetch every missing channel asset into `target/shader-bank/assets/` with a
manifest, document in `SHADER_ASSETS.md`.

**WP-H Engine channel textures** (after WP-A). Bind 2D textures, cubemaps and
volumes per the WP-G manifest with their sampler settings; bind the deck
texture as `u_video` for stem shaders; then re-bench `needs_texture`.

**WP-I Output mode switch** (after WP-A). Per R6.

WP-B result: catalog holds 1,191 shaders — 812 `selfcontained`, 214
`needs_texture`, 147 `multipass`, 18 originals. Only 23 have a `lite` variant,
so the render tier and a rules-based `pi` variant are the real levers.

### Phase 3 (follow-up, not started by this plan)

Startup precompile (R5); buffer graph
for `multipass`; versioned deploy scripts; acquisition repair (handoff items
1-2, 4, 6).
