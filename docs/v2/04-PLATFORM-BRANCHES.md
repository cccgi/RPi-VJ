# Platform Branches

Branching is authorised only where the hardware genuinely differs: video decode, frame import, presentation, audio capture, and MIDI/HID transport. Everything else — reducer, virtual surface, ISF host, compositor graph, analyzer, show format — is one implementation.

## 1. The matrix

| | **Appliance** — RPi 5 | **Portable** — MacBook Pro M2 Pro | **Workstation** — i-12th gen + RTX 3090 |
|---|---|---|---|
| GPU | VideoCore VII / V3D 7.1 | Apple M2 Pro (16–19 core) | GA102, 24 GB GDDR6X |
| wgpu backend | Vulkan 1.3 (V3DV) → GLES 3.1 escape hatch | Metal | Vulkan (preferred) or D3D12 |
| Shader target | SPIR-V | MSL via naga | SPIR-V / DXIL |
| Hardware decode | **HEVC only** — no H.264 block | VideoToolbox: HEVC, H.264, ProRes | NVDEC: HEVC, H.264, AV1, VP9 |
| Preferred clip format | MP4 / HEVC Main 8-bit 4:2:0 | HAP / HAP Q, or HEVC | HAP / HAP Q, or anything |
| Decode backend | GStreamer `v4l2slh265dec` | VideoToolbox | ffmpeg + NVDEC (or D3D11VA) |
| Frame import | DMA-BUF → `VK_EXT_external_memory_dma_buf` | `CVPixelBuffer`/IOSurface → `MTLTexture` | CUDA→Vulkan external memory, or D3D11 shared handle |
| Texture compression | ETC2 + native ASTC. **No S3TC/BC** | BC + ASTC + ETC2 | BC (full) |
| Presentation | DRM/KMS atomic, no compositor | `CAMetalLayer`, windowed or fullscreen | Swapchain, windowed/fullscreen/exclusive |
| Audio capture | PipeWire, quantum 256 @ 48 kHz | CoreAudio via `cpal` | WASAPI (shared or exclusive) via `cpal`; ASIO optional |
| MIDI | ALSA sequencer (`midir`) — multi-client | CoreMIDI — multi-client | WinMM — **exclusive per port** |
| HID | hidraw + udev rule | IOKit HID | HID API |
| Target load | 2 decks + 2 shader layers @ 1080p60 | 4 decks + 6 layers @ 1440p–4K60 | 8 decks + 8+ layers @ 4K60 |
| Realistic role | Gig appliance, boots into the show | Portable gig rig | Development, authoring, big-show rig |

## 2. Raspberry Pi 5 — Appliance tier

The binding constraints, all carried forward from the earlier research and all still true:

- **One codec block, HEVC only.** BCM2712 dropped the H.264 decoder that BCM2711 had. H.264 runs on the A76 cores at roughly 50–60% CPU for 1080p60 — usable for one stream, not for two decks plus DSP plus shaders. Standardise the show library on HEVC Main 8-bit 4:2:0. Main10 only after the whole decode → import → display path is measured.
- **No S3TC/BC texture support in V3D.** Mesa's V3D driver does ETC1/ETC2 and, more recently, native ASTC. HAP's BC1/BC3 payload would need a CPU transcode per frame, which is worse than the hardware HEVC path. This is why HAP is a desktop-tier format.
- **GLES 3.1 conformant, not 3.2.** V3DV is Vulkan 1.3 conformant (Mesa 24.3+), which is what makes the wgpu decision viable here at all — but it is a younger driver with thinner tooling than the GLES path. Gate B decides whether it holds under sustained 60 fps compositing with a live video import.
- **DRM/KMS exclusivity.** Boot to multi-user target, no desktop session, `dtoverlay=vc4-kms-v3d`, increased CMA. A running compositor holds DRM master and will fight you for it.
- **Discover, do not assume.** DRM card/connector numbering and PipeWire node IDs are not stable; resolve them at startup. The HEVC decoder and ISP claim adjacent DRM-namespace nodes, so `/dev/dri/card0` is not reliably the HDMI-owning node.
- **Power and thermals.** A quality 5 A supply and active cooling are not optional; throttling produces exactly the nondeterministic frame timing that makes a rig feel broken.

### The escape hatch

If Gate B shows that wgpu cannot import V4L2 DMA-BUF frames without a per-frame CPU copy, the Pi compositor falls back to raw GLES/EGL with `EGLImage` import — the path `audiovis` and the wider embedded-GL world have proven. That fallback is confined to `vj-render`'s backend module and does not leak into the compositor graph, ISF host, or anything above. It would be recorded honestly in ADR-100 rather than papered over.

The escape hatch has a cost worth naming up front: an ISF shader would then need both a SPIR-V and a GLSL-ES compilation path in the cache. That is the price of the fallback and the reason it is a fallback.

## 3. Windows / RTX 3090 — Workstation tier

The development platform, and the one with essentially no constraints. NVDEC handles every codec you would plausibly use, and 24 GB of VRAM removes texture budget as a design concern.

Three things that still need care:

