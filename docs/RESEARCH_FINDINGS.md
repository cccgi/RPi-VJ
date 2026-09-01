# Research Findings

## Scope and confidence

These findings cover Raspberry Pi 5 media acceleration, headless display, clip triggering, Stream Deck XL HID integration, and low-latency PipeWire capture. “Verified” means supported by primary upstream documentation. “Target validation required” means the API or protocol is credible but must be confirmed against the exact Pi OS image and physical devices.

## Raspberry Pi 5 media capabilities

### Verified

- BCM2712 contains a VideoCore VII GPU and a dedicated 4K60 HEVC decoder.
- Other video codecs, including H.264, run in software on Pi 5. Raspberry Pi documents approximate H.264 CPU consumption of 10–20% for 1080p24 and 50–60% for 1080p60.
- VideoCore VII supports OpenGL ES 3.1 and Vulkan 1.3 according to current Raspberry Pi processor documentation. Design a GLES shader backend against 3.1 core unless the deployed Mesa image proves additional features.
- Bookworm uses the full KMS stack. Legacy/FKMS configuration should not be introduced.

Sources:

- <https://www.raspberrypi.com/documentation/computers/processors.html>
- <https://www.raspberrypi.com/documentation/computers/config_txt.html>
- <https://datasheets.raspberrypi.com/rpi5/raspberry-pi-5-product-brief.pdf>

### Consequences

- Standardize the performance clip library on HEVC Main, 8-bit 4:2:0 first. Main10 should only be adopted after the complete decode/import/display path is tested.
- Retain H.264 as an ingest/compatibility format or for deliberately intra-heavy clips. Budget CPU accordingly.
- Do not reserve GPU compute by assuming decode is performed by the GPU. The HEVC codec block, V3D shader cores, and display hardware are separate resources, though they share memory bandwidth and thermal/power limits.

## Video backend evaluation

### mpv/libmpv

mpv has mature demux, decode selection, timing, caching, color conversion, and direct DRM output. Its `gpu`/`gpu-next` DRM context can operate without X11 or Wayland. Its DRM PRIME integration can use hardware-decoded buffers when the driver and format negotiation support it.

Two integration forms are viable:

- **mpv subprocess plus JSON IPC:** smallest initial implementation, crash isolation, operationally easy to restart. Control-plane overhead is negligible relative to decode and vblank latency.
- **libmpv render API:** useful when mpv must render into an application-owned EGL context for shader composition. It increases EGL/FFI/lifecycle coupling and shares the application's crash domain.

Current choice: subprocess for Phase 0/1. Migrate to libmpv or GStreamer only when shader composition requires decoded textures rather than final scanout.

Source: <https://mpv.io/manual/stable/>

### GStreamer/gstreamer-rs

GStreamer exposes pipeline negotiation directly and has stateless V4L2 decoder elements that output `video/x-raw(memory:DMABuf)`. `kmssink` can scan out compatible buffers; `glupload` can import compatible DMABUFs into an EGL context.

Strengths:

- Explicit caps and buffer ownership make a zero-copy path observable.
- Better fit for multiple live pipelines, decoder pools, appsink/appsrc, and custom state machines.
- Maintained Rust bindings.

Risks:

- The actual stateless HEVC element name and availability depend on the installed GStreamer build and kernel media devices.
- DMABUF fourcc/modifier compatibility between decoder, V3D/EGL, and KMS must be negotiated on-device.
- `videoconvert`, CPU-mapped appsink, or an incompatible GL upload silently introduces copies.

Sources:

- <https://gstreamer.freedesktop.org/documentation/v4l2codecs/>
- <https://gstreamer.freedesktop.org/documentation/additional/design/dmabuf.html>

### FFmpeg/libav plus V4L2 Request and DRM atomic KMS

This path provides the greatest control but requires implementing container/demux behavior, request controls, decoded-picture-buffer management, DMABUF lifetime, explicit/implicit fences, DRM format modifiers, atomic modesetting/page flips, colorspace, error recovery, hotplug, and clocking. It is not justified until profiling proves mpv/GStreamer cannot meet a measured requirement.

Decision: do not implement this as the next step. Preserve it as a fixed-format appliance option after the higher-level paths are measured.

## Zero-copy definition and verification

Desired direct scanout flow:

```text
MP4 demux -> HEVC parser -> V4L2 stateless decoder
         -> DMABUF-backed YUV frame -> DRM plane -> HDMI
```

Desired shader-composition flow:

```text
V4L2 DMABUF -> EGLImage import -> GLES texture
             -> shader FBO/swapchain -> GBM/EGL -> DRM atomic commit
```

Verification evidence must include:

