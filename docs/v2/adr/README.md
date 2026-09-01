# Architecture Decision Records

ADR-001 through ADR-007 in `docs/handoff/adr/` remain valid where this set does not supersede them. Superseded ones are named below.

Each decision here is accepted unless its stated validation gate fails.

---

## ADR-100 — wgpu everywhere, GLES only as a measured Pi fallback

**Status:** Superseded in place by [`ADR-100-render-backend.md`](ADR-100-render-backend.md) (2026-08-27), which records the measured outcome of the Phase B1 spike. The entry below is the original pre-spike decision, kept for the record. Short version of the update: accepted for the Workstation and Portable tiers on measured evidence; still provisional on the Appliance tier, where nothing has been measured and the GLES escape hatch stays open.

**Decision:** One render core in Rust on wgpu — Vulkan on the Pi and the RTX 3090, Metal on the M2 Pro. ISF/GLSL compiles to SPIR-V once and naga translates to each backend. Raw GLES/EGL is retained as a Pi-only escape hatch, selected only if Gate B shows wgpu cannot import decoded frames without per-frame CPU copies.

**Rationale:** With macOS in scope, a GLES-only core means writing a second renderer. V3DV's Vulkan 1.3 conformance makes the single-API approach viable on the Pi. The earlier GLES recommendation was correct for a Pi-only appliance and is invalidated by scope change, not by error.

**Consequences:** The backend spike (Phase B1) is mandatory and is the highest-risk item in the program. If the escape hatch is taken, ISF assets need both SPIR-V and GLSL-ES compilation paths in the cache, and that cost is recorded here rather than hidden. Raw Vulkan is not authorised on theoretical performance grounds; it requires evidence that both higher-level paths fail a measured requirement.

## ADR-101 — Virtual control surface with named regions

**Status:** Accepted.

**Decision:** Three layers — transport, device profile (`SurfaceModel`), semantic mapping (regions and bindings). The engine exposes named regions such as `clip.grid` and `layer.2.opacity`; device profiles project physical topology onto them. No crate below `vj-app` names a specific controller.

**Rationale:** Six controllers with four input paradigms and five feedback models. Integrating each directly produces six copies of the paging, banking, takeover, and feedback-diffing logic, and a seventh the day another controller is bought.

**Consequences:** One indirection layer between a button and a command, which costs nothing at these rates. Devices whose LED protocol is SysEx bulk still need a small Rust driver; everything else is a TOML profile. Gate H's "add a seventh controller with one TOML file" is the acceptance test for this decision.

## ADR-102 — Capability tiers with declared degradation

**Status:** Accepted.

**Decision:** Three tiers — `Appliance` (Pi 5), `Portable` (M2 Pro), `Workstation` (RTX 3090). A show declares its requirements and an explicit degradation block per lower tier. The engine never infers what to drop.

**Rationale:** One show file must work across a 15 W SoC and a 350 W GPU. Silent automatic degradation would drop the wrong layer at a gig.

**Consequences:** Authoring has to think about the Pi. A show that omits a degradation block for a tier simply does not run there, which is the correct failure. `vj-ingest` must produce per-tier clip renditions.

## ADR-103 — Tempo arbiter with an explicit authority policy

**Status:** Accepted.

**Decision:** One component owns tempo and phase. Sources report; a policy selects. Default `Priority([Manual, Link, MidiClock, Audio])`, with a confidence floor per source and a smooth ramp on handover.

**Rationale:** Audio-derived tempo is an estimate and must never silently outrank an authoritative source. rekordbox and Serato both speak Link, which carries phase as well as tempo.

**Consequences:** Every tempo consumer reads one state, so beat-locked shaders and quantized launches cannot disagree. Link support in your rekordbox version is a Gate J item, not an assumption. Pro DJ Link fits the same interface if you later move to CDJs.

## ADR-104 — Media tiers: HEVC on the Pi, HAP on the desktop

**Status:** Accepted, subject to Gate A. Extends ADR-005.

