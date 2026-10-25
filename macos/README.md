# RPi-VJ for macOS

The Mac port is a separate Rust app in macos/app. It keeps the existing desktop source
tree unchanged and renders the original GLSL shaders inside the Rust app; Chrome and
Chromium are not used for rendering.

## Build

Extract this package on a Mac. Open Terminal in the RPi-VJ-Mac-Port folder and run:

    bash macos/setup-macos.sh

The setup script checks for Apple's Command Line Tools, installs stable Rust with the
official rustup installer if needed, fetches crate dependencies, selects the native
Apple Silicon or Intel target, and builds target/mac-app/RPi-VJ.app. If macOS opens an
Apple tools installer, wait for it to finish and run the setup script again.

## Included offline library

The package includes the catalog, every shader source path in that catalog, and the
complete shared asset store: 1,740 shaders and 70 asset files (about 64.3 MB). The app
copies the library to ~/Library/Application Support/RPi-VJ/library the first time it
runs, so edits and future downloads are writable. Desktop state is stored in the
Application Support/RPi-VJ folder. RPI_VJ_ROOT can override the library during development.

## Device support

Select the USB interface as an audio input for live line-in analysis. macOS does not
offer speaker output as a standard capture source; install a virtual capture input such
as BlackHole to analyze system playback. MIDI uses CoreMIDI.

Shader rendering and still images do not require libmpv. Video layers require a
compatible libmpv dylib, which may be pointed to with RPI_VJ_MPV. Video playback is
optional.

## Build-host note

This ZIP contains portable Rust source and shader assets, not a precompiled Mac binary.
The .app must be built on macOS with Apple's SDK. The build could not be launched or
verified on the Windows host that prepared this package.
