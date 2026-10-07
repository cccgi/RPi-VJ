# RPi-VJ: native Rust / libmpv rewrite

This branch, `codex/native-mpv-core`, replaces the performance runtime with a fresh Rust engine. The root Cargo workspace builds only `engine/`. Chrome/Chromium, browser profiles, WebHID, Web Audio and HTTP servers are not dependencies or fallbacks.

**Development baseline, not an appliance-qualified release.** Native rendering and device adapters are implemented, but Pi hardware testing and measured release gates remain essential. No existing Pi deployment has been changed.

- [Build, controls and native deployment scaffold](engine/README.md)
- [Research, architecture, DSP, module map and API contracts](docs/native/ARCHITECTURE.md)
- [Hardware qualification and outstanding work](docs/native/QUALIFICATION.md)
- [Offline HEVC preparation tool](engine/tools/prepare_media.py)

The runtime embeds two libmpv decoders, hosts GLES shaders and blending in Rust, and uses an independently supervised native USB controller plus audio/DSP workers. HEVC is the prepared-media format. Hardware-decoder confirmation gates clip promotion by default.

```sh
cargo test --locked --lib
cargo check --locked --features native
cargo build --release --locked --features native
```

The native build needs SDL2, libmpv and platform HID libraries; see `engine/README.md`. The engine loads candidates on demand and warms one next clip and preserves the active output while loading. It does not yet guarantee instant switching or appliance-grade boot reliability.

Historical `src/`, `native/`, `vj/`, `gig/`, `scripts/`, `config/`, `tasks/` and older docs remain as migration/reference material. They are outside the new root runtime/build path. Do not run their deployment scripts for this branch. The current service templates are `engine/deploy/rpi-vj-native.service` and `engine/deploy/rpi-vj-controller.service`.