- **CUDA↔Vulkan interop.** NVDEC output is a CUDA array; getting it into a wgpu texture without a round trip through system memory requires external memory objects. This works and is well-trodden, but it is unsafe interop code and belongs behind a narrow, tested seam in `vj-video`. The simpler D3D11VA → shared handle path is the fallback if CUDA interop proves fragile with wgpu's abstraction.
- **WinMM port exclusivity.** Only one process may open a MIDI input port. If Resolume has your APC open, the engine cannot. This is the single most likely source of "it works on my Mac" confusion. Prefer the Windows MIDI Services / WinRT path where available; otherwise partition devices between applications explicitly.
- **Audio capture.** WASAPI shared mode adds buffering; exclusive mode gets you close to the Pi's latency but takes the device away from everything else. ASIO through the FLX10's driver is the professional path if you want the DJ software and the visualiser sharing one interface.

Because this tier has headroom, it is where the **capability ceiling** gets defined: shows are authored here, and the ingest tool generates the Appliance renditions from the same masters.

## 4. macOS / M2 Pro — Portable tier

Metal-only, and that is fine — naga generates MSL from the same SPIR-V the other tiers use.

- **VideoToolbox** decodes to `CVPixelBuffer` backed by an `IOSurface`, which converts to an `MTLTexture` with no copy. This is the cleanest video import path of the three platforms.
- **CoreMIDI is multi-client**, so the engine and Resolume can hold the same controller simultaneously. Useful during the transition period while you still run Resolume for real shows.
- **Syphon** is the macOS equivalent of Spout and is the obvious later addition for feeding the engine's output into Resolume, or vice versa, during migration. Not in the first release; the architecture should not preclude it.
- Unified memory means the CPU-side decode path for HAP is unusually cheap here — HAP Q at 1080p is comfortable and HAP at 4K is plausible.

## 5. Capability tiers and show portability

A show file declares what it needs; the engine declares what it has; the loader reconciles them.

```toml
# shows/warehouse-set.toml
schema_version = 1
name = "Warehouse Set"
authored_tier = "workstation"

[requirements]
min_tier      = "appliance"
video_decks   = 4              # desired
shader_layers = 6              # desired
render_scale  = 1.0

[degradation.appliance]
video_decks   = 2
shader_layers = 2
render_scale  = 0.75           # internal render at 1440x810, upscale to 1080p
shader_quality = "low"          # ISF LOD selector
disable_layers = ["fx.bloom", "fx.feedback"]
```

Rules:

- Degradation is **declared, not inferred.** The engine will not silently guess which of your six layers to drop.
- If a show declares `min_tier` above the running tier, it refuses to load with a diagnostic naming the shortfall. It does not half-load.
- Clip renditions are per tier. A show referencing a HAP master with no HEVC rendition fails Appliance load with a list of the clips that need transcoding — and `vj-ingest --tier appliance` fixes it in one command.
- `shader_quality` maps to an ISF LOD convention: shaders may declare `LOW`/`MED`/`HIGH` variants via a preprocessor define, and the compiler caches the tier-appropriate one.

This is what makes "author on the 3090, perform on the Pi" a real workflow rather than an aspiration.

## 6. Build and CI

| Target | Toolchain | How |
|---|---|---|
| `aarch64-unknown-linux-gnu` | Native on Pi (primary), or `cross` with a custom image carrying the aarch64 sysroot for PipeWire/GStreamer/hidapi/ALSA | Native build is the supported path; a cross image is a CI convenience, and the earlier research is right that it needs a purpose-built Docker image that does not exist yet |
| `x86_64-pc-windows-msvc` | Native | Requires the CUDA toolkit only if the NVDEC backend feature is enabled |
| `aarch64-apple-darwin` | Native | VideoToolbox and CoreMIDI via system frameworks |

Cargo features gate the platform backends so a Pi build never compiles CUDA bindings:

```
default          = ["surface-hid", "surface-midi"]
backend-v4l2     # Pi decode
backend-nvdec    # Windows decode
backend-vtb      # macOS decode
backend-gles     # Pi render escape hatch
tempo-link       # Ableton Link
```

CI must build all three targets on every change to a shared crate. A change that compiles on Windows and breaks the Pi is the failure mode this whole structure exists to prevent, and it is cheap to catch.

## Sources

- [Raspberry Pi processor documentation](https://www.raspberrypi.com/documentation/computers/processors.html)
- [V3D — Mesa 3D documentation](https://docs.mesa3d.org/drivers/v3d.html)
- [Raspberry Pi V3D driver adds native ASTC texture compression — Phoronix](https://www.phoronix.com/news/V3D-Native-ASTC-Compression)
- [Mesa 24.3 adds Vulkan 1.3 conformance for V3DV](https://www.linuxtoday.com/blog/mesa-24-3-open-source-graphics-stack-adds-vulkan-1-3-conformance-for-v3dv/)
- [Raspberry Pi driver updated with Vulkan 1.3 support — Vulkan.org](https://www.vulkan.org/news/auto-22842-c5c77c2da002ef993800251296a06bf3)
- [GStreamer V4L2 codecs documentation](https://gstreamer.freedesktop.org/documentation/v4l2codecs/)
- [GStreamer DMA-BUF design](https://gstreamer.freedesktop.org/documentation/additional/design/dmabuf.html)
