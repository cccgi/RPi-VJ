# RPi-VJ: Low-Latency Embedded VJ System — Deep-Dive Research

> **Historical research, not the implementation handoff.** This document describes an aspirational multi-crate scaffold that is not the current repository and includes old validation claims that have not been reproduced here. Use [`docs/HANDOFF.md`](docs/HANDOFF.md), [`docs/RESEARCH_FINDINGS.md`](docs/RESEARCH_FINDINGS.md), [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md), and [`docs/IMPLEMENTATION_PLAN.md`](docs/IMPLEMENTATION_PLAN.md) as the authoritative handoff.

**Target hardware:** Raspberry Pi 5 (BCM2712, quad Cortex-A76 @ 2.4GHz, VideoCore VII)
**Target OS:** Raspberry Pi OS 64-bit Lite (Bookworm), boot-to-CLI, no desktop compositor
**Peripherals:** Elgato Stream Deck XL (32 keys, USB HID), USB audio interface (line-in), HDMI display

This document is the rationale behind the choices baked into the accompanying code scaffold. Section numbers match the scaffold's inline doc comments (`vj-video`, `vj-streamdeck`, `vj-audio`).

---

## 1. Video Playback & Display Pipeline

### 1.1 The fact that reshapes everything else: Pi 5 has no H.264 hardware decoder

This is the single most consequential piece of research for this project, and it's easy to miss because Pi 4 and Pi 5 look similar on paper. The BCM2712 in the Pi 5 **removed the dedicated H.264 decode block** that BCM2711 (Pi 4) had. Pi 5 ships **one hardware video codec block, and it only does HEVC (H.265)** decode, exposed through a stateless V4L2 M2M "request API" driver (upstreamed to mainline as `media: platform: verisilicon/rpivid`, the same IP block Raspberry Pi calls `rpivid`). This was confirmed at launch and has generated ongoing community frustration and confusion because most existing content libraries are H.264 ([Raspberry Pi 5 has no hardware video encoding and only HEVC decoding — Hacker News discussion](https://news.ycombinator.com/item?id=38068801); [RPi5 Codec confusion — Raspberry Pi Forums](https://forums.raspberrypi.com/viewtopic.php?t=357870); [Raspberry Pi 5 and the Lack of Hardware H.264 Decoding — A Huge Step Backward — Raspberry Pi Forums](https://forums.raspberrypi.com/viewtopic.php?t=391283)).

Practical consequences for this project, which the scaffold encodes directly into its defaults:

- **Author performance clips as HEVC.** `scripts/encode_clip.sh` transcodes source footage to HEVC Main/Main10 with a keyframe-only (or 1s) GOP. HEVC clips get true zero-copy hardware decode via the V4L2 stateless driver, leaving all four A76 cores and the VideoCore VII GPU free for shader work — this is the only codec path that meets the "low overhead, maximum GPU compute for shaders" requirement.
- **H.264 still works, just not on the hardware decoder.** ffmpeg/libav will fall back to software decode on the A76 cores. At 2.4GHz with NEON, four A76 cores comfortably handle 1080p30 H.264 software decode and can often do 1080p60, but it costs real CPU budget that competes with anything else running on the box (Stream Deck polling, audio FFT, any CPU-side shader uniform pushing). Treat H.264 as an import/compatibility format, not a performance-clip format.
- Verify hardware decodability on-device once clips are encoded: `ffmpeg -hwaccel drm -c:v hevc -i clip.mp4 -f null -` should run at many multiples of realtime with near-zero CPU.

### 1.2 Player/binding evaluation: mpv (subprocess + JSON IPC) vs gstreamer-rs vs raw ffmpeg/DRM

