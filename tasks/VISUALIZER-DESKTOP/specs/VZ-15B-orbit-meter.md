# VZ-15B — Original preset: Orbit Meter

## Goal

Author a crisp radial bar and peak-dot visualizer.

## Existing files allowed

- Read one working preset/schema example selected by the orchestrator.
- Create `vj/shows/presets/orbit-meter.toml`.

## Requirements

Use 128–256 FFT bins, radial layout, rounded bars, peak hold, bass-controlled inner radius, treble-controlled rotation, onset pulse, three-stop amber/magenta/cyan gradient, and light bloom. No feedback pass is required.

Observable: low-to-high frequency order is clearly traceable around the circle, peaks fall smoothly, silence decays instead of showing a full ring, and center remains finite/artifact-free.

