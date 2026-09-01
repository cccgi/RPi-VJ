# RPi-VJ — Status (as of this handoff)

Blunt inventory of what's actually been verified vs. assumed. Read this
before trusting any claim elsewhere in the repo's comments.

## Environment this was built in

Cloud sandbox, x86_64 Linux, no Raspberry Pi, no Stream Deck XL, no USB
audio interface, no HDMI display attached. Everything below was verified
to the extent that environment allowed and no further.

## Verified

- **`cargo check --workspace` and `cargo build --workspace` both succeed**,
  compiling all four crates (`vj-core`, `vj-video`, `vj-streamdeck`,
  `vj-audio`) against their real dependency versions — not mocked or
  feature-gated out. This was done on x86_64 with matching native dev
  headers installed (`libpipewire-0.3-dev`, `libspa-0.2-dev`,
  `libhidapi-dev`, `libudev-dev`) — the same packages
  `scripts/setup.sh` installs on the target Pi.
- During that process, **9 real compile errors were found and fixed**
  against the actual crate APIs, not hypothetical ones:
  - Missing `tokio` feature flags (`fs`, `io-util`, `io-std`) required by
    code that was written assuming they were enabled by default.
  - `vj_video::ipc::MpvIpc` originally tried to hold an `mpsc::Receiver`
    inside a `#[derive(Clone)]` struct — doesn't compile, receivers aren't
    cloneable. Fixed by returning `(MpvIpc, Receiver)` from `connect()`
    instead.
  - `elgato-streamdeck`'s real `set_button_image` signature takes
    `image::DynamicImage`, not the `ImageRect` type initially assumed —
    fixed in both `device.rs` and `icons.rs`.
  - `elgato_streamdeck::info::Kind` doesn't implement `Display`/`ToString`
    — code that called `.to_string()` on it to check for "xl" was
    replaced with a proper `matches!(kind, Kind::Xl | Kind::XlV2)`.
  - PipeWire's `NODE_RATE` and `TARGET_OBJECT` key constants are gated
    behind the `v0_3_33`/`v0_3_44` Cargo features respectively — not
    present by default. Added `features = ["v0_3_44"]` to `vj-audio`'s
    `pipewire` dependency.
  - `properties!` macro in `pipewire-rs` needs `Into<Vec<u8>>`, which
    `&String` doesn't satisfy directly — fixed by using `.as_str()` at
    each call site.
- The workspace produces **zero compiler warnings** on a clean build.

## NOT verified (i.e., assumed/designed but untested)

Everything involving actual hardware or a live running system:

- mpv actually rendering to a Pi 5's DRM/KMS output at all, with any flag
  combination.
- Any HEVC clip actually decoding via the V4L2-stateless hardware path
  and reaching the screen zero-copy.
- Real trigger-to-frame latency — the <50ms target is a budget derived
  from reasoning about where time goes (RESEARCH.md §1.3), not a
  measurement.
- The Stream Deck XL actually connecting, being detected as `Kind::Xl`,
  and its hidraw udev permissions actually working without root.
- Whether the button-state read loop's `is_timeout()` heuristic correctly
  identifies a real hidapi read timeout vs. an actual error on Linux —
  written from documentation/API shape, not observed behavior.
- Whether PipeWire actually delivers buffers to `vj-audio`'s `process`
  callback at all, at the configured quantum, from a real USB Audio Class
  device.
- Whether the `bytemuck_cast_slice` buffer-alignment assumption in
  `capture.rs` holds for the buffers PipeWire actually hands back (flagged
  in its own doc comment).
- Whether `scripts/setup.sh` runs cleanly start-to-finish on a real
  Raspberry Pi OS 64-bit Lite (Bookworm) install — it was written against
  documented `raspi-config`/`apt`/systemd behavior, never executed.
- Whether `scripts/encode_clip.sh`'s ffmpeg flags produce a file the Pi 5's
  hardware HEVC decoder actually accepts (the script prints a
  self-verification command using `ffmpeg -hwaccel drm` for exactly this
  reason — run it before trusting an encoded clip).

## Known-incomplete / stubbed on purpose

- **Button press/release diffing** (`vj-streamdeck/src/device.rs`) —
  currently re-fires "Pressed" on every poll tick a key reads as held
  down, rather than only on the press edge. Will misbehave (repeat
  triggers) as-is. Explicitly flagged as the top priority fix in
  `IMPLEMENTATION_PLAN.md` §2.2.
- **Icon status updates** — the code path to push an updated Stream Deck
  key icon after a clip triggers is not wired into `vj-core`'s main loop
  (`TODO` comment present at the call site).
- **Shader audio-feature delivery** — `vj-audio` computes RMS/peak/band
  features correctly in isolation but there is no implemented mechanism
  for getting them to whatever renders shaders (no shader host was
  specified or built — this needs a scoping decision, see
  `IMPLEMENTATION_PLAN.md` §2.3).
- **`vj-video/src/raw_drm.rs`** — typed skeleton, zero implementation,
  deliberately out of scope unless a measured need for it emerges.
- **Crossfade / dual-buffer playback** — not implemented; hard-cut
  triggering only, by design (RESEARCH.md §1.3).
- **Cross-compilation** (`scripts/cross-build-aarch64.sh`) — the script
  exists but explicitly documents that it needs a custom `cross` Docker
  image with an aarch64 sysroot for `pipewire`/`hidapi`/mpv's C
  dependencies, which has not been built. Native on-Pi build
  (`scripts/build.sh`) is the tested-in-principle path (in the sense that
  the same crates compile natively on x86_64 with the equivalent native
  libs — aarch64 itself hasn't been cross-checked).

## Research claims and their sourcing

All factual claims about Pi 5 hardware capabilities, codec support, and
PipeWire/Stream Deck library behavior in `docs/RESEARCH.md` are backed by
web sources cited inline in that document (community forums, GitHub
issues/discussions, crate documentation, tuning guides) — not from
training-data recall alone. The one load-bearing fact — no H.264 hardware
decode on BCM2712 — is corroborated by multiple independent sources listed
in RESEARCH.md §1.1, including Raspberry Pi community forum threads
specifically about this limitation. That said, none of it has been
cross-checked against Raspberry Pi's own official datasheet/errata for
BCM2712 if one exists with more authoritative detail — worth a direct check
if this fact is ever safety- or budget-critical to a decision.

## What to trust this handoff for

The architecture, module boundaries, and reasoning in `RESEARCH.md` and
`IMPLEMENTATION_PLAN.md` are sound engineering judgment calls, cross-checked
against real crate APIs where compilation could verify them. The code is a
correct-shaped starting point, not a tested implementation. Budget real
hardware-bring-up time — this is not a "should just work" handoff.
