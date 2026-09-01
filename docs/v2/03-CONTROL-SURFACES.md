# Control Surface Layer

This is the subsystem the earlier research does not have. It is specified first because it is the largest new requirement and because both delivery tracks depend on it.

## 1. The problem

Six controllers, four input paradigms, five feedback models, three operating systems:

| Device | Transport | Grid | Continuous controls | Feedback model |
|---|---|---|---|---|
| Stream Deck XL | USB HID | 4 × 8 momentary keys | none | 96 × 96 JPEG bitmap per key |
| APC Key 25 (mk1/mk2) | USB MIDI | 5 × 8 pads | 8 knobs, 25 keys | mk1: 3-colour velocity · mk2: RGB SysEx |
| Akai Fire | USB MIDI | 4 × 16 pads | 4 relative encoders + select | RGB SysEx bulk + 128 × 64 OLED |
| MPK mini mk3 | USB MIDI | 2 × 4 pads × 2 banks | 8 knobs, 25 keys, joystick | none addressable (treat as input-only) |
| Launchpad Mini MK3 | USB MIDI | 8 × 8 pads + 16 function | none | 128-colour palette + RGB SysEx, Programmer Mode |
| X-Touch Mini | USB MIDI | 16 buttons, 2 layers | 8 push-encoders, 1 fader | button LEDs + 13-segment encoder rings with 4 display modes |

Writing six device integrations against the engine directly produces six copies of the same paging, banking, soft-takeover, and feedback-diffing logic, and a seventh copy the day you buy a Launchpad Pro. The layer below exists to make that not happen.

## 2. Three-layer model

```
┌─────────────────────────────────────────────────────────────┐
│ Engine (vj-core)                                            │
│   EngineCommand / EngineEvent / EngineState                 │
└───────────────▲─────────────────────────────┬───────────────┘
                │ commands                    │ state snapshots
┌───────────────┴─────────────────────────────▼───────────────┐
│ 3. Semantic mapping   (vj-surface)                          │
│    VirtualSurface: named regions + bindings + MIDI learn    │
│    soft-takeover · banking/paging · feedback projection     │
└───────────────▲─────────────────────────────┬───────────────┘
                │ ControlEvent                │ FeedbackFrame
┌───────────────┴─────────────────────────────▼───────────────┐
│ 2. Device profile    (vj-devices)                           │
│    SurfaceModel: physical topology + capability descriptor  │
│    TOML profile, optional Rust driver for SysEx devices     │
└───────────────▲─────────────────────────────┬───────────────┘
                │ raw input                   │ raw output
┌───────────────┴─────────────────────────────▼───────────────┐
│ 1. Transport   (vj-surface-midi, vj-surface-hid)            │
│    midir (ALSA seq / CoreMIDI / WinRT-MM) · hidapi          │
│    enumeration · hot-plug · stable identity                 │
└─────────────────────────────────────────────────────────────┘
```

Rule: **layer N never names a concept from layer N+2.** The engine has never heard of a Launchpad. A device profile has never heard of `clip.grid`.

## 3. Layer 1 — Transport

### Identity and hot-plug

A controller is identified by `(vendor_id, product_id, serial_or_port_name, instance_ordinal)`. Two Launchpads on one rig must remain distinguishable across a replug, so identity is persisted in `config/surfaces.toml` and matched on connect. Where a device exposes no serial, the ordinal is assigned by first-seen order and the user can pin it in config.

Hot-plug is normal operation, not an error. On disconnect: keep all mapping state, mark the surface offline, keep the show running. On reconnect: re-initialise the device (including any mode SysEx), then replay the full current feedback frame.

### Cross-platform MIDI gotchas — this is where the three OSes actually diverge

| OS | Backend | Multi-client? | Consequence |
|---|---|---|---|
| Linux (Pi) | ALSA sequencer via `midir` | Yes | Nothing special. Class-compliant devices need no driver. |
| macOS | CoreMIDI via `midir` | Yes | The engine and Resolume can hold the same device simultaneously. |
| Windows | WinMM via `midir` | **No** — exclusive port open | If Resolume or rekordbox has the APC open, the engine cannot open it, and vice versa. |

