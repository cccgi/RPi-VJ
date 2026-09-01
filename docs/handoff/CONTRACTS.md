# Implementation Contracts

These contracts define architectural boundaries. Exact crate APIs may change, but implementations must preserve ownership, real-time, and acknowledgment semantics.

## Domain commands

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineCommand {
    TriggerClip {
        request_id: RequestId,
        deck: DeckId,
        clip_id: ClipId,
        transition: TransitionSpec,
        quantize: Quantize,
    },
    SelectShader {
        request_id: RequestId,
        layer: LayerId,
        shader_id: ShaderId,
    },
    SetParameter {
        target: ParameterPath,
        value: ParameterValue,
        ramp_ms: u32,
    },
    SetLayerOpacity { layer: LayerId, value: f32, ramp_ms: u32 },
    SetBlendMode { layer: LayerId, mode: BlendMode },
    Blackout { enabled: bool },
}
```

`TriggerClip` acceptance does not mean the clip is visible. The lifecycle is `CommandAccepted -> ClipPrepared -> ClipActivated` or a failure event.

## Engine events

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EngineEvent {
    CommandAccepted { request_id: RequestId },
    CommandRejected { request_id: RequestId, code: ErrorCode, message: String },
    ClipPrepared { request_id: RequestId, deck: DeckId, clip_id: ClipId },
    ClipActivated {
        request_id: RequestId,
        deck: DeckId,
        clip_id: ClipId,
        actual_time_ns: u64,
    },
    ShaderCompileFailed { shader_id: ShaderId, diagnostics: Vec<ShaderDiagnostic> },
    DeviceStatusChanged { device: DeviceKind, connected: bool },
    StateChanged { revision: u64 },
}
```

## Audio snapshot

Use a fixed-capacity layout. Do not allocate a `Vec` for each update.

```rust
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AudioFeatures {
    pub sequence: u64,
    pub captured_at_ns: u64,
    pub rms: [f32; 2],
    pub volume: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub onset: f32,
    pub beat_trigger: u32,
    pub beat_phase: f32,
    pub bpm: f32,
    pub beat_confidence: f32,
    pub spectrum_len: u32,
    pub spectrum: [f32; 256],
}
```

The render thread reads the newest complete snapshot and may skip intermediate analyzer updates. It must never wait for the analyzer.

## Component traits

```rust
pub trait VideoDeck: Send {
    fn prepare(&mut self, clip: &ClipSpec) -> Result<PrepareToken, VideoError>;
    fn activate(&mut self, token: PrepareToken, at: SwitchTime) -> Result<(), VideoError>;
    fn seek(&mut self, position_ns: u64) -> Result<(), VideoError>;
    fn latest_frame(&self) -> Option<VideoFrameHandle>;
}

pub trait ShaderRepository: Send + Sync {
    fn metadata(&self, id: ShaderId) -> Option<&ShaderMetadata>;
    fn compiled(&self, id: ShaderId) -> Result<CompiledShaderAsset, ShaderError>;
}

pub trait ShaderHost {
    fn instantiate(&mut self, asset: CompiledShaderAsset) -> Result<ShaderInstanceId, ShaderError>;
    fn set_parameter(&mut self, instance: ShaderInstanceId, id: ParameterId, value: ParameterValue)
        -> Result<(), ShaderError>;
    fn render(&mut self, instance: ShaderInstanceId, frame: FrameContext, target: RenderTarget)
        -> Result<(), RenderError>;
}

pub trait ControllerAdapter: Send {
    fn poll(&mut self, output: &mut Vec<ControlEvent>) -> Result<(), ControlError>;
    fn apply_feedback(&mut self, snapshot: &ControllerFeedback) -> Result<(), ControlError>;
}
```

GPU handles must be opaque and render-backend-owned. No control or decoder component may call GPU APIs without going through the renderer's import/install command.

## Audio shader contract

All project shaders may use:

```glsl
uniform float u_bass;
uniform float u_mid;
uniform float u_treble;
uniform float u_volume;
uniform float u_beat_onset;
uniform float u_beat_phase;
uniform float u_bpm;
uniform float u_beat_confidence;
uniform sampler2D u_fft;
uniform int u_fft_bins;
```

Ranges:

- energy, volume, onset, phase, and confidence: normalized `0.0..1.0`;
- BPM: beats per minute, zero until a usable estimate exists;
- FFT texture: normalized perceptual/log-spaced magnitudes, left-to-right low-to-high frequency;
- onset: immediate impulse with a configurable 80–140 ms visual decay;
- beat trigger, if exposed as an ISF event, is true for one rendered frame.

For standard ISF files, these appear in `INPUTS`; `u_fft` uses `TYPE: audioFFT`.

## JSON wire example

```json
{
  "type": "trigger_clip",
  "request_id": "7ee5f03d-e3a5-4882-8f21-48eb5e607ace",
  "deck": "a",
  "clip_id": "tunnel-red-01",
  "transition": { "kind": "crossfade", "duration_ms": 120 },
  "quantize": "immediate"
}
```

JSON is for configuration and low-rate administration. Audio features and per-frame render state stay in process. If the control plane is later separated, use a versioned protobuf or fixed shared-memory record for high-rate telemetry.

## Configuration ownership

Suggested files:

```text
config/system.toml       graphics backend, output mode, logging, watchdog
config/audio.toml        device selector, rate, FFT, bands, smoothing, beat sensitivity
config/show.toml         layers, clip/shader assets, transitions, initial state
config/streamdeck.toml   profiles, pages, bindings, feedback style
```

Every persisted schema requires a `schema_version`. Reject unknown major versions with a clear diagnostic. Save mutable show state through write-to-temporary plus atomic rename.

