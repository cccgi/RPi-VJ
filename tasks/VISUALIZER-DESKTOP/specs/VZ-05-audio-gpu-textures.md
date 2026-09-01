# VZ-05 — FFT and waveform GPU texture bridge

## Goal

Upload the newest fixed audio snapshot to two reusable 256×1 float textures once per render frame.

## Existing files allowed

- Read `vj/crates/vj-audio/src/lib.rs`.
- Modify `vj/crates/vj-render/src/lib.rs`; create `vj/crates/vj-render/src/audio_gpu.rs` as part of this task.

## Requirements

Implement `AudioGpuTextures` with `R32Float` FFT and waveform textures, views, sampler, bind-group layout, bind group, and last uploaded sequence. Use `COPY_DST | TEXTURE_BINDING`; upload only when sequence changes. Upload exactly 256 floats per texture with correctly padded `bytes_per_row` required by wgpu. Do not allocate staging vectors per frame; own reusable padded buffers.

Return a small upload result containing whether data changed and bytes written. Silence/default snapshots must upload safely.

## Verification

Add module tests or a GPU test that uploads a ramp, copies the textures to readable buffers, and verifies first/middle/last values. Observable: unchanged sequence writes zero bytes; changed sequence writes the expected bounded amount.
