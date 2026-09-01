# Repository Layout

## 1. Workspace

```
vj/
├── Cargo.toml                     workspace root
├── rust-toolchain.toml
├── crates/
│   ├── vj-types/                  IDs, parameter values, errors. Zero platform deps.
│   ├── vj-core/                   reducer, EngineState, commands/events, show model
│   ├── vj-tempo/                  tempo arbiter, Link, MIDI clock, tap, LFO bank
│   ├── vj-audio/                  capture trait + platform impls, DSP analyzer
│   ├── vj-render/                 wgpu device, render graph, compositor, texture pool
│   ├── vj-isf/                    ISF 2 parse/validate/compile, pipeline cache
│   ├── vj-video/                  VideoDeck trait, clip index, decode backends
│   ├── vj-surface/                virtual surface, bindings, takeover, learn, feedback
│   ├── vj-surface-midi/           midir transport + generic MIDI profile engine
│   ├── vj-surface-hid/            hidapi transport + Stream Deck
│   ├── vj-devices/                device profiles + SysEx drivers
│   ├── vj-api/                    admin API, show file I/O
│   └── vj-app/                    the binary — wiring, config, supervisor
├── profiles/
│   ├── models/                    SurfaceModel TOML, one per controller
│   │   ├── elgato.stream-deck-xl.toml
│   │   ├── akai.apc-key-25-mk1.toml
│   │   ├── akai.apc-key-25-mk2.toml
│   │   ├── akai.fire.toml
│   │   ├── akai.mpk-mini-mk3.toml
│   │   ├── novation.launchpad-mini-mk3.toml
│   │   └── behringer.x-touch-mini.toml
│   └── bindings/                  default + user bindings per model
├── shaders/                       ISF library
├── shows/                         show files
├── config/
│   ├── system.toml                backend, output mode, logging, watchdog
│   ├── audio.toml                 device, rate, FFT, bands, smoothing, sensitivity
│   ├── surfaces.toml              device identity pinning, per-device binding choice
│   └── tempo.toml                 authority policy, quantize defaults, Link
├── deploy/
│   ├── pi/                        systemd, udev, PipeWire drop-ins, install script
│   ├── windows/
│   └── macos/
├── tools/
│   ├── vj-ingest/                 transcode to tier renditions, probe, thumbnails
│   ├── vj-bench/                  decode/import/render benchmarks per Gate
│   └── vj-latency/                HID-to-photon harness
└── docs/v2/                       this document set
```

## 2. Dependency rules

```
vj-types  ← everything (leaf, no deps beyond serde)
vj-core   ← vj-types
vj-audio, vj-tempo, vj-video, vj-isf, vj-surface  ← vj-types (+ vj-core for command types)
vj-render ← vj-types, vj-isf
vj-surface-midi, vj-surface-hid ← vj-surface, vj-types
vj-devices ← vj-surface, vj-surface-midi, vj-surface-hid
vj-api    ← vj-core, vj-types
vj-app    ← all
```

Enforced rules:

- **No crate below `vj-app` names a specific controller.** `vj-devices` names models; `vj-surface` and above see only regions and capabilities.
- **No crate except `vj-render` calls a GPU API.** Decoders hand out opaque `VideoFrameHandle`s; the renderer performs import.
- **Platform `cfg` blocks appear only in** `vj-audio`, `vj-video`, `vj-render`'s surface/present module, and the two `vj-surface-*` transports. A `cfg(target_os)` anywhere else is a design error and should fail review.
- **`vj-types` has no I/O.** It is the vocabulary, shared by tools and tests.

## 3. Crate responsibilities

**`vj-types`** — `ClipId`, `ShaderId`, `LayerId`, `DeckId`, `SurfaceId`, `RequestId`, `ParameterPath`, `ParameterValue`, `BlendMode`, `Quantize`, `TransitionSpec`, `Tier`, and the error enums. Everything is `Clone + Serialize + Deserialize`.

**`vj-core`** — `EngineCommand`, `EngineEvent`, `EngineState`, the reducer, the show model, and the effect types workers consume. Pure logic, fully unit-testable with no devices present. This is where the command/state table tests live and it is the crate that should have the highest coverage in the workspace.

**`vj-audio`** — `AudioCapture` trait plus `PipeWireCapture` / `CpalCapture`, the analyzer, and `AudioFeatures`. The analyzer is deterministic and testable against WAV fixtures with no audio device attached — a hard requirement, since most development happens far from the DJ booth.

**`vj-tempo`** — `TempoArbiter`, source adapters, the quantized launch scheduler, and the LFO bank. Testable against synthetic clocks.

**`vj-video`** — `VideoDeck` trait, `ClipIndex`, the hot-slot pool, and the per-platform decode backends behind features. Frame handles are opaque; the crate never touches wgpu.

