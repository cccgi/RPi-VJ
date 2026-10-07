# Native release qualification

No Pi hardware run has been performed for this rewrite. No assertion of appliance-grade reliability, 1080p60, hardware decode or instant switching is supported yet.

A read-only SSH inspection of the Pi address recorded in `HANDOFF.md` was attempted during development; the connection was refused. No deployment or configuration change was made on that host.

## Current evidence and limits

- 21 portable Rust tests pass, covering DSP, control generations, page migration, IPC framing, USB packet layout and checkpoint recovery. Native Clippy passes with warnings denied.
- Linux Docker builds and links both executables. Controller dependency inspection confirms no SDL, GL or libmpv linkage. All 18 migrated GLSL shaders and the compositor pass glslang compilation/linking.
- The USB controller runs independently of the renderer, with persistent desired state, bounded Unix IPC, reconnection and progress watchdogs. Physical HID behavior still requires device tests.
- The renderer implements candidate cutover, one-next-clip warming, native shader hosting, effects, HUD, audio reconnect and actual-presentation reports. Arbitrary clips are not guaranteed instant.
- Both service units and the udev rule are templates, not proven Pi boot configuration. Neither process depends on the other's startup readiness.
- Controller readiness follows a full LCD upload; renderer readiness follows a swap. Neither proves physical display output. Driver hangs and hardware disconnects remain target tests.
- Full ISF support, arbitrary interactive shader parameter editing, and hard real-time scheduling are not claimed. The supported shader profile and existing fixed presets are implemented.

## Gate A: exact image and native first pixel

Record Pi model/RAM, power supply, active cooling, OS image/hash, kernel, Mesa, SDL build/backends, libmpv/FFmpeg versions/build options, connector EDID/mode and storage. Require a GLES 3 context through `SDL_VIDEODRIVER=kmsdrm`, 1920x1080 at 60 Hz and non-software GPU renderer. Verify DRM owner and USB permissions under the service user without an interactive desktop login. Run `systemd-analyze verify` on the installed unit. Confirm no old system or user kiosk unit owns the display/controller.

Test boot with the HDMI sink on, off, delayed and hot-plugged. No fallback to another display backend or a browser is permitted. A missing display/seat must yield actionable journal output and bounded recovery. The current SDL integration needs this first-pixel gate before deployment.

## Gate B: HEVC decoder and compositing

Use a prepared HEVC clip with motion and visible frame numbering. Require actual embedded `hwdec-current` confirmation and corroborate decoder use/CPU load with platform diagnostics. Fail if software decode, CPU pixel upload, missing EGL import or unexpected format conversion exhausts the budget. Test both decoders concurrently, including load and loop boundaries, at steady-state temperature. Record memory and render/copy costs; hardware decode does not establish zero-copy.

Test clip only, shader only/reference output, clip+shader alpha, additive and keying. Validate orientation, color/range, aspect ratio, black level, blackout and recovery. Compare GL output with reference captures; source compilation must produce real pixels. Measure p50/p95/p99/p99.9 frame times, missed vblanks, thermal flags and decoder drops. The 16.67 ms frame period is the overall budget, not an allocation for each pass.

## Gate C: controller and audio

Record key-down timestamps, dispatch, request acknowledgement, decoded-frame readiness and presentation. Test first/last keys, empty slots, rapid alternating page commands during image upload, mixed-page key presses, 1000 clip triggers, repeated same-clip retriggers and full control queues. Verify key feedback always follows the correct generation. Acceptance targets: prepared trigger p95 <=50 ms/p99 <=100 ms; page repaint p95 <=250 ms. Only the next clip is warmed; measure cold and warm triggers separately.

Unplug/replug Stream Deck repeatedly, including during an image packet. Confirm reset/repaint/input resynchronization and brightness after a cold power cycle. Verify both supported XL product IDs on available hardware; protocol reference is not a substitute for a hardware test. USB calls can still block in the OS; add worker heartbeat monitoring and bounded recovery before calling this appliance-grade.

Audio fixtures: silence, DC, 100/1000/6000 Hz tones, clicks, varying BPM kicks, clipping, noise, left-only, right-only and anti-phase stereo. Inject capture overflow and unplug/replug the audio interface. Clips must continue; stale uniform values must decay or clear. Qualify stable device selection and automatic reconnect before the final release.

## Gate D: field reliability

100 consecutive cold boots with no network, keyboard, desktop login or manual prompts. Count a success only when the HDMI output advances, the controller page is visible and a physical key launches its intended clip. Separately report audio-ready/degraded state. Target all 100 passing; a single unexplained failure blocks release.

Run an eight-hour stress show with page changes, clip loops, shader modulation, USB interruptions and real storage contention. Bound RSS and queue depth; zero unrecovered hangs. Test corrupt/missing clips, delayed storage, full disk, decoder/GPU failure, invalid shader, process kill, restart bursts and abrupt power loss. The accepted image needs a known-good recovery procedure and a rollback artifact. Reliability gates must be rerun after changing the kernel, driver, decoder, media profile or power/storage hardware.

## Delivery sequence

1. Native first pixel and embedded HEVC proof on the exact Pi image.
2. Measured clip-only switching plus decoded standby preparation.
3. Controller readiness, reconnect and navigation timing.
4. Audio reconnect and DSP calibration.
5. Shader/ISF profile and compositor qualification at 1080p60.
6. Fault injection, cold-boot campaign, long soak and pinned release image.

Do not work around a failed gate with a browser, a silent software decoder or an undocumented resolution reduction.
