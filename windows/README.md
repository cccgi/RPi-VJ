# RPi-VJ for Windows

This is a portable x64 Windows package of the Rust desktop VJ app. It renders shaders
inside the Rust/OpenGL app; Chrome and Chromium are not part of the runtime.

## Install and launch

1. Extract the complete ZIP to a writable folder, such as Documents\RPi-VJ.
2. Open PowerShell in the extracted folder and run:
   powershell -ExecutionPolicy Bypass -File .\Setup-RPiVJ.ps1
3. The setup script checks the executable, all shader source paths and catalog asset
   files. It installs the official, Microsoft-signed Visual C++ runtime only if absent,
   then starts the app. Windows may show a UAC prompt for that runtime installation.
4. To launch again, double-click Launch-RPiVJ.bat.

The setup script uses Microsoft's current x64 Visual C++ Redistributable download and
validates its Authenticode signature before installation. It needs internet only if the
runtime is missing. Shader rendering and textures work offline after extracting the ZIP.

## Included content

The package carries the full library: 1,740 catalog shaders, all referenced shader
source files, and the complete 70-file texture/media asset store (about 64.3 MB).
Keep the folder writable so the app can store edited shaders, thumbnails and settings.

Video playback through libmpv is optional and is not included. The app supports live
audio from an input device; audio-reactive visuals need an audio input selected in the
app. A current graphics driver with OpenGL support is required for GPU shader rendering.

## Troubleshooting

If the setup script says shaders or assets are missing, re-extract the ZIP with Windows
File Explorer's "Extract All"; do not run the script from inside the ZIP preview.
If Windows blocks the PowerShell script, open PowerShell in the extracted folder and
use the command above. If the app cannot create thumbnails or edited shader files, move
the folder out of Program Files to a writable user folder.
