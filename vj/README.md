# vj — cross-platform VJ engine workspace

Track B's clean-room workspace. See `docs/v2/` for the architecture and roadmap; this file covers only how to build and what is real.

## Status

This is a **scaffold**: the crate boundaries, the shared vocabulary, the command plane, the control-surface abstraction, and the tempo model exist and are tested. The platform backends — GPU, video decode, audio capture, MIDI/HID transport — are trait definitions with the seams cut and no implementations behind them.

```
cargo test --workspace     # 62 tests, all passing
cargo clippy --workspace --all-targets   # no warnings
cargo run -p vj-app        # prints the detected tier and its budget
```

What the tests actually cover — this is the part worth trusting:

| Crate | Verified behaviour |
|---|---|
| `vj-types` | Quantize grid maths respects the meter rather than assuming 4/4; tier ordering |
| `vj-core` | A quantized trigger still prerolls immediately; a trigger on a missing deck is rejected, not panicked; a sweeping encoder coalesces to one queued command instead of growing the queue; a saturated queue drops oldest and counts it |
| `vj-surface` | Relative-CC decoding for all three encodings; absolute-vs-relative inference from a sweep; grid extent derivation; scroll-window projection |
| `vj-surface::takeover` | `scale` converges on the endpoint without ever jumping, in both directions; `pickup` swallows until crossing; a bank change forces re-acquisition |
| `vj-tempo` | Link outranks audio; low-confidence audio is not admitted as authority; losing every source decays confidence instead of zeroing BPM; a press 47 ms late still lands on this bar; authority change is reported once |
| `vj-audio` | Dual-rate envelope rises fast and falls slow; EMA smoothing is rate-independent |
| `vj-video` | Pi hardware-decodes HEVC only; HAP is rejected on the Appliance tier; a missing rendition fails loudly |
| `vj-isf` | Appliance caps passes at four; arbitrary intermediate scales are rejected |
| `vj-surface-hid` | A held key fires exactly once — the bug in the current `RPi-VJ/src/streamdeck.rs` |
| `vj-devices` | Launchpad grid note mapping round-trips; a full 64-pad refresh is one SysEx message; SysEx payload bytes never set the high bit |
| `vj-devices` (integration) | Every shipped device profile parses; X-Touch encoders are relative with rings |

## Layout

```
crates/vj-types           vocabulary — no I/O, no platform code
crates/vj-core            commands, events, state, reducer, coalescing queue
crates/vj-tempo           tempo arbiter, quantized launch scheduler, LFO bank
crates/vj-audio           capture trait, AudioFeatures, dual-rate envelope
crates/vj-video           VideoDeck trait, clip spec, warmth tiers, codec/tier rules
crates/vj-isf             ISF 2 metadata, embedded profile validation, cache key
crates/vj-render          the only crate that may own a GPU device
crates/vj-surface         virtual surface, bindings, soft takeover, feedback frames
crates/vj-surface-midi    MIDI transport
crates/vj-surface-hid     HID transport, button-edge diffing
crates/vj-devices         device profiles + SysEx drivers — the only crate naming products
crates/vj-api             admin API request/response schema
crates/vj-app             the binary
profiles/models           SurfaceModel TOML, one per controller
```

## Rules the layout enforces

- No crate below `vj-app` names a specific controller.
- No crate except `vj-render` calls a GPU API.
- `cfg(target_os)` appears only in `vj-audio`, `vj-video`, `vj-render`'s present module, and the two `vj-surface-*` transports. Anywhere else is a design error.
- `vj-types` has no I/O.

## Features

```
default        = ["surface-hid", "surface-midi"]
backend-v4l2   Pi decode          backend-nvdec  Windows decode
backend-vtb    macOS decode       backend-gles   Pi render escape hatch
tempo-link     Ableton Link
```

## Next

Phase B0's remaining work: CI building all three targets, and replacing `detect_tier()`'s `cfg!` guess with real platform probing. Then Phase B1 — the GPU backend spike, which is the highest-risk item in the program.