- Decoder output caps or frame descriptors showing DMABUF/DRM PRIME.
- No `-copy` hardware decoder, CPU `videoconvert`, or mapped frame in the hot path.
- DRM plane state showing expected framebuffer formats/modifiers.
- CPU profiles without per-frame memcpy/color-conversion hotspots.
- Stable page-flip timing under the target clip and shader workload.

## Trigger latency and caching

At 60 Hz, waiting for the next scanout contributes 0–16.7 ms. The remaining budget includes HID poll/report delivery, command dispatch, container open/probe, seek to a random-access point, decoder startup, reference-frame decoding, texture/plane setup, and scheduling.

Three caching levels must not be conflated:

1. **Page cache warm:** bytes are in RAM. This removes storage latency only.
2. **Demux warm:** container metadata and initial GOP are parsed/read. Decoder startup remains.
3. **Frame ready:** a decoder context is active and the first displayable frame is held as a DMABUF/EGL image. This is the only tier suitable for a strict sub-50 ms requirement.

Recommended implementation:

- Normalize clip resolution, pixel format, frame rate, codec profile, and color metadata.
- Put MP4 `moov` metadata first with `faststart`.
- Start every clip with a closed random-access point. Use a short closed GOP; all-intra improves random access but increases bitrate/storage/decode bandwidth.
- Keep 4–8 hot decoder slots, selected by explicit cueing and recent-use policy.
- Warm initial GOP bytes for the complete bank with bounded page-cache advice/read-ahead.
- Hold only the first ready frame for cold clips when possible. A 1920×1080 NV12 frame is roughly 3.0 MiB before alignment/modifier overhead; an RGBA frame is roughly 7.9 MiB.
- Timestamp HID receive, command enqueue/dequeue, decoder-ready, render-submit, and DRM page-flip completion using `CLOCK_MONOTONIC_RAW` or a consistent monotonic clock.

## Stream Deck XL findings

### Library choices

- `elgato-streamdeck` provides device abstractions and supports XL variants. It reduces protocol maintenance and is the preferred option if its exact API compiles on Bookworm.
- Raw Rust `hidapi` is appropriate for a fixed appliance and keeps the report protocol visible. The current scaffold uses this path.
- `python-streamdeck` is a valuable protocol reference/prototyping tool but adds a Python runtime and process/IPC boundary without improving the hot path.

Sources:

- <https://github.com/streamduck-org/elgato-streamdeck>
- <https://docs.rs/hidapi/latest/hidapi/struct.HidDevice.html>

### Current assumptions requiring hardware validation

- Vendor ID `0x0fd9` and product IDs `0x006c`/`0x008f` cover the intended XL revisions.
- Input report ID/header and 32 button bytes begin at the offsets used by `src/streamdeck.rs`.
- Output report `0x02/0x07`, one-based key index, 1024-byte packets, page fields, JPEG orientation, and key numbering match both target revisions.
- A 20 ms read timeout produces acceptable input latency. USB interrupt polling interval and device report rate set the true floor.

Icons should be extracted/encoded at ingest time and cached as device-ready JPEG buffers. Do not decode video, resize thumbnails, render fonts, or encode JPEG after a button press.

## PipeWire findings

- PipeWire's default clock rate is 48 kHz in standard configuration; the quantum is configurable globally and through node scheduling properties.
- 256/48000 is 5.33 ms per quantum. Total capture latency includes device/USB buffering and may exceed one quantum.
- `node.latency=256/48000` is a request. `node.force-quantum=256` or graph metadata/config can enforce the quantum while the node is active.
- A system-wide fixed min/default/max quantum reduces variability but may harm unrelated clients. This is appropriate only on a dedicated appliance image.

Sources:

- <https://pipewire.pages.freedesktop.org/pipewire/page_man_pipewire_conf_5.html>
- <https://pipewire.pages.freedesktop.org/pipewire/page_man_pipewire-props_7.html>
- <https://pipewire.pages.freedesktop.org/pipewire/page_man_pipewire_1.html>

The capture callback must copy or reference data into a bounded preallocated SPSC structure, publish lightweight meters atomically, and return immediately. FFT, smoothing, beat detection, logging, and rendering belong on non-RT threads.

## Operational findings

- Use a quality 5 A supply and active cooling. CPU/GPU throttling causes nondeterministic frame timing.
- Discover DRM connector/card and PipeWire node identities at install/startup; do not assume stable numeric IDs.
- Choose either a user PipeWire session or a deliberately configured system instance. A root/system service cannot automatically see an arbitrary user's PipeWire socket.
- Service restart must clean up child mpv processes and release DRM master.
- Device reconnect is normal behavior, not an exceptional shutdown condition.

