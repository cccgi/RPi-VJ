# Failure handling

`403`, `429`, and navigation-destroyed errors leave the importer queue intact. It uses bounded automatic waits; rerun after the Shadertoy page is available. Do not delete `ids.json` or `incomplete_ids.json`.

Before downloading assets again, run `audit_shadertoy_assets.ps1`. A per-ID archival copy can be incomplete while the shared, deduplicated runtime asset store is valid.

Keep shaders with unsupported inputs, compiler errors, crashes, or excessive GPU time in the desktop batch and exclude them from Pi pages. The Rust benchmark measures both the reactive and legacy wrappers in one graphics session. It selects the highest resolution where both normal and loud audio passes have at least three frames and p95 GPU time at most 13 ms. The pixel check also rejects near-black or flat output. A repaired source gets a new fingerprint and must be benchmarked again. A changed fingerprint uses a new remote results directory, while a retry of the same fingerprint resumes its JSONL results.

The benchmark supervisor restores the native renderer and controller services even when a shader fails. If a run stops, inspect `target/shader-pipeline/<batch>/pi-native-results.jsonl` and rerun `-Resume <batch>`. Publishing requires a complete result for every staged variant and verifies that the live show still has its original page and shader prefix.