**`vj-isf`** — ISF 2 parsing, embedded-profile validation, GLSL assembly, shaderc → SPIR-V, and the on-disk pipeline cache keyed by `(source hash, profile version, backend, tier)`. Runs off the render thread by construction — it has no access to a GPU device.

**`vj-render`** — the only crate that owns a `wgpu::Device`. Render graph, compositor passes, blend and key modes, texture/target pool, frame import, presentation. The Pi GLES escape hatch, if taken, lives entirely in a backend submodule here.

**`vj-surface`** — the virtual surface: regions, bindings, banking, soft takeover, MIDI learn, the feedback worker, and the diffing feedback cache. Sees `SurfaceModel` and `FeedbackFrame`, never a product name.

**`vj-devices`** — model TOML loading and the SysEx drivers (Fire pad bulk + OLED, Launchpad Programmer Mode + RGB, APC mk2 RGB, X-Touch ring modes). Small crate, high test value: driver encode/decode round-trips against captured byte fixtures.

**`vj-app`** — configuration, wiring, thread supervision, signal handling, graceful shutdown, metrics export. Thin by design; if logic accumulates here it belongs in `vj-core`.

## 4. Key contracts

```rust
pub trait VideoDeck: Send {
    fn prepare(&mut self, clip: &ClipSpec) -> Result<PrepareToken, VideoError>;
    fn activate(&mut self, token: PrepareToken, at: SwitchTime) -> Result<(), VideoError>;
    fn seek(&mut self, position_ns: u64) -> Result<(), VideoError>;
    fn latest_frame(&self) -> Option<VideoFrameHandle>;
    fn state(&self) -> DeckState;
}

pub trait AudioCapture: Send {
    fn start(&mut self, sink: Box<dyn FnMut(&[f32], u32) + Send>) -> Result<(), AudioError>;
    fn stop(&mut self);
    fn info(&self) -> CaptureInfo;   // rate, channels, quantum, device name
}

pub trait ControllerAdapter: Send {
    fn surface_id(&self) -> SurfaceId;
    fn poll(&mut self, out: &mut Vec<ControlEvent>) -> Result<(), SurfaceError>;
    fn apply_feedback(&mut self, frame: &FeedbackFrame) -> Result<(), SurfaceError>;
    fn connected(&self) -> bool;
}

pub trait ShaderRepository: Send + Sync {
    fn metadata(&self, id: ShaderId) -> Option<&ShaderMetadata>;
    fn compiled(&self, id: ShaderId, tier: Tier) -> Result<CompiledShader, ShaderError>;
}
```

`SurfaceDriver`, `SurfaceModel`, `ControlEvent`, and `FeedbackFrame` are specified in [`03-CONTROL-SURFACES.md`](03-CONTROL-SURFACES.md).

## 5. Relationship to the existing repository

The current `RPi-VJ` repository is a single-crate binary: `src/main.rs`, `src/player.rs` (mpv subprocess + JSON IPC), `src/streamdeck.rs` (raw hidapi), and `native/pw_capture.c`. It is Track A's starting point and stays where it is.

Track B starts a clean workspace beside it. Two things migrate:

- The Stream Deck reconnect and cached-icon-replay behaviour → generalised into `vj-surface`'s feedback cache.
- The PipeWire capture C code → replaced by `vj-audio`'s Rust `PipeWireCapture`, but its negotiated-format findings (once Gate D validates them) are the specification for the replacement.

Nothing else migrates. `src/player.rs` is deliberately a dead end — see [`01-RESEARCH-EVALUATION.md`](01-RESEARCH-EVALUATION.md) §3.3.

## 6. Testing posture

| Layer | How it is tested without hardware |
|---|---|
| Reducer / state | Command→state tables, request lifecycle, queue saturation and coalescing |
| Analyzer | WAV fixtures: silence, sine, pink noise, click train, real track excerpt. Assert no flicker on steady input, quiet on silence, onset timing within tolerance |
| Tempo | Synthetic clock; authority handover; quantize boundary maths |
| ISF | Conformance fixtures — generator, image filter, two-pass blur, persistent feedback, audioFFT input, deliberate compile failure, over-budget request |
| Surface drivers | Captured byte fixtures per device; encode/decode round-trip; feedback diff produces the minimum message set |
| Bindings | Region projection across all six topologies; soft-takeover behaviour across a bank change |
| Render | Golden-image comparison per blend mode at a fixed seed, on each backend |

Hardware-only concerns — decode path, import copies, HID-to-photon latency, thermals, Link phase error — are covered by the gates in [`08-VALIDATION.md`](08-VALIDATION.md), not by unit tests, and must not be claimed as passing until they are measured on the device.