The Windows exclusivity problem is a real operational constraint for a rig that runs alongside Resolume today. Mitigations, in order of preference: (a) use the Windows MIDI Services / WinRT MIDI path where the device and OS support multi-client; (b) split devices between applications rather than sharing one; (c) a virtual MIDI loopback (loopMIDI) with a small fan-out. Document the chosen approach per show; do not silently fail to open a port.

USB HID (Stream Deck) is multi-client-hostile everywhere — only one process may claim the interface. On Linux this also requires a udev rule granting the service user access to the Elgato hidraw nodes.

## 4. Layer 2 — Device profile and `SurfaceModel`

A profile declares what the device *physically is*. It never declares what a control *does*.

```rust
pub struct SurfaceModel {
    pub id: SurfaceModelId,          // "akai.fire", "novation.launchpad-mini-mk3"
    pub display_name: String,
    pub transport: TransportSpec,     // Midi { in_port, out_port } | Hid { vid, pid }
    pub init: Vec<InitAction>,        // e.g. enter Programmer Mode, select Standard mode
    pub controls: Vec<ControlDescriptor>,
    pub displays: Vec<DisplayDescriptor>,
    pub driver: Option<DriverId>,     // Some(_) for SysEx-heavy devices
}

pub struct ControlDescriptor {
    pub id: ControlId,                // "pad.3.11", "enc.2", "fader.0"
    pub kind: ControlKind,
    pub position: Option<GridPos>,    // row/col for grid membership
    pub input: InputSpec,             // Note{ch,num} | Cc{ch,num,mode} | HidKey{index}
    pub feedback: FeedbackCapability,
}

pub enum ControlKind {
    Pad,                              // momentary, grid member
    Button,                           // momentary, non-grid (transport, scene, function)
    Key { note: u8 },                 // piano key — velocity-sensitive, not a trigger by default
    Encoder { detents: Option<u16> },
    Fader,
    Joystick { axes: u8 },
}

pub enum InputSpec {
    Note { channel: u8, number: u8 },
    Cc { channel: u8, number: u8, mode: CcMode },
    HidKey { index: u8 },
}

pub enum CcMode {
    Absolute,                          // 0..127 — needs soft takeover
    RelativeTwosComplement,            // 1..63 up, 127..65 down
    RelativeSignedBit,                 // 1..63 up, 65..127 down
    RelativeBinaryOffset,              // 64 = zero
}

pub enum FeedbackCapability {
    None,
    Monochrome,                                  // on / off
    Palette { size: u16 },                       // velocity-indexed colour table
    Rgb { via: RgbEncoding },                    // SysEx or per-channel
    Bitmap { w: u16, h: u16, format: ImageFormat },   // Stream Deck key
    EncoderRing { segments: u8, modes: Vec<RingMode> },
    Text { cols: u8, rows: u8 },
}

pub enum RingMode { Single, Pan, Fan, Spread, Trim }
```

Simple devices (MPK mini, X-Touch Mini in Standard mode) are fully described by a TOML profile and need no code. Devices whose LED/display protocol is SysEx bulk transfer get a small Rust driver implementing:

```rust
pub trait SurfaceDriver: Send {
    fn model(&self) -> &SurfaceModel;
    fn initialise(&mut self, out: &mut dyn MidiSink) -> Result<(), SurfaceError>;
    fn decode(&mut self, raw: RawInput, out: &mut Vec<ControlEvent>) -> Result<(), SurfaceError>;
    /// Called at most `feedback_hz` times per second, off the input thread.
    fn encode_feedback(&mut self, frame: &FeedbackFrame, out: &mut dyn MidiSink)
        -> Result<(), SurfaceError>;
    fn shutdown(&mut self, out: &mut dyn MidiSink);
}
```

`encode_feedback` receives the *desired* full state and is responsible for diffing against what it last sent. This is where the Stream Deck's cached-icon replay behaviour from the current `src/streamdeck.rs` gets generalised: every device gets content-hashed, coalesced, priority-ordered updates for free.

## 5. Layer 3 — Virtual surface

The engine exposes a stable set of **named regions**. Device profiles bind physical control ranges to them.