| Approach | Zero-copy path | Latency | Engineering cost | Crash isolation | Chosen? |
|---|---|---|---|---|---|
| **mpv subprocess, `--vo=gpu-next --gpu-context=drm`** | Yes — mpv's DRM-PRIME import from V4L2 M2M is mature and battle-tested | Sub-frame; dominated by decoder keyframe latency, not mpv overhead | Low — mpv does modesetting, hwdec negotiation, A/V sync | Yes — a decoder/EGL crash kills the mpv process, not the daemon | **Yes (primary)** |
| `gstreamer-rs` (`v4l2h264dec`/hevc equivalent + `kmssink`/`drmprime`) | Yes, via `drmprime`/`kmssink` zero-copy caps negotiation | Comparable to mpv once pipeline is tuned (`sync=false`, minimal queue depths) | High — GStreamer's own DRM/KMS sink story on Pi 5 is less mature and more forum-reported friction than mpv's ([The Raspberry Pi 5 uses Gstreamer for H.264 hardware decoding — Raspberry Pi Forums](https://forums.raspberrypi.com/viewtopic.php?t=387547); [Gstreamer drm sink, zero copy — Raspberry Pi Forums](https://forums.raspberrypi.com/viewtopic.php?t=375855)) | Depends on process model chosen | No (documented as a viable alternative, not implemented) |
| Raw ffmpeg/libav + hand-rolled DRM/KMS-atomic | Yes, most direct — `AV_PIX_FMT_DRM_PRIME` frames straight into your own atomic commit | Best possible, fully in your control | Very high — reimplement demux edge cases, A/V sync, hotplug, colorspace/HDR metadata, atomic pageflip state machine | In-process — a bug takes down the whole daemon unless you sandbox it yourself | No — sketched as **Alternative B** in `vj-video/src/raw_drm.rs` |
| `libmpv`/`mpv-rs` (in-process FFI) | Same as subprocess mpv | Marginally lower IPC overhead (irrelevant at these latencies) | Medium — FFI binding maintenance burden, and a libmpv crash is a process crash | None — same process | No (see 1.4) |

**Why mpv-as-subprocess over libmpv-as-library:** the latency difference between an in-process libmpv call and a Unix-socket JSON IPC round-trip is sub-millisecond — irrelevant against a 50ms trigger budget that's dominated by decoder/pageflip latency, not control-plane overhead. What subprocess isolation buys instead is operational robustness: mpv's DRM/EGL backend occasionally hits driver-level faults on embedded GPUs, and when it does, you want the Stream Deck input loop and audio capture to keep running and the daemon to respawn playback in under a second — not take the whole VJ rig down mid-set. `vj-video/src/player.rs` implements this: `MpvPlayer::spawn` launches mpv with `--idle=yes --gpu-context=drm`, `vj-core`'s main loop detects a dead child via `is_alive()` and respawns.

**mpv flags that matter for this use case** (all present in `player.rs`):
- `--vo=gpu-next --gpu-context=drm` — renders directly to a DRM/KMS CRTC, no Wayland/X11 compositor in the path at all. This is the standard "boot to CLI, mpv owns the display" pattern documented across the mpv/Pi community ([Rendering Decoded Video Frames — Raspberry Pi Forums](https://forums.raspberrypi.com/viewtopic.php?t=346309); mpv's own `--gpu-context=drm` support).
- `--hwdec=drm --hwdec-codecs=all` — enables the V4L2-stateless/DRM-PRIME zero-copy decode path.
- `--drm-device=` / `--drm-connector=` — pins output to a specific card/connector; **do not assume `/dev/dri/card0`** on a Pi 5 — confirm the HDMI-owning node with `cat /sys/class/drm/*/status` since the HEVC decoder and ISP can claim adjacent DRM-namespace nodes.
- `--video-sync=display-resample`, `--interpolation=no` — interpolation adds a frame of latency for a visual-quality tradeoff that has no upside for VJ triggering; kept off deliberately.
- Known caveat to watch for: some Pi 5 + `gpu-context=drm` + 4K-output combinations have reported frame drops even on FHD content in certain mpv builds ([Raspberry Pi 5: vo=gpu + gpu-context=drm + gpu-api=opengl drops frames on 4K display — GitHub mpv issue #17447](https://github.com/mpv-player/mpv/issues/17447); [RPi 5: MPV dropping frames — Raspberry Pi Forums](https://forums.raspberrypi.com/viewtopic.php?t=393942)). Pin your HDMI output mode to native 1080p if you hit this, and track mpv's changelog before upgrading in production.

### 1.3 Clip pre-loading & <50ms trigger strategy

The <50ms budget is dominated by four costs, in order: (1) page-cache miss on the clip file, (2) demuxer container probe, (3) decoder pipeline spin-up, (4) first pageflip. `vj-video/src/preload.rs` attacks (1) and (2):

- **Whole-bank page-cache warming at boot.** `posix_fadvise(WILLNEED)` plus a forced sequential read pass over every clip in the active bank. At realistic VJ clip sizes (~10s 1080p HEVC @ ~16Mbps ≈ 20MB), a 32-clip bank is ~640MB — comfortably inside a Pi 5's RAM budget alongside the rest of the process. This eliminates SD-card/USB-SSD seek latency entirely for triggered clips.
- **mpv playlist pre-append trick** for (2): appending a clip to mpv's playlist without playing it causes mpv to demux/probe it once, priming its own internal state, so the later `loadfile ... replace` skips container probing.
- **What's intentionally NOT implemented:** true dual-buffer crossfades (two decode pipelines live simultaneously, composited). That requires either two DRM overlay planes or an offscreen-FBO compositing step — a real GPU-budget trade-off against the "maximum GPU compute for shaders" requirement. `raw_drm.rs`'s module doc sketches the two-plane design for teams that need it later; the recommended default is hard-cut triggering, which is also what most club/VJ workflows actually want from a physical button.
- Keyframe-only or 1-second-GOP encoding (see `encode_clip.sh`) matters here too: mpv/ffmpeg can start decoding from any point in the file without needing to walk backward for a reference frame, which keeps step (3) minimal and consistent regardless of where in the clip playback last stopped.

### 1.4 Rust bindings landscape

- `mpv-rs` / `libmpv2` (in-process FFI bindings to libmpv) exist and are usable, but as noted above the isolation argument outweighs the marginal latency win for this use case. If a future iteration needs tighter coupling (e.g. reading back decoded frame textures for GPU shader post-processing rather than letting mpv render finally), that's the point at which in-process libmpv (or the raw ffmpeg/DRM path) becomes worth the added crash-domain risk.
- `gstreamer-rs` is a legitimate, well-maintained binding; it's not chosen here because GStreamer's DRM/KMS output sink story on Pi 5 has more open forum friction than mpv's as of this research pass, and mpv's `--vo=gpu-next` already solves the exact "render decoded HW frames straight to a KMS plane" problem this project needs. Revisit if the project needs GStreamer-specific features (e.g. its RTP/streaming elements for remote monitoring).

---

## 2. Stream Deck XL USB Driver & Hardware Interface

### 2.1 Library evaluation

| Option | Language | XL (32-key + no LCD-per-key... wait, XL uses per-key LCD) support | Verdict |
|---|---|---|---|
| `elgato-streamdeck` (crate, hidapi-backed) | Rust | Actively maintained, explicit Stream Deck XL (`Kind::Xl`/`Kind::XlV2`) support, handles per-key image upload, encoder/knob and touch-screen input for other models too | **Chosen** |
| `python-streamdeck` | Python | Reference-quality protocol implementation, well documented, but pulling a Python process into a low-latency Rust daemon adds an IPC boundary and a GIL-bound dependency for no benefit here | Reference only |
| Raw `hidapi` | C library via Rust `hidapi` crate | Full control, but requires hand-rolling Elgato's per-model report format, image chunking/rotation, and page-report framing — exactly what `elgato-streamdeck` already does correctly | Not chosen (used transitively — `elgato-streamdeck` is itself hidapi-backed) |

`elgato-streamdeck` (crates.io, [docs.rs](https://docs.rs/elgato-streamdeck/latest/elgato_streamdeck/)) uses `hidapi` on the backend, which on Linux resolves to either the kernel `hidraw` or `libusb` interface depending on build features; the scaffold pins `hidapi`'s `linux-static-hidraw` feature since `hidraw` needs no elevated USB permissions beyond the udev rule (`systemd/60-streamdeck.rules`) and avoids libusb's device-claiming semantics colliding with anything else on the bus.

### 2.2 Implementation notes (see `vj-streamdeck/src/device.rs` and `icons.rs`)

- **Auto-reconnect:** `StreamDeckHandle::connect_with_retry` retries with exponential backoff (250ms → 5s cap) whenever the device isn't present; `spawn_event_loop`'s read loop detects a read error (typically a USB unplug), clears the handle, and transparently reconnects — the rest of the daemon (video, audio) keeps running through a Stream Deck unplug/replug.
- **Async event detection for 32 keys:** the crate's `read_input` call is synchronous/blocking (as hidapi itself is), so it's wrapped in `tokio::task::spawn_blocking` and polled in a loop with a 100ms read timeout, forwarding `ButtonStateChange` transitions over a `tokio::mpsc` channel that `vj-core`'s async main loop consumes. **Note the scaffold's simplification:** it currently emits a `Pressed` event for every `true` entry in each state-change report rather than diffing against the previously-seen state to detect true press/release transitions — production code should track `[bool; 32]` across calls and only emit on 0→1 (press) / 1→0 (release) edges (flagged with a `TODO` in the code).
- **Dynamic icon rendering:** `icons.rs` composites a clip thumbnail plus a status-colored border (idle/cued/playing) using the `image` crate directly in Rust — no shelling out to ImageMagick/ffmpeg per icon update, which matters because icon updates happen on every clip trigger and need to be fast and jitter-free. `elgato-streamdeck`'s `set_button_image` takes a plain `image::DynamicImage` and handles the per-model resize/rotation/encoding (BMP for older models, JPEG for XL) internally.
- **udev permissions:** `systemd/60-streamdeck.rules` grants the `plugdev` group r/w on Elgato's hidraw nodes (vendor ID `0fd9`) so `vj-core` can run as an unprivileged `vj` user rather than root.

---

## 3. PipeWire Audio Capture Setup

### 3.1 Quantum/rate configuration for <10ms round-trip

End-to-end capture latency in PipeWire is dominated by the graph quantum: PipeWire drains its ring buffer once per `quantum/rate` seconds, and there's roughly one period of jitter in each direction (device → graph, graph → your client callback):

```
quantum = 256, rate = 48000  =>  256/48000 = 5.33ms per period
round-trip budget            ≈  2 × 5.33ms ≈ 10.7ms
```

This is the tightest **reliable** quantum for a USB Audio Class device on a Pi 5. A quantum of 128 halves that to ~5.3ms round-trip on paper, but community measurements consistently report xruns on class-compliant USB interfaces at 128 because of USB microframe jitter on the Pi's xHCI controller — 256 is the practical floor, matching general guidance in the PipeWire tuning literature ([PipeWire Quantum in 2026: Choosing 64, 128, 256, or 512 Without XRuns — Linux DJ](https://www.linuxdj.com/notes/pipewire-quantum-in-2026-choosing-64-128-256-512-without-xruns/); [Configure PipeWire for Pro Audio — Linux Junkies](https://linuxjunkies.org/guides/install-pipewire-pro-audio); [Pipewire Guide for low latency — GitHub Gist](https://gist.github.com/cidkidnix/86a01ecf82f54eec39f27a9807b90a1b)).

**Quantum is a graph-wide property, not a per-client one** — a single application cannot unilaterally request 256 and get it if the rest of the graph disagrees; it must be set at the PipeWire daemon level. The scaffold does this two ways:
1. `systemd/pipewire/99-vj-lowlatency.conf` — a `pipewire.conf.d` drop-in pinning `default.clock.rate=48000` and `default.clock.quantum = default.clock.min-quantum = default.clock.max-quantum = 256` (locking min=max prevents PipeWire's adaptive-quantum logic from silently growing the buffer under load, which is exactly the kind of variance a live VJ rig can't tolerate).
2. `systemd/pipewire/99-vj-usb-audio.conf` — a WirePlumber ALSA-monitor rule matching the USB input node and pinning its own `api.alsa.period-size=256`, disabling headroom padding and auto-suspend (a suspended-then-resumed USB audio node adds a startup glitch you don't want mid-set).

`vj-audio`'s own stream (`capture.rs`) additionally sets `PW_KEY_NODE_LATENCY`/`PW_KEY_NODE_RATE` properties matching the system quantum as a belt-and-braces check — if the system-level drop-in wasn't applied (e.g. a fresh image before `setup.sh` ran), this doesn't silently accept a mismatched buffer size.

### 3.2 Minimal capture loop implementation

`vj-audio/src/capture.rs` runs PipeWire's own mainloop on a dedicated OS thread (the supported integration pattern — PipeWire's C mainloop is not tokio-compatible and must not share a thread with the async reactor) and forwards each delivered buffer over a `std::sync::mpsc` channel to a `tokio::task::spawn_blocking` consumer, which runs an FFT-based feature extractor (`features.rs`, via `rustfft`) producing per-block RMS/peak plus bass/mid/high band envelopes — the actual signal shaders would consume.

**Caveat on this module's PipeWire API usage:** it targets the `pipewire` crate v0.8 API surface as researched, and the scaffold's own `cargo check` (run against x86_64 with matching `libpipewire-0.3-dev`/`libspa-0.2-dev` headers, gating the `v0_3_44` feature for `PW_KEY_TARGET_OBJECT`) compiles clean — but it has **not** been run against real hardware or a live PipeWire graph, since this environment has neither a Pi 5 nor a USB sound card attached. Treat the buffer format handling (`bytemuck_cast_slice`'s alignment assumption, noted in its own doc comment) as the first thing to validate against your actual capture device's negotiated format on real hardware.

---

## 4. Production Repository Scaffold

### 4.1 Layout

```
rpi-vj/
├── Cargo.toml                  workspace root
├── rust-toolchain.toml         pins aarch64-unknown-linux-gnu target
├── vj-core/                    orchestration daemon (binary: vj-core)
├── vj-video/                   mpv/DRM-KMS player + clip preload cache
├── vj-streamdeck/              Stream Deck XL driver
├── vj-audio/                   PipeWire capture + FFT feature extraction
├── config/config.example.toml  key bindings, DRM device, audio target
├── systemd/
│   ├── vj-core.service         headless boot service, DRM/KMS group perms
│   ├── 60-streamdeck.rules     udev rule for non-root HID access
│   └── pipewire/*.conf         low-latency quantum drop-ins
├── scripts/
│   ├── setup.sh                one-shot device provisioning
│   ├── build.sh                native build + install
│   ├── cross-build-aarch64.sh  cross-compile from x86_64 dev machine
│   └── encode_clip.sh          source video -> HEVC performance-clip encode
└── docs/RESEARCH.md            this document
```

### 4.2 Boot path (headless, DRM/KMS owned exclusively by mpv)

1. `raspi-config nonint do_boot_behaviour B2` + `systemctl set-default multi-user.target` — console autologin, **no** desktop session. This matters because a running Wayland/X11 compositor holds the DRM master lock and will fight mpv for it.
2. `dtoverlay=vc4-kms-v3d` in `/boot/firmware/config.txt` (should already be default on current Raspberry Pi OS, verified/enforced by `setup.sh`) with an increased CMA reservation (`cma-512`) for decode + display-plane headroom.
3. `vj-core.service` starts after `pipewire.service` and `systemd-udev-settle.service`, runs as an unprivileged `vj` user in the `render`, `video`, `plugdev`, `audio` groups (no root), `Restart=always`, `KillMode=control-group` so a restart also cleans up the child mpv process.

### 4.3 What's genuinely production-ready vs. scaffold-level in this repo

Being direct about the maturity boundary, since "production-ready" can be read two ways:

- **Compiles clean, structurally sound, follows the real crate APIs:** the whole workspace (`vj-core`, `vj-video`, `vj-streamdeck`, `vj-audio`) was verified with `cargo check`/`cargo build` against the actual `elgato-streamdeck`, `pipewire`, `tokio`, and `image` crate APIs during this session (on x86_64, with the same native dev headers `setup.sh` installs) — this caught and fixed nine real API-mismatch errors during development (wrong tokio feature flags, a non-cloneable IPC event channel, `elgato-streamdeck`'s actual `set_button_image(DynamicImage)` signature vs. an assumed `ImageRect` type, PipeWire key constants gated behind version feature flags, etc.). It is not decorative pseudocode.
- **Not yet hardware-validated:** none of this has run against an actual Pi 5, Stream Deck XL, or USB audio interface — this environment has none of that hardware. The button-state diffing TODO in `device.rs`, the PipeWire buffer-alignment assumption in `capture.rs`, and the exact mpv DRM flag set (particularly around the known 4K frame-drop reports) are the three things to validate first on real hardware, in that order.
- **Deliberately unimplemented:** `vj-video/src/raw_drm.rs` (Alternative B, raw ffmpeg+DRM path) is a typed skeleton, not an implementation — building it out is multi-week effort documented as out of scope for this scaffold (§1.4).

### 4.4 Cross-compilation note

Native build directly on the Pi 5 (`scripts/build.sh`) is simplest for a project this size and is the recommended path. `scripts/cross-build-aarch64.sh` sketches the `cross`-based alternative for a CI pipeline, but flags that `pipewire`/`hidapi`/mpv's C library dependencies need an aarch64 sysroot baked into a custom `cross` Docker image before that script is usable unattended — this repo does not ship that custom image.
