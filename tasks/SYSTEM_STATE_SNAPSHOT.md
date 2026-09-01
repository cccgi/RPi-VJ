# RPi-VJ System State & Codebase Snapshot
**Generated for:** Multi-Agent Collaboration (Claude Opus 5 & Antigravity)  
**Snapshot Date:** August 25, 2026

---

## 1. Directory Structure & Key Files

```
RPi-VJ/
├── tasks/                                # Multi-Agent Protocol & Tracking
│   ├── ORCHESTRATION_WORKFLOW.md         # File-based Agent protocol
│   ├── TRACKER.md                        # Master status board
│   ├── CLAUDE_COLLABORATION_BRIEFING.md  # Main briefing for Claude Opus 5
│   ├── SYSTEM_STATE_SNAPSHOT.md          # This snapshot file
│   ├── specs/                            # Micro-task specs (TASK-00 to 13)
│   └── reviews/                          # Review logs (REVIEW-00-05, 00-06, 11-13)
│
├── vj/
│   ├── Cargo.toml                        # Workspace root
│   ├── crates/                           # Modular Rust Crates
│   │   ├── vj-types/                     # Show manifest, layers, grid schemas, SystemActions
│   │   ├── vj-core/                      # State reducer, commands, event dispatch
│   │   ├── vj-audio/                     # CPAL capture, 5-band crossover DSP, kick detector
│   │   ├── vj-surface-hid/               # Stream Deck XL USB HID driver & 96x96 icon cache
│   │   ├── vj-surface-midi/              # Behringer X-Touch Mini driver & LED rings
│   │   ├── vj-video/                     # GStreamer dual-deck hardware HEVC closed-GOP player
│   │   ├── vj-isf/                       # ISF JSON metadata parser & GLSL code generator
│   │   ├── vj-render/                    # Linear alpha compositor & shader pipeline
│   │   ├── vj-tempo/                     # Ableton Link session tracking & phase sync
│   │   ├── vj-api/                       # WebSocket telemetry & remote control server
│   │   └── vj-app/                       # Master EngineRunner runtime loop
│   │
│   ├── tools/
│   │   └── vj-ingest/                    # CLI for transcode, USB export & SSH sync
│   │
│   ├── shaders/                          # Curated & 3D Raymarching ISF Shader Library
│   │   ├── vs_dimensional_monolith.fs    # 3D Raymarched Brutalist Monoliths
│   │   ├── vs_sacred_polyhedron.fs       # 3D Sacred Geometry & Exploding Shards
│   │   ├── vs_cyber_wormhole.fs          # 3D Relativistic Cyber Wormhole
│   │   ├── vs_granular_nebula.fs         # 3D Volumetric Fluid Nebula
│   │   ├── vs_shattered_glitch_core.fs   # 3D Fractured Glitch Torusknot
│   │   ├── neon_tunnel.fs                # 3D Speed Tunnel
│   │   ├── audio_wave_rings.fs           # Concentric Audio Wave Rings
│   │   ├── spectrum_bars.fs              # 5-Band Equalizer
│   │   ├── cyber_grid.fs                 # Retro Synthwave 3D Grid & Sun
│   │   ├── kaleidoscope_mirror.fs        # Fractal Prism Mirror
│   │   ├── plasma_flow.fs                # Liquid Acid Plasma
│   │   ├── particle_warp.fs              # Quantum Warp Field
│   │   ├── audio_oscilloscope.fs         # Laser Oscilloscope Beam
│   │   └── glitch_strobe.fs              # Kick Invert Strobe
│   │
│   ├── web/                              # Studio UI & Master 60 FPS Live Preview
│   │   ├── index.html                    # 4x8 matrix, Master HDMI dock, modals
│   │   ├── style.css                     # Dark carbon UI, neon pulsing animations, HUD
│   │   └── app.js                        # Master compositor, drag-and-drop, audio engine
│   │
│   └── deploy/pi/                        # Headless Raspberry Pi 5 Deployment
│       ├── rpi-vj.service                # DRM/KMS systemd service
│       ├── 99-vj-controllers.rules       # udev rules for Stream Deck, X-Touch, Sound Blaster
│       └── setup-pi.sh                   # Automated OS provisioning script
```

---

## 2. Key Data Models & Protocols

### Show Manifest (`vj/crates/vj-types/src/show.rs`)
```rust
pub struct ShowManifest {
    pub schema_version: u32,
    pub name: String,
    pub bpm_default: f32,
    pub beats_per_bar: u32,
    pub requirements: SystemRequirements,
    pub banks: Vec<BankConfig>,
    pub layers: Vec<LayerConfig>,
}

pub enum SystemAction {
    BankNext,
    BankPrev,
    BankSelect(usize),
    Blackout,
    TapTempo,
    BpmUp,
    BpmDown,
    BeatSync,
    PhaseNudgeHalf,
}
```

### Audio DSP Spectrum (`vj/crates/vj-audio/src/analyzer.rs`)
```rust
pub struct AudioSpectrum {
    pub sub_bass: f32,   // 20 - 75 Hz
    pub bass: f32,       // 75 - 200 Hz
    pub low_mid: f32,    // 200 - 1000 Hz
    pub high_mid: f32,   // 1000 - 5000 Hz
    pub treble: f32,     // 5000 - 16000 Hz
    pub kick_onset: bool,
    pub energy_ema: f32,
}
```

### Hardware Mapping Summary
*   **Stream Deck XL:** 32 physical keys (4 rows $\times$ 8 columns). Banks 1–3 mapped to pages. In-slot live feedback with 96x96 hardware icon cache.
*   **Behringer X-Touch Mini:** Encoders 1–8 (CC 16–23, Ch 0), LED Rings (CC 48–55), Faders (CC 9), Buttons (Notes 0–23).
*   **Creative Sound Blaster PLAY! 3:** 48kHz / 16-bit stereo input via PipeWire/CPAL buffer 256.

---

## 3. How to Run & Verify

1.  **Run All Unit Tests:**
    ```bash
    cargo test --workspace
    ```
2.  **Open Studio UI & Live HDMI Preview:**
    Open `vj/web/index.html` in any browser.
3.  **Transcode Media for Pi 5:**
    ```bash
    cargo run -p vj-ingest -- transcode --input ./raw_clips/ --output ./media/hevc/
    ```
4.  **Export Show to USB:**
    ```bash
    cargo run -p vj-ingest -- export-usb --manifest show.toml --output D:\
    ```