```
clip.grid                 2-D region, arbitrary size, addressed (bank, row, col)
clip.bank.next / .prev    discrete
layer.<n>.opacity         continuous 0..1
layer.<n>.blend           enumerated
layer.<n>.select          discrete
fx.<slot>.param.<n>       continuous 0..1
fx.<slot>.select          discrete
transport.blackout        toggle
transport.tap             discrete
transport.quantize        enumerated (off / 1 / 1-2 / 1-4 / bar / phrase)
master.opacity            continuous
master.speed              continuous, bipolar
```

A binding is a projection of a device region onto an engine region:

```toml
# profiles/bindings/launchpad-mini-mk3.default.toml
schema_version = 1
model = "novation.launchpad-mini-mk3"

[[binding]]
region  = "clip.grid"
controls = "pad.*"          # the 8x8 grid
mode    = "grid"
origin  = { row = 0, col = 0 }
# 8x8 window onto a wider clip grid; scrolls with clip.bank.*

[[binding]]
region  = "clip.bank.next"
controls = "func.right.4"

[[binding]]
region  = "transport.blackout"
controls = "func.top.7"
behaviour = "momentary"
```

```toml
# profiles/bindings/x-touch-mini.default.toml
[[binding]]
region  = "layer.{n}.opacity"     # n = encoder index
controls = "enc.*"
mode    = "fan-out"
takeover = "relative"             # encoder is relative; no takeover needed
ring    = "single"

[[binding]]
region  = "fx.a.param.{n}"
controls = "btn.top.*"
layer   = "B"                      # X-Touch layer B
```

The same `clip.grid` region is served by a 4 × 8 Stream Deck, a 5 × 8 APC, a 4 × 16 Fire, and an 8 × 8 Launchpad — simultaneously, each with its own scroll window. That is the whole point of the layer.

### Soft takeover

Absolute controls (APC knobs, MPK knobs, X-Touch fader) will jump the parameter when a bank or page changes. Per-binding policy:

| Mode | Behaviour |
|---|---|
| `jump` | Value applies immediately. Default for faders where a physical jump is acceptable. |
| `pickup` | Ignored until the physical value crosses the current parameter value. Safest; feels dead until crossed. |
| `scale` | Moves the parameter proportionally toward the endpoint. Best default for knobs. |
| `relative` | Not applicable — the control is already relative. |

`scale` is the recommended default for every absolute knob on the APC and MPK.

### Feedback projection

The engine publishes an immutable `EngineState` snapshot with a revision counter. A feedback worker (never the render thread, never the input thread) projects it through each active binding into a per-device `FeedbackFrame`, then hands the frame to the driver, which diffs and emits.

Priority order, enforced by the worker when a device's output budget is saturated:

1. Press/release acknowledgement, blackout, and error state
2. Active clip / active shader selection
3. Parameter values (encoder rings, fader LEDs)
4. Thumbnails and clip-loaded state
5. Decorative meters and beat animation

Rate caps: bitmap devices ≤ 15 fps of image updates; MIDI LED devices ≤ 30 Hz of *changed* messages; decorative tier ≤ 10 Hz. Full-grid SysEx refreshes are permitted only on connect, mode change, and bank change.

## 6. Per-device research notes

Confidence is marked. Anything marked *validate on target* should be confirmed with a MIDI monitor against your actual unit before it is hardcoded — and the MIDI-learn path (§7) means it does not have to be hardcoded at all.

### Stream Deck XL — *verified*

USB HID, vendor `0x0fd9`. Two product IDs cover the XL revisions in the current code (`0x006c`, `0x008f`) — confirm both against your units. 32 keys, 4 × 8, 96 × 96 px, JPEG for XL (older models use BMP). No MIDI. Requires a udev rule on Linux for non-root hidraw access. Use the `elgato-streamdeck` crate rather than hand-rolled report framing (see [`01`](01-RESEARCH-EVALUATION.md) §3.4).

Role in your rig: **the clip-launch and page-navigation surface**, because it is the only device that can show a clip thumbnail on the button.

### Novation Launchpad Mini MK3 — *verified, public spec*

