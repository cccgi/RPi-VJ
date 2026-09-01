# Current State

## What the repository currently implements

The repository is a small Rust binary, not yet the modular engine described in the target architecture.

### Playback

`src/player.rs` starts one `mpv` subprocess with DRM/KMS output and sends newline-delimited JSON commands over stdin. Selecting a clip issues `loadfile <path> replace`.

Current characteristics:

- one playback deck;
- no hidden-deck preroll or crossfade;
- no frame-ready acknowledgement before activating a clip;
- no custom GPU compositor;
- no decoded texture access;
- no shader host;
- no explicit loop command in application state;
- child-exit monitoring only.

### Stream Deck

`src/streamdeck.rs` directly uses `hidapi` and supports the two configured Stream Deck XL product IDs. It:

- reconnects after disconnect;
- detects rising-edge button presses;
- writes paged JPEG reports to 32 keys;
- caches normal and active icons in memory;
- restores icons after reconnect;
- generates placeholder hash-pattern icons.

It does not yet provide pages, parameter controls, thumbnail extraction, update coalescing, key-release events, long presses, brightness control, or prioritized feedback.

### Application control

`src/main.rs` maps command-line clip paths directly to keys and connects two unbounded crossbeam channels. It has no durable state model, typed engine command bus, configuration loader, telemetry, health endpoint, or shutdown coordinator.

### Deployment assets

The repository contains initial systemd, udev, PipeWire, and installation files. They must be revalidated whenever device ownership, executable arguments, or the graphics backend changes.

## Preserve during refactoring

- Stream Deck reconnect behavior and cached icon replay.
- Direct non-desktop deployment intent.
- Child/device error propagation through `anyhow` and structured logs.
- Release profile settings appropriate for an appliance.
- A runnable vertical slice after every phase; do not replace the launcher with a large non-running scaffold.

## Known technical debt

- Unbounded channels can grow without limit during device or consumer stalls.
- `mpv` IPC is write-only; the application cannot determine first-frame readiness or authoritative playback state.
- `mpv` is configured with OpenGL despite Vulkan being the preferred long-term Pi 5 backend.
- JPEG generation and storage are not rate-limited or content-addressed.
- Control types are tied directly to one device instead of domain commands.
- No automated tests cover HID report paging, reconnect state, command reduction, or player failure behavior beyond icon encoding.

## Definition of the next safe increment

Before adding shaders, introduce a typed state/control core around the existing launcher:

1. Add stable IDs and `EngineCommand`/`EngineEvent` types.
2. Replace unbounded queues with bounded queues and parameter coalescing.
3. Add a reducer-owned `EngineState` snapshot.
4. Adapt Stream Deck presses into domain commands.
5. Preserve current playback through an `MpvPlayer` implementation of a `VideoDeck` trait.

This produces a testable seam for replacing `mpv` later without blocking all other development.

