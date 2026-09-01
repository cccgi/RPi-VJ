# G-10 — show.json → 32-Key Binding + Blackout
**Model:** cheap

## Deliverable
`gig/src/show.js` + `gig/show.json`

## show.json shape
```json
{
  "name": "Sat 29 Aug",
  "bpmDefault": 128,
  "banks": [
    { "name": "Clips",
      "pads": [ {"key":0, "type":"clip",   "src":"media/tunnel.mp4", "label":"TUNNEL"},
                {"key":1, "type":"shader", "src":"shaders/vs_cyber_wormhole.fs",
                 "label":"WORMHOLE", "blend":"add", "opacity":0.8} ]
    }
  ],
  "system": [
    {"key":24, "action":"bankPrev",  "label":"< BANK"},
    {"key":25, "action":"bankNext",  "label":"BANK >"},
    {"key":30, "action":"tapTempo",  "label":"TAP"},
    {"key":31, "action":"blackout",  "label":"BLACK"}
  ]
}
```
Unlisted keys are empty. Missing files must be reported at load, not at press.

## SURFACE-ONLY GIG — revised 2026-08-25
Saturday has **no monitor, keyboard or mouse**. Every function must be reachable
from the Stream Deck XL. Anything not on a key does not exist.

### Fixed bottom row (keys 24–31) — identical on every bank
Muscle memory at 1am matters more than flexibility.
```
24 BANK◀   25 BANK▶   26 PAGE    27 TAP
28 FX      29 STOPALL 30 SCENE   31 BLACKOUT
```
`31 BLACKOUT` is hardcoded and always works, even if `show.json` fails to parse.

### Keys 0–23: 24 content pads per bank
Banks cycle: `CLIPS → SHADERS → SCENES → SYS`

### The SYS bank (last in the cycle)
Status readouts rendered onto key icons, updated ~1/s:
```
 0 SHUTDOWN(arm)  1 fps      2 temp     3 mem      ...   7 CONFIRM
 8 RESTART KIOSK  9 audio in 10 deck    11 scale
16 REBOOT PI     ...
```
ARM at key 0 and CONFIRM at key 7 are at opposite ends of the top row **on
purpose** — no single hand reaches both. See `G-19-headless-ops.md` Part B.
Live fps/temp/memory on keys is the only telemetry Frank gets with no monitor;
it is how he knows the rig is healthy without stopping the music.

## Requirements
- Bottom row (keys 24–31) is reserved for system actions across all banks, so
  blackout is always in the same physical place. Muscle memory matters at 1am.
- **Key 31 is blackout and is hardcoded as a fallback** even if `show.json`
  fails to parse. If the JSON is broken, load an empty show with a working
  blackout rather than showing nothing.
- Generate pad icons client-side: label text on a colour-coded background
  (clip = green `#05ffa1`, shader = magenta `#ff2a6d`, system = amber), encoded
  to JPEG via canvas `toBlob`. Active pad gets an inverted/bright variant.
- Keyboard mirror: number/letter keys map to the same 32 indices, so the show
  is fully playable if the Stream Deck dies. This is the G-08 cut-list escape
  hatch — build it here, not later.

## Observable
Load a `show.json` with at least 6 clips and 8 shaders, on the **Pi**: every
pad triggers the right content, bank switching works, blackout works from both
the deck and the keyboard, and a deliberately corrupted `show.json` still
boots to a black screen with a working blackout key.