**Decision:** Appliance tier standardises on MP4/HEVC Main 8-bit 4:2:0. Desktop tiers prefer HAP/HAP Q. `vj-ingest` produces both from one master and the clip index records which renditions exist.

**Rationale:** The Pi has one codec block and it only does HEVC. HAP carries BC1/BC3 blocks and V3D has no S3TC/BC texture support — only ETC2 and ASTC — so HAP on the Pi would require a per-frame CPU transcode, which is worse than the hardware HEVC path.

**Consequences:** Every show clip exists in at least two renditions. A missing rendition is a load-time failure with a clear diagnostic, not a runtime surprise. Main10 stays out until the whole decode → import → display path is measured.

## ADR-105 — Two parallel tracks over shared crates

**Status:** Accepted.

**Decision:** Track A hardens the existing Pi repository for gigs; Track B builds the portable engine in a new workspace. `vj-types`, `vj-core`, `vj-surface`, `vj-surface-*`, and `vj-devices` are written once and consumed by both. Track A's control-surface phase *is* Track B's.

**Rationale:** You need your controllers working at gigs before a rewrite can plausibly finish. Sharing the control-surface and command-plane crates means the largest new investment is spent once.

**Consequences:** Track A carries mpv, which the portable engine will never use — accepted, because Track A's video path is small and isolated behind `VideoDeck`. Track A takes no new features after Phase A4 and is retired at B7.

## ADR-106 — `VideoDeck` with per-platform backends; mpv is Track A only

**Status:** Accepted. Supersedes ADR-004's naming of GStreamer as *the* implementation.

**Decision:** `VideoDeck` is the seam. Backends: GStreamer V4L2 on the Pi, ffmpeg+NVDEC (or D3D11VA) on Windows, VideoToolbox on macOS. mpv subprocess remains only in Track A.

**Rationale:** The abstraction was right; naming one implementation was premature for a codebase that needs three. The subprocess isolation argument holds for mpv specifically and does not generalise — the moment the compositor needs decoded textures rather than final scanout, subprocess isolation is exactly what you cannot have.

**Consequences:** Three decode implementations to maintain and three Gate A runs. The frame-import seam is the unsafe part of the codebase and gets disproportionate test attention.

## ADR-107 — Hot decoder slots, not page-cache warming

**Status:** Accepted. Supersedes the caching strategy in `RESEARCH.md` §1.3.

**Decision:** A bounded pool of live decoder slots holding presentable first frames, sized per tier (4 on Appliance, 12 on Portable, 32 on Workstation), populated by explicit cueing plus recent-use eviction. Page-cache warming is retained as a cheap tier-1 optimisation, not as the latency strategy.

**Rationale:** Only a live decoder holding a presentable frame meets a hard sub-50 ms budget. Page-cache warm removes storage latency; demux-warm removes container probing; neither removes decoder startup.

**Consequences:** Memory cost per hot slot (~3 MiB for a 1080p NV12 frame before alignment) bounds the pool. A cold trigger keeps the current source visible and reports degraded latency rather than blanking — this is a behaviour contract, not a best effort.

## ADR-108 — `elgato-streamdeck` crate over hand-rolled HID

**Status:** Accepted. Supersedes the raw-hidapi endorsement in `docs/RESEARCH_FINDINGS.md`.

**Decision:** Use the `elgato-streamdeck` crate. Lift the current implementation's reconnect and cached-icon-replay behaviour into `vj-surface`'s generic feedback cache, where every device inherits it.

**Rationale:** "A fixed appliance benefits from a visible protocol" holds only while the target is one fixed appliance. Across three OSes and two XL revisions, per-model report framing, image format, key rotation, and page sizing are maintenance not worth owning. The crate is hidapi-backed, so the transport story is unchanged.

**Consequences:** One more dependency, and its API shape constrains the HID adapter. The press-edge diffing bug in the current code is fixed by `vj-surface-hid::diff_buttons` regardless of which library sits underneath.
