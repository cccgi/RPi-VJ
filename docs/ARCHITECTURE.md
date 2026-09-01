# Target Architecture

## Goals

- Headless boot to a deterministic HDMI output path.
- Physical button-to-visible-frame response with a measured p99 target.
- Stable 48 kHz/256-frame USB line-in capture without xruns.
- No work in real-time callbacks that can allocate, block, log, or access storage.
- Bounded memory use and graceful degradation when clips or devices fail.
- A future shader compositor can be added without rewriting device control or audio analysis.

## Component model

```text
                         control plane
 Stream Deck HID thread ───────────────┐
   reconnect / key edges / icons       │ Trigger{key,timestamp}
                                      v
                              State coordinator
                              clip bank / cues
                              health / lifecycle
                               │            │
                    PlayerCommand          IconState
                               │            └────────> HID thread
                               v
                         Video backend
                  decoder pool + presentation
                               │
                  DMABUF / EGLImage / DRM FB
                               v
                         DRM/KMS -> HDMI

 USB audio -> PipeWire RT callback -> bounded SPSC audio blocks
                                      │
                                      v
                              DSP worker (FFT/features)
                                      │ latest snapshot
                                      v
                              render/shader thread
```

## Process boundary

### Initial production path

- One Rust supervisor/control process.
- One child mpv process owning DRM/KMS.
- PipeWire capture should ultimately be in the Rust process through supported bindings or a small C FFI module. The current standalone C client is a validation utility, not final integration.

This keeps mpv/driver faults outside the supervisor while device state survives a player restart.

### Shader-compositor path

When custom shaders must consume decoded clip textures, choose one of:

1. Application-owned GBM/EGL context plus libmpv render API.
2. GStreamer decoder pool exporting DMABUFs into application-owned EGLImages.

Run a two-day hardware spike for both. Select based on measured copy count, frame pacing, decoder-pool behavior, API stability, and recovery—not theoretical call overhead.

## Thread and scheduling model

| Thread/process | Work | Scheduling guidance |
|---|---|---|
| Coordinator | State machine and channels | Normal priority; never blocks on device I/O |
| HID worker | Blocking `read_timeout`, reconnect, icon report writes | Normal priority; bounded command queue; coalesce icon updates |
| PipeWire callback | Acquire buffer, publish bounded audio block/meter, requeue | PipeWire RT priority; no allocator/locks/I/O |
| DSP worker | Windowing, FFT, bands, beat detection, smoothing | Elevated normal/SCHED_FIFO only after measurement; preallocate plans/buffers |
| Renderer/player | Decode/render/presentation | Isolate from CPU 0 housekeeping; do not starve PipeWire |
| Ingest worker | Thumbnail/clip validation and cache warming | Low priority; never runs during a performance unless explicitly safe |

Do not assign all application threads real-time priority. A runaway high-priority loop can starve USB, PipeWire, systemd, and recovery paths.

## State model

```text
Booting -> DiscoveringDevices -> WarmingBank -> Ready
                                         └──> DegradedReady

Ready --Trigger--> Switching --FramePresented--> Playing
Playing --Trigger--> Switching

VideoBackendFailure -> RestartingVideo -> Ready/Playing
DeckDisconnected -> ReadyWithoutDeck -> Ready
AudioDisconnected -> VisualsWithoutAudio -> Ready
FatalConfigError -> Failed
```

Each transition emits a structured event with a monotonic timestamp. Device loss must not destroy unrelated state.

## Interfaces

Use bounded channels. The suggested logical types are:

```rust
struct Trigger {
    key: u8,
    received_at: Instant,
}

enum PlayerCommand {
    Warm { clip: ClipId },
    Trigger { clip: ClipId, requested_at: Instant },
    Stop,
    Shutdown,
}

enum PlayerEvent {
    Ready { clip: ClipId },
    FirstFrameReady { clip: ClipId, at: Instant },
    Presented { clip: ClipId, at: Instant },
    Failed { clip: Option<ClipId>, error: String },
}

struct AudioFeatures {
    sequence: u64,
    captured_at: Instant,
    rms: [f32; 2],
    peak: [f32; 2],
    bass: f32,
    mid: f32,
    treble: f32,
    spectral_flux: f32,
    beat: bool,
}
```

The render thread reads only the latest `AudioFeatures` snapshot. It must not queue stale analysis frames.

## Clip-bank model

Each clip has immutable ingest metadata:

```toml
[[clips]]
id = "tunnel-green"
path = "/opt/rpi-vj/clips/tunnel-green.mp4"
key = 0
thumbnail = "/opt/rpi-vj/cache/tunnel-green-key.jpg"
loop_mode = "seamless"
warm_priority = 100
```

At startup, validate:

- file exists and is readable;
- codec/profile, dimensions, frame rate, pixel format, color metadata, duration, first random-access point;
- thumbnail is device-ready;
- total configured warm-cache budget fits the configured memory limit.

Invalid clips become disabled keys with an error icon; they must not crash the service.

## Video presentation strategy

### Phase 1

One mpv process; hard cuts via JSON IPC; HEVC hardware decode requested; display synchronization enabled; no application shaders. This validates hardware, OS, DRM, clip normalization, and latency instrumentation.

### Phase 2

Add a bounded hot-set manager. If mpv cannot retain independent ready decoder slots, move this responsibility to GStreamer or libmpv instances. A trigger swaps a ready buffer at vblank. Cold clips fall back to the single-player path and expose a degraded-latency metric.

### Phase 3

Own GBM/EGL and import decoder DMABUFs as textures. Render video and audio-reactive GLSL into the output surface. Keep decode, texture import, shader rendering, and KMS commit on a clearly owned render thread/context.

## Audio feature pipeline

```text
F32 stereo 256-frame blocks
 -> DC removal / optional gain
 -> overlap accumulator (for example 1024 samples, 75% overlap)
 -> Hann window
 -> real FFT
 -> power spectrum
 -> log-frequency bass/mid/treble aggregation
 -> attack/release smoothing
 -> spectral-flux onset detector
 -> immutable latest feature snapshot
```

Exact band boundaries and smoothing constants are artistic configuration, not RT-callback concerns. Clamp and sanitize all values before uploading uniforms.

## Reliability and observability

- Structured logs include component, state transition, clip ID, device identity, and monotonic timing deltas.
- Counters: triggers, cold triggers, missed deadlines, dropped frames, player restarts, HID reconnects, PipeWire xruns, audio blocks dropped, HDMI reconnects.
- Gauges: CPU per core, temperature, throttle state, resident memory, hot decoder slots, audio callback duration, render duration.
- Keep a small in-memory flight recorder of recent transitions and timing samples; flush it only from a non-RT thread after failure.
- Add a watchdog heartbeat only after main-loop liveness accurately represents useful service health.

## Security and service isolation

- Run as an unprivileged dedicated user with `video`, `render`, `audio`, and narrowly scoped HID access.
- Keep `NoNewPrivileges`, filesystem protection, and an explicit device allowlist.
- The mpv IPC endpoint must be private to the service user. Do not expose unauthenticated mpv IPC over the network.
- Treat clip paths/configuration as untrusted input: canonicalize within the clip root and pass arguments without a shell.

