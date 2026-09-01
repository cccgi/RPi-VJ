# Research Findings

## Hardware facts that drive the design

### Graphics API

Raspberry Pi 5 VideoCore VII officially supports OpenGL ES 3.1 and Vulkan 1.3. It should not be described as an OpenGL ES 3.2 target. Vulkan is the preferred long-term renderer; GLES/EGL remains a valuable fallback and prototype path.

Source: [Raspberry Pi 5 product page](https://www.raspberrypi.com/products/raspberry-pi-5/) and [processor documentation](https://www.raspberrypi.com/documentation/computers/processors.html).

### Video decoding

The Pi 5 provides a hardware HEVC decoder. Other codecs, including H.264 decoding, run in software. Raspberry Pi reports approximately 50–60% CPU usage for 1080p60 H.264 decode. Consequently:

- MP4 is only the container requirement; codec choice remains critical.
- The performance ingest profile should prefer HEVC Main, 8-bit, 4:2:0.
- H.264 must remain a compatibility path, but it cannot be assumed cheap enough for two decks plus DSP plus shaders.
- Actual GStreamer decoder output and GPU import behavior must be measured on the deployed Bookworm/Mesa image.

Source: [Raspberry Pi processor documentation](https://www.raspberrypi.com/documentation/computers/processors.html).

## Open-source project evaluation

### DatanoiseTV/audiovis

[audiovis](https://github.com/DatanoiseTV/audiovis) is the closest ARM-focused proof source. It demonstrates:

- direct DRM/KMS output through GBM/EGL;
- raw GLES rendering on small ARM boards;
- ISF parsing, multipass rendering, and persistent buffers;
- audio capture and perceptual analysis;
- layers, effects, modulation, MIDI/OSC, and web feedback.

Limitations for this product:

- its common renderer targets GLES 2 portability;
- it does not supply the required local MP4 loop/deck engine;
- it is a young project and should be treated as reference code, not an unquestioned platform dependency.

Use: borrow or prototype its DRM, ISF, modulation, and analyzer patterns.

### im-knots/varda

[Varda](https://github.com/im-knots/varda) is conceptually closest to the desired end product: Rust, Vulkan/wgpu, video sources, compositing, ISF-style shaders, effects, FFT/beat features, and headless control.

Limitations:

- published Linux artifacts are x86-64;
- its shader bodies use GLSL 450/Vulkan semantics rather than ordinary portable ISF GLSL;
- Pi 5 builds, resource use, DRM output, and decode texture interop are not established by upstream documentation;
- adopting the entire application would add features and operational surface outside this product's scope.

Use: study architecture and selectively reuse appropriately licensed code; do not assume drop-in aarch64 production readiness.

### nannou

[nannou](https://github.com/nannou-org/nannou) provides creative-coding, audio, video, ISF, and wgpu-related crates. It is valuable as an implementation reference, especially for ISF metadata and render-pass behavior. It is not the recommended application shell because this appliance needs narrow lifecycle control, deterministic resource ownership, DRM-oriented deployment, and minimal framework overhead.

### wgpu and wgpu-native

[wgpu](https://github.com/gfx-rs/wgpu) provides a safe Rust API with first-class Vulkan support on Linux. WGSL is its native shader language; GLSL and SPIR-V require optional compilation paths. Use Rust `wgpu` directly. `wgpu-native` only makes sense if a C++ host is selected.

The unresolved issue is video interop. If decoded frames cannot be imported without expensive copies using stable wgpu APIs, raw GLES/EGLImage or raw Vulkan may outperform a clean wgpu design. This must be decided by the target benchmark, not preference.

## Shader standard conclusion

Use ISF 2 as the content and metadata contract, with a documented embedded compatibility profile. Use Vulkan-compatible generated shaders internally.

Supported initial ISF profile:

- fragment generators and filters;
- `float`, `bool`, `long`, `color`, `point2D`, `event`, `image`, and `audioFFT` inputs;
- `TIME`, `TIMEDELTA`, `FRAMEINDEX`, `RENDERSIZE`, and `PASSINDEX`;
- single pass plus bounded multipass;
- persistent buffers;
- maximum four passes;
- intermediate scale restricted to 0.25, 0.5, or 1.0;
- no desktop-only extensions, unbounded loops, or runtime-sized resource creation.

ISF already defines audio and FFT inputs as image-like textures and defines multipass/persistent render behavior. Source: [ISF specification](https://github.com/mrRay/ISF_Spec) and [ISF documentation](https://docs.isf.video/).

Recommended compilation pipeline:

```text
ISF file
  -> parse JSON header
  -> validate embedded profile and resource budget
  -> inject ISF built-ins/macros and project audio bindings
  -> compile GLSL with shaderc/glslang to SPIR-V
  -> create and cache wgpu/Vulkan pipeline
```

Compilation happens during indexing, startup, or explicit administrative reload—not during a live trigger.

## Audio DSP conclusion

Capture 48 kHz stereo float audio through PipeWire, with ALSA or CPAL as a temporary adapter. The capture callback only writes fixed-size data into an SPSC ring. A dedicated analyzer thread performs:

1. channel conditioning and DC removal;
2. Hann windowing;
3. 2048-point real FFT with 256- or 512-sample hop;
4. power and log magnitude calculation;
5. perceptual/log aggregation to 64–256 shader bins;
6. bass/mid/treble and RMS extraction;
7. automatic noise-floor/upper-envelope normalization;
8. attack/release smoothing;
9. spectral-flux onset detection;
10. tempo/phase tracking with confidence.

Do not normalize each spectrum by its own maximum; that makes silence appear active. Publish raw, fast, and slow feature variants where useful.

Time-constant smoothing must be independent of callback rate:

```text
alpha = 1 - exp(-dt / tau)
output += alpha * (input - output)
```

Onset detection should use positive spectral flux against a rolling median plus median absolute deviation, with a 100–160 ms refractory interval. Beat tracking should publish BPM, phase, confidence, beat index, and onset separately.

## Stream Deck conclusion

Direct HID control is appropriate and already works in the scaffold. The open-source [`elgato-streamdeck`](https://github.com/streamduck-org/elgato-streamdeck) crate also supports Stream Deck XL and can replace or inform the local protocol implementation.

LCD updates must be cached, coalesced, and prioritized. Generate clip thumbnails during indexing and send only changed keys. Audio meters or animations should be capped around 10–15 fps so HID traffic never competes with input responsiveness.

