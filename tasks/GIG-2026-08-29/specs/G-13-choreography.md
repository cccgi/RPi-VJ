# G-13 — Beat Choreography / Modulation Matrix
**Model:** mid · **Blocked by:** G-05, G-07

## Why
"Repetitive" is a modulation problem, not a shader problem. A shader with
static uniforms looks the same at minute 8 as at minute 1. The fix is cheap:
move the uniforms on musical time.

## Deliverable
`gig/src/choreo.js`

## Part A — tempo clock
Fill in the `bpm / beatPhase / barPhase` fields G-05 currently returns as zero.
- **Tap tempo** from the Stream Deck (key 30 per `show.json`): average the last
  4 intervals, reject outliers beyond ±30%.
- **Auto-BPM**: track inter-onset intervals from `kick`, histogram them into
  bins over a rolling 8s window, pick the mode, fold into 70–180 BPM.
- **Manual nudge** ±0.1 BPM and a phase-reset "downbeat now" key.
- Priority: manual tap > auto. Never let auto-BPM override a hand tap for at
  least 30 seconds.
- `beatPhase` = fract(beats), `barPhase` = fract(beats/4).

At 1am a DJ trusts a tap they made over an algorithm. Honour that ordering.

## Part B — modulation matrix
```js
export class Choreo {
  addMod({ source, target, depth, mode })
  // source: 'lfoSine'|'lfoTri'|'lfoSaw'|'lfoSquare'|'lfoRandom'
  //       | 'bass'|'treble'|'energy'|'kick'|'beatPhase'|'barPhase'
  // rate is in *bars*, not Hz: 1/4, 1/2, 1, 2, 4, 8, 16
  // target: any uniform name in the active shader, or a post param
  // mode: 'add' | 'multiply' | 'replace'
  apply(uniforms, features, dt)   // returns modulated uniform set
}
```
LFO rates expressed in bars and driven from `barPhase` means every movement
lands on the music for free. That is the whole trick — do not use wall-clock Hz.

## Part C — scene presets
A `scenes` block in `show.json`: shader + post params + a mod list, recallable
from one pad. This is what turns 14 shaders into a set that develops.

Ship at least four: `slow-build`, `peak-hard`, `breakdown-drift`, `strobe-hit`.

## Observable
On the **Pi** with live music: tapped BPM tracks the track within 2 BPM and
holds; an 8-bar LFO on a shader parameter visibly completes its cycle on the
downbeat; switching scene presets audibly-visibly changes the character.
Note measured vs actual BPM for three tracks in `../RESULTS.md`.
