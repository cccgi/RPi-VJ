# Desktop Visualizer Board

Status: `TODO`, `WIP`, `BLOCKED(reason)`, `NEEDS-REVIEW`, `DONE`.

| Task | Status | Depends on | Primary ownership | Observable |
|---|---|---|---|---|
| VZ-01 typed visualizer contracts | TODO | — | vj-types | schema tests |
| VZ-02 real FFT analyzer | TODO | — | vj-audio | tone-bin tests |
| VZ-03 deterministic audio fixtures | TODO | VZ-02 | vj-audio tests | fixture assertions |
| VZ-04 desktop surface host | TODO | — | vj-render surface | RTX window |
| VZ-05 audio GPU texture bridge | TODO | VZ-02,VZ-04 | vj-render audio_gpu | texture readback test |
| VZ-06 frame parameter buffer | TODO | VZ-01,VZ-04 | vj-render params | layout/unit test |
| VZ-07 analytic primitive renderer | TODO | VZ-05,VZ-06 | vj-render primitives | bars/radial screenshot |
| VZ-08 spectrum history ring | TODO | VZ-05,VZ-07 | vj-render history | advancing history image |
| VZ-09 render-target pool and bloom | TODO | VZ-04,VZ-06 | vj-render post | bloom toggle + reuse counters |
| VZ-10 bounded visualizer graph | TODO | VZ-07,VZ-08,VZ-09 | vj-render visualizer | no-alloc preset cycle |
| VZ-11 show preset schema | TODO | VZ-01 | vj-types show | TOML round trip |
| VZ-12 modulation evaluator | TODO | VZ-01 | vj-core | curve/ramp tests |
| VZ-13 command/reducer lifecycle | TODO | VZ-11,VZ-12 | vj-core | ready/degraded events |
| VZ-14 desktop app integration | TODO | VZ-03,VZ-10,VZ-13 | vj-app | live audio visualizer |
| VZ-15A Neon Horizon preset | TODO | VZ-14 | preset asset | approved screenshot |
| VZ-15B Orbit Meter preset | TODO | VZ-14 | preset asset | approved screenshot |
| VZ-15C Pulse Mandala preset | TODO | VZ-14 | preset asset | approved screenshot |
| VZ-16 metrics and preset-cycle harness | TODO | VZ-15A/B/C | vj-app metrics | CSV + percentile output |
| VZ-17 workstation soak | TODO | VZ-16 | run-only | eight-hour report |