The only one of the six with a proper public **Programmer's Reference Manual**. Two USB MIDI ports are exposed ("DAW" and "MIDI"); bind the MIDI port unless you specifically want the DAW-port behaviour. The device must be put into **Programmer Mode** with a SysEx message before the note layout and full LED addressing behave as documented — this is the single most common integration mistake with this device, and it must be re-sent on every reconnect.

LEDs: 128-colour palette addressed by note velocity for cheap updates, plus SysEx for arbitrary RGB and for lighting many pads in one message. Flash and pulse modes exist in hardware and are worth using — a hardware-pulsing "cued" pad costs one message instead of a 30 Hz animation.

Role: **the primary clip grid** — 8 × 8 RGB, cheap updates, hardware pulse for cue state.

### Akai Fire — *community-reverse-engineered, no official spec*

Class-compliant USB MIDI. 4 × 16 RGB pad grid, four relative rotary encoders plus a select encoder, transport buttons, and a 128 × 64 monochrome OLED. Akai publishes no MIDI implementation chart; the working reference is the SEGGER three-part reverse-engineering series, plus the FL Studio community's findings.

What matters architecturally: pad colours are set through a **SysEx bulk update** carrying a run of `(index, r, g, b)` 7-bit triples, so lighting the whole grid is one message rather than 64 — which makes the Fire the *cheapest* device to refresh but means the driver must build and diff a full grid buffer rather than emit per-pad messages. The OLED is a SysEx bitmap blit with a non-obvious pixel packing; treat it as a low-rate status display (transport state, BPM, active bank), never an animation target.

Encoders are relative — no soft takeover needed, which makes the Fire's four encoders the best home for FX parameters.

Role: **FX parameters + a 4 × 16 secondary grid + status readout.**

### Akai APC Key 25 — *validate on target; mk1 and mk2 differ substantially*

Two hardware generations with materially different LED protocols share a product name:

- **mk1**: 5 × 8 pads with three-colour LEDs (green / red / amber, plus blink states) addressed by note velocity. No RGB.
- **mk2**: RGB pads, addressed by a velocity-indexed palette for basic colours and SysEx for full RGB, in the same family as the APC40 mk2 / APC mini mk2 protocol.

Detect the generation from the USB product ID at connect and load the corresponding profile; do not attempt one profile with runtime branches. The 25 keys should default to **not** being clip triggers — map them to a shader-parameter or note-driven modulation region, since a keyboard under a VJ's hands during a set is more useful as an expressive input than as 25 more launch buttons. The 8 knobs are absolute CC → `scale` takeover.

Role: **note-driven modulation + a secondary grid + 8 macro knobs.**

### Akai MPK mini mk3 — *validate on target; treat as input-only*

8 velocity pads across 2 banks, 8 knobs, 25 keys, 4-way joystick, on-board arpeggiator. Pad LEDs are backlit but host-addressable feedback is not documented and should not be assumed — profile it with `FeedbackCapability::None` until proven otherwise, and design its bindings so that no feedback is needed (momentary actions, not toggles whose state you cannot see).

Knobs transmit absolute CC by default and the on-device editor can change assignments, so this is the device most likely to arrive with a different map than the profile expects. It is the strongest argument for MIDI learn (§7).

Role: **portable expressive input** — pads as one-shot triggers, knobs as macros, keys as modulation.

### Behringer X-Touch Mini — *validate on target; configure before deploying to the Pi*

Two operating modes selected on the device: **Standard (MIDI)** — the one to use — and **MC (Mackie Control)**. In Standard mode: 8 push-encoders with 13-segment LED rings, 16 buttons in two rows, one 60 mm fader, and two layers (A/B) that double the control count.

Two practical gotchas:

- Encoder ring **display mode** (single dot / pan / fan / spread / trim) is set at runtime by CC and is what makes a bipolar parameter readable at a glance. Bind ring mode to parameter type: `pan` for bipolar, `fan` for unipolar, `single` for enumerated.
- Encoders can be configured absolute or relative, and that configuration is persisted **on the device** via Behringer's X-Touch Editor, which is Windows/macOS only. **Configure the unit on your PC or Mac before it ever goes near the Pi**, and record the resulting configuration in the repo so a factory-reset unit can be restored. Set encoders to relative.

