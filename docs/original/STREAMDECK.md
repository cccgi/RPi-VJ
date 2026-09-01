# Stream Deck XL pages and button layouts

This is the final nine-page layout defined by `gig/show.json` plus the system-row overrides in `gig/index.html`. Keys are numbered left-to-right, top-to-bottom: 0–7, 8–15, 16–23, and 24–31.

## Reading the sketches

- A label is the text shown on the key; repeated `Abstract` or `Laser` labels select distinct files despite identical short labels.
- `—` is deliberately unassigned.
- Keys 24 and 25 always navigate. The left key shows the current page number with the previous destination in its corner; the right key shows the current page name with the next destination number.
- RESET (30) and BLACK (31) never move.
- Clip/shader/effect selections are exclusive within their own type: choosing another of the same type replaces the previous selection.

## Controls common to every page

| Key | Control | Behavior |
|---:|---|---|
| 24 | previous page | Wraps backward through all nine pages |
| 25 | next page | Wraps forward through all nine pages |
| 30 | RESET | Stops/blanks both clips, clears shader, restores normal layout/speed/levels, and exits blackout |
| 31 | BLACK | Toggles program blackout |

For a controlled shutdown, hold RESET for three seconds, then press BLACK within five seconds. Releasing RESET normally performs RESET; treat the shutdown gesture as a deliberate maintenance action.

## Page 1 — 1080p

Primary high-resolution clips and core audio-reactive generators.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | Abstract | Abstract | Abstract | Abstract | Abstract | Abstract | Abstract | Abstract |
| Row 2 | Abstract | Abstract | Abstract | Laser | Laser | Laser | Laser | Laser |
| Row 3 | PLASMA | TUNNEL | KALEIDO | PARTICLE | OSCILLO | MULTIBAND | SPECTRUM | WORMHOLE |
| Row 4 | ◀ page | page ▶ | SPEED − | SPEED + | CLR CLIP | CLR FX | RESET | BLACK |

- SPEED changes all active clip decks by 1% per press, bounded to 0.25–4×.
- CLR CLIP stops and blanks clips without clearing the visual generator.
- CLR FX clears the active shader without stopping clips.

## Page 2 — Blend modes

Additional clips/generators plus compositing selection.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | Laser | Laser | Laser | Laser | Laser | WAVERINGS | BEATTUNNEL | LIQMETAL |
| Row 2 | STROBEGEO | SPECBARS | CYBERGRID | STRATA | POLYHEDRA | SHATTERED | DEEPDRIFT | 1080p |
| Row 3 | NORMAL | ADD | SCREEN | MULTIPLY | OVERLAY | SOFTLIGHT | LUMAKEY | DIFFERENC |
| Row 4 | ◀ page | page ▶ | BPM − | BPM + | TAP | HUD | RESET | BLACK |

The blend buttons change how the shader/effect layer is combined with video. BPM adjusts the internal tempo, TAP learns tempo from repeated presses, and HUD toggles on-screen diagnostics.

## Page 3 — System

Historical auxiliary clips and shader-opacity presets. The page name “System” is a placeholder retained from the original configuration; it is not a general settings menu.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | Laser | Laser | Laser | Laser | — | — | — | — |
| Row 2 | — | — | — | — | — | — | — | — |
| Row 3 | NORMAL | ADD | SCREEN | MULTIPLY | OVERLAY | SOFTLIGHT | LUMAKEY | DIFFERENC |
| Row 4 | ◀ page | page ▶ | OPACITY 25 | OPACITY 50 | OPACITY 75 | OPACITY 100 | RESET | BLACK |

Opacity presets set the shader-layer contribution without changing the selected blend mode.

## Page 4 — Video FX

