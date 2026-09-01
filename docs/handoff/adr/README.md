# Architecture Decision Records

These decisions are accepted unless their stated validation gate fails.

## ADR-001 — Modular Rust monolith

**Status:** Accepted

**Decision:** Use one Rust process with separately owned audio, analyzer, render, decode, control, indexing, and Stream Deck threads.

**Rationale:** Lowest latency and operational complexity on a single-purpose Pi. Typed in-process contracts are easier to reason about than network services.

**Consequences:** A process crash affects all modules. Mitigate with state persistence, fallback assets, watchdogs, and systemd restart. Split the control plane only if measured fault-isolation needs justify it.

## ADR-002 — wgpu/Vulkan target with a measured GLES fallback

**Status:** Provisional; resolve in Implementation Phase 4

**Decision:** Prefer wgpu/Vulkan for the production compositor. Select GLES/EGL if the final Pi stack cannot import decoded frames efficiently or reliably through wgpu.

**Rationale:** wgpu offers safer resource management and maintainable render-graph code; GLES maps naturally to traditional ISF and established EGLImage/GBM paths.

**Consequences:** A small backend comparison spike is mandatory. Raw Vulkan is not authorized merely for theoretical performance; it requires evidence that both higher-level paths fail.

## ADR-003 — ISF 2 embedded compatibility profile

**Status:** Accepted

**Decision:** ISF 2 is the shader content/metadata format, restricted by the profile in `RESEARCH_FINDINGS.md`.

**Rationale:** It provides existing content, declared controls, image/audioFFT inputs, and multipass semantics without inventing a project-only authoring format.

**Consequences:** Some desktop shaders require conversion. All show assets are validated and precompiled before activation.

## ADR-004 — GStreamer behind a video-deck abstraction

**Status:** Accepted, subject to target decoder tests

**Decision:** Use GStreamer for the target demux/decode implementation and hide it behind `VideoDeck`. Keep the current mpv launcher as the transitional implementation.

**Rationale:** GStreamer provides Linux pipeline negotiation and potential V4L2/DMA-BUF integration while the abstraction prevents it from infecting control and compositor code.

**Consequences:** Rust binding and buffer-lifetime complexity must be explicitly tested. The backend may be replaced without changing domain commands.

## ADR-005 — MP4/HEVC performance ingest profile

**Status:** Accepted, subject to Gate A

**Decision:** Use MP4 containers with HEVC Main 8-bit 4:2:0 as the preferred show format; support H.264 as software-decode compatibility media.

**Rationale:** Pi 5 hardware accelerates HEVC but software-decodes H.264 at significant CPU cost for 1080p60.

**Consequences:** Media ingest/transcoding is required. GOP and loop settings must be selected from target decoder measurements.

## ADR-006 — Typed internal events; JSON only at boundaries

**Status:** Accepted

**Decision:** Use Rust enums and fixed-layout snapshots inside the process. Use versioned JSON for configuration and low-rate admin APIs only.

**Rationale:** Avoid serialization, allocation, and schema ambiguity on real-time paths while retaining human-readable external control.

**Consequences:** A future process split needs protobuf/shared-memory contracts rather than blindly streaming current JSON at audio rates.

## ADR-007 — Two-deck prepared switching

**Status:** Accepted

**Decision:** Keep one visible and one hidden/prerolled deck. Activate only after a frame-ready event.

**Rationale:** Provides deterministic switching without decoding the entire library continuously.

**Consequences:** An unprepared selection may take longer and must leave the current source visible. Exact loop behavior may require head/tail buffering or second-context preroll.

