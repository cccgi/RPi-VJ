# Desktop Visualizer Dispatch Prompts

Use this template, replacing `VZ-NN` and the hardware line:

```text
You are implementing one bounded task in the RPi-VJ Rust workspace. Open and follow exactly:
tasks/VISUALIZER-DESKTOP/specs/VZ-NN-*.md

Do not read any other file under tasks/ or docs/. Read only the existing source files explicitly named by the spec. Do exactly the scoped work; do not refactor adjacent modules or add deferred features.

Run every verification command in the spec that this environment supports. Hardware-only observables require the physical machine named by the spec; otherwise report NEEDS-REVIEW, not DONE.

Append a report of at most ten lines to tasks/VISUALIZER-DESKTOP/RESULTS.md. Include files changed, exact commands/results, observable, surprises, and recommended status. Do not edit BOARD.md.

Machine: [Windows RTX 3090 / development machine / other]
```

Recommended routing:

- Cheap: VZ-01, 03, 06, 11, 12, 13, 17.
- Mid: VZ-02, 04, 05, 07, 08, 09, 10, 14, 16.
- Mid creative/shader: VZ-15A, VZ-15B, VZ-15C.