Effects applied to the video/clip layer.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | INVERT | HUE | CONTRAST | B&W | POSTER | THRESH | WARM | COOL |
| Row 2 | MIRROR H | MIRROR V | QUAD | KALEIDO | ZOOM IN | ZOOM OUT | WAVE | TWIRL |
| Row 3 | PIXELATE | SCANLINE | RGB SHFT | EDGE | VIGNETTE | GRAIN | HALFTONE | STROBE |
| Row 4 | ◀ page | page ▶ | FX AMT 25 | FX AMT 50 | FX AMT 75 | FX AMT 100 | RESET | BLACK |

Selecting a video effect replaces the prior video effect; the four amount buttons control its intensity.

## Page 5 — Effect FX

The same effect palette applied to the generated shader/effect layer rather than the clip layer.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | INVERT | HUE | CONTRAST | B&W | POSTER | THRESH | WARM | COOL |
| Row 2 | MIRROR H | MIRROR V | QUAD | KALEIDO | ZOOM IN | ZOOM OUT | WAVE | TWIRL |
| Row 3 | PIXELATE | SCANLINE | RGB SHFT | EDGE | VIGNETTE | GRAIN | HALFTONE | STROBE |
| Row 4 | ◀ page | page ▶ | FX AMT 25 | FX AMT 50 | FX AMT 75 | FX AMT 100 | RESET | BLACK |

This separation permits, for example, mirroring a shader while leaving the underlying clip unchanged.

## Page 6 — Quad Mirror

Twenty-eight prepared quarter-frame clips intended for quad-mirror display.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | Q1 | Q2 | Q3 | Q4 | Q5 | Q6 | Q7 | Q8 |
| Row 2 | Q9 | Q10 | Q11 | Q12 | Q13 | Q14 | Q15 | Q16 |
| Row 3 | Q17 | Q18 | Q19 | Q20 | Q21 | Q22 | Q23 | Q24 |
| Row 4 | ◀ page | page ▶ | Q25 | Q26 | Q27 | Q28 | RESET | BLACK |

## Page 7 — 50th

A themed birthday-event bank. Keys 11–29 are intentionally open for expansion.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | 50TH SHOW | CHOCO CAKE | GOLD PARTY | BLACK GOLD | CANDLE CAKE | CELEBRATE | GOLD 50 | GOLD GLOW |
| Row 2 | BDAY CAKE | COLOR PARTY | GOLD GREET | — | — | — | — | — |
| Row 3 | — | — | — | — | — | — | — | — |
| Row 4 | ◀ page | page ▶ | — | — | — | — | RESET | BLACK |

## Page 8 — 720p

Lower-resolution clip alternatives for reduced decode/render load.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | Abstract | Abstract | Abstract | Abstract | Abstract | Abstract | Abstract | Abstract |
| Row 2 | Abstract | Abstract | Abstract | Abstract | Abstract | Laser | Laser | Laser |
| Row 3 | Laser | Laser | Laser | Laser | Laser | Laser | Laser | Laser |
| Row 4 | ◀ page | page ▶ | Laser | Laser | Laser | Laser | RESET | BLACK |

## Page 9 — Neon

Twenty-seven numbered neon clips.

| | C1 | C2 | C3 | C4 | C5 | C6 | C7 | C8 |
|---|---|---|---|---|---|---|---|---|
| Row 1 | NEON1 | NEON2 | NEON3 | NEON4 | NEON5 | NEON6 | NEON7 | NEON8 |
| Row 2 | NEON9 | NEON10 | NEON11 | NEON12 | NEON13 | NEON14 | NEON15 | NEON16 |
| Row 3 | NEON17 | NEON18 | NEON19 | NEON20 | NEON21 | NEON22 | NEON23 | NEON24 |
| Row 4 | ◀ page | page ▶ | NEON25 | NEON26 | NEON27 | — | RESET | BLACK |

## Operator notes

- Key LCDs are state feedback, not merely decoration: active controls are redrawn after selection, page change, or reconnect.
- BLACK remains available even when a page has unassigned cells.
- Page navigation wraps, so Page 1 previous goes to Page 9 and Page 9 next goes to Page 1.
- The labels describe the historical configuration. Clip content is determined by the file path in `gig/show.json`, and those media files are not distributed.
