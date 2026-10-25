---
name: shadertoy-pi-pipeline
description: Import Shadertoy shaders from a profile or supplied ID list, qualify them on Raspberry Pi 5, and publish only measured shaders to native Stream Deck review pages.
metadata:
  short-description: Curate and publish native Pi shader batches
---

# Shadertoy Pi Pipeline

Use this skill when adding a curated Shadertoy set to the native RPi-VJ shader library. The Raspberry Pi appliance remains a Rust/SDL/GLES program: Edge is a desktop-only importer and never ships to or renders the appliance.

## Workflow

Run `engine/tools/run_shader_pipeline.ps1` with no arguments for prompts, or
give `-Profile`, `-Url`, or `-File`. The one entry point downloads sources,
converts the Image pass to native shaders, stages required image assets,
benchmarks on the idle Pi, chooses the highest passing 60 FPS render tier,
and appends new Stream Deck pages. A failed phase leaves resumable state under
`target/shader-pipeline/<name>/`; pass `-Resume <name>` to retry.

The live Pi show is downloaded again after benchmarking. Existing pages and
shaders are preserved, the remote show hash is checked before installation,
and a dated backup is kept for rollback. A shader whose input cannot be bound
or whose benchmark fails stays in the desktop batch exclusion reports.

Read [the command reference](references/commands.md) for exact commands and state locations. Read [failure handling](references/failures.md) for transient download and benchmark failures.

## Operating constraints

- A downloaded shader is not performance-qualified. Conversion with its actual texture bindings and Pi benchmark results are the gates.
- Keep raw archive completeness separate from runtime asset completeness. Run `engine/tools/audit_shadertoy_assets.ps1` before re-fetching assets.
- Preserve the full desktop library and all existing Pi pages. Append only the measured subset.
- Download retries use bounded browser pacing and preserve the queue; they do not attempt to bypass access controls.
