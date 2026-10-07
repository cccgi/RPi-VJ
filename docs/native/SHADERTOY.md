# Shadertoy ingestion and audio adaptation

The appliance does not load Shadertoy pages, execute JavaScript, use WebGL, or contact the site at performance time. Shadertoy is an authoring source. Import happens offline on a workstation from an official API JSON export, and the resulting GLSL file is compiled during native renderer startup.

The supplied corpus is recorded in `engine/tools/shadertoy_sources.json`. The site was protected by a Cloudflare verification page during this review, so page scraping was intentionally not used. For each ID, obtain the JSON through the official API with the account's API key, then run:

```sh
python engine/tools/import_shadertoy.py export.json \
  --name ttKGDt --output-dir engine/examples/shaders
```

The importer accepts exactly one `Image` render pass. It rejects Buffer, Common, Sound, multipass feedback, `iChannel0..3`, includes, extensions, oversized source, and other dependencies that cannot be bounded and validated for the Pi renderer. This prevents a visually impressive shader from silently becoming a runtime dependency or an unbounded GPU workload.

Shadertoy's `mainImage(out vec4, in vec2)` is wrapped into our GLSL ES 3.0 entry point. The wrapper maps `iResolution`, `iTime`, `iTimeDelta`, `iFrame`, `iDate`, and `iMouse` to deterministic native values. It then applies a controlled audio adapter:

```glsl
energy = clamp(0.72 + 0.30*u_volume + 0.22*u_bass + 0.12*u_mid, 0.35, 1.35);
pulse  = 1.0 + 0.16*u_beat_onset + 0.08*sin(u_beat_phase*6.2831853);
color.rgb *= energy*pulse;
```

The renderer supplies `u_bass`, `u_mid`, `u_treble`, `u_volume`, `u_beat_onset`, `u_beat_phase`, `u_bpm`, the 32-bin `u_spectrum` array, and smoothed flow values every frame. Imported shaders may use the scalar uniforms directly for more expressive modulation. For example, replace a time-only rotation with `float t = u_time + 0.4*u_bass + 0.15*u_beat_phase;`, or modulate ray-march step size with a clamped band value. Audio is smoothed before the GPU boundary; no shader receives raw USB/ALSA samples.

## Review gates

1. Preserve the author and license fields printed by the importer beside the local shader.
2. Compile with `glslangValidator` against the repository's fullscreen vertex shader.
3. Reject shaders with multipass feedback, texture channels, dynamic loops, unsupported extensions, or excessive instruction cost.
4. Run a 60-second audio sweep and verify that silence remains stable, bass changes are visible, and onset pulses do not flicker.
5. Measure compile time, GPU frame time, missed swaps, and thermal behavior on the Pi 5 before assigning a Stream Deck key.

Cloudflare and Shadertoy pages are not acceptable in the kiosk dependency graph. If an API export is unavailable, use a shader with a compatible local license or author a native equivalent; do not add a Chromium workaround.
