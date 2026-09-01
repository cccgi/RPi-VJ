# RPi-VJ Implementation Handoff

This directory is the authoritative implementation handoff for the Raspberry Pi 5 VJ engine. It consolidates the research in the repository, records the selected architecture, describes the current code accurately, and turns the target design into ordered work packages.

## Reading order

1. [Current state](CURRENT_STATE.md) — what exists, what is missing, and what must be preserved.
2. [Research findings](RESEARCH_FINDINGS.md) — hardware facts, evaluated projects, and technology conclusions.
3. [Target architecture](TARGET_ARCHITECTURE.md) — component boundaries, threading, data flow, and failure behavior.
4. [Contracts](CONTRACTS.md) — Rust interfaces, command/event schemas, audio uniforms, and configuration contracts.
5. [Implementation plan](IMPLEMENTATION_PLAN.md) — phased tasks, dependencies, exit criteria, and explicit non-goals.
6. [Validation plan](VALIDATION_PLAN.md) — Pi-specific experiments and performance gates.
7. [Architecture decisions](adr/README.md) — concise decisions and trade-offs.

The older root documents remain useful background:

- [`RESEARCH.md`](../../RESEARCH.md) contains the first playback/Stream Deck/PipeWire investigation.
- [`rpi-vj-architecture.md`](../../rpi-vj-architecture.md) contains the original broad architecture report.
- [`README.md`](../../README.md) documents the current executable and deployment assumptions.

Where these disagree, this handoff directory takes precedence. Any hardware or driver conclusion still marked **validate on target** is not a promise; it is an experiment required before committing the corresponding backend.

## Product objective

Deliver a headless Raspberry Pi 5 appliance that can:

- switch local 1920x1080 60 fps MP4 loops from a Stream Deck XL;
- generate audio-reactive ISF/GLSL visuals at 60 fps;
- composite video and shader layers with alpha, additive, screen, multiply, luma key, and chroma key modes;
- expose stable parameter and state contracts to the Stream Deck and a local administration API;
- survive controller/audio disconnects without stopping HDMI output.

## Primary architectural constraint

The render path has a 16.67 ms frame budget. No disk I/O, shader compilation, image encoding, blocking IPC, device enumeration, or unbounded allocation may execute on the render thread.