Role: **the mixer surface** — 8 layer opacities on encoders with rings, master on the fader, blend modes on the buttons.

### A note on the DDJ-FLX10

The FLX10 is not in this list because in rekordbox or Serato it runs in the vendor's native/HID mode and is not available as a general MIDI source to a third application. Its role in this architecture is as the **audio source** (analogue line into the Pi's USB interface today) and, indirectly, as the tempo source through rekordbox's Ableton Link — see [`05-TEMPO-AND-SYNC.md`](05-TEMPO-AND-SYNC.md). Worth noting for later: the FLX10 has two computer USB ports, so routing its audio to the Pi over USB instead of analogue is a plausible future simplification, subject to confirming class-compliant behaviour on Linux.

## 7. MIDI learn

Given that three of the five MIDI devices have no authoritative public spec, MIDI learn is not a convenience feature — it is the mechanism by which profiles become correct.

- Enter learn mode from the admin API or a bound control.
- Select an engine region; move or press the physical control; the engine captures `(port, channel, message type, number, observed value range, inferred CcMode)`.
- `CcMode` is *inferred*: sweep the control and observe whether values are monotonic 0..127 (absolute) or clustered around 1–3 / 125–127 (relative two's complement) or 63–65 (binary offset). Ask for confirmation rather than guessing silently.
- Learned bindings persist to `profiles/bindings/<model>.<name>.toml` via write-temp-plus-atomic-rename, and can be committed to the repo.

Learn writes bindings, never `SurfaceModel`s. Physical topology stays declarative.

## 8. Threading and prohibited work

| Thread | Owns | Prohibited |
|---|---|---|
| MIDI input (one per device) | `midir` input connection | allocation per message, engine state mutation, GPU, disk |
| HID input (one per device) | blocking `read_timeout` loop | image encoding, thumbnail extraction, engine state mutation |
| Feedback worker (one, shared) | per-device `FeedbackFrame` cache and diff | GPU calls, blocking on the render thread, video decode |
| Learn/config worker | mapping capture, TOML persistence | anything on the live path |

Input threads emit `ControlEvent` into a bounded MPSC queue. The reducer converts to `EngineCommand`. No adapter mutates playback, render, or layer state directly — the rule Set D established for the Stream Deck, generalised to all surfaces.

Thumbnails and any per-key image are produced by the media indexer at ingest and cached as device-ready encoded buffers. Nothing is resized, rendered, or JPEG-encoded after a button press, on any device, ever.

## 9. Acceptance criteria for this subsystem

- All six devices connect simultaneously and drive `clip.grid` from independent scroll windows.
- Unplugging any device leaves the show running; replugging restores its exact current page, including re-entering Programmer Mode / Standard mode where required.
- No missed rising edges under rapid multi-key input on any device while the GPU is at full load.
- A seventh controller can be added by writing one TOML profile, with a Rust driver required only if its LED protocol is SysEx bulk.
- Absolute knobs never jump a parameter after a bank change under the `scale` policy.
- Feedback traffic on every device stays within its rate cap under a beat-synchronised decorative animation.

## Sources

- [Launchpad Mini MK3 Programmer's Reference Manual](https://www.djshop.gr/Attachment/DownloadFile?downloadId=10737)
- [Novation Launchpad Mini MK3 Reference Manual](https://manualzz.com/doc/53273609/novation-launchpad-mini-mk3-manual)
- [Decoding the Akai Fire MIDI implementation — part 1 (SEGGER)](https://blog.segger.com/decoding-the-akai-fire-part-1/)
- [Decoding the Akai Fire MIDI implementation — part 2 (SEGGER)](https://blog.segger.com/decoding-the-akai-fire-part-2/)
- [Decoding the Akai Fire MIDI implementation — part 3 (SEGGER)](https://blog.segger.com/decoding-the-akai-fire-part-3/)
- [nsmith-/mpk2 — Akai MPK SysEx program editing](https://github.com/nsmith-/mpk2)
- [streamduck-org/elgato-streamdeck](https://github.com/streamduck-org/elgato-streamdeck)
