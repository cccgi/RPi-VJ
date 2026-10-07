# Pi 5 qualified shader bank

Qualification rule: the native Rust renderer produced 60 FPS with zero late
frames at both 1920x1080 and 1280x720 during the Pi benchmark. The desktop
library remains complete; the Pi deployment uses only these entries.

| Shader | 1080p | 720p |
|---|---:|---:|
| 33cGDj | 60 | 60 |
| 33cSz7 | 60 | 60 |
| 3XyXzm | 60 | 60 |
| 3csSWB | 60 | 60 |
| 3dsBWX | 60 | 60 |
| 3dtBDN | 60 | 60 |
| DlySDD | 60 | 60 |
| Mtj3Rh | 60 | 60 |
| dlKSW3 | 60 | 60 |
| dsyBDy | 60 | 60 |
| dtyczc | 60 | 60 |
| s3VGzt | 60 | 60 |
| tc3fWn | 60 | 60 |
| tlGXRz | 60 | 60 |
| tlVSRW | 60 | 60 |

The Stream Deck Pi bank is generated from this qualification set and grouped
by the existing fractal/raymarch, noise/liquid, symmetry/tunnel, and general
similarity classifiers.
