# G-18 — X-Touch Mini via WebMIDI
**Model:** mid · **NEW, added 2026-08-25** · Required: gig is surface-only.

## Hardware facts — verify, do not assume
The Behringer X-Touch Mini has **8 rotary encoders with LED rings, 16 buttons
(2 rows of 8), ONE fader, and Layer A/B buttons.** Earlier project docs claim
"8 faders" — that is wrong and describes the X-Touch Compact. Build for one fader.

Default **Standard mode** sends CC. Expected (confirm on the device):
```
encoders   CC 16–23  (ch 0, relative or absolute per Editor config)
LED rings  CC 48–55  (send to drive the ring)
buttons    Notes 0–15
fader      CC 9
Layer B    offsets the whole map — treat as a second page
```
Frank must configure the unit in **X-Touch Editor on the PC** first and confirm
absolute (not relative) encoder mode — relative mode changes your maths.

## Deliverable
`gig/src/xtouch.js`

```js
export class XTouchMini {
  static async open()          // navigator.requestMIDIAccess({sysex:false})
  onEncoder(cb)                // cb(index 0..7, value 0..1)
  onButton(cb)                 // cb(index 0..15, pressed)
  onFader(cb)                  // cb(value 0..1)
  setRing(index, value)        // 0..1 -> CC 48+index
  get connected()
}
```

## CRITICAL: request MIDI **without** sysex
`requestMIDIAccess({ sysex: false })` avoids the permission prompt that
`sysex: true` triggers. **You cannot click a prompt at the gig — there is no
mouse.** The LED rings are driven by plain CC, so sysex is not needed. If any
code path requires sysex, redesign it.

## Hot-plug
Listen to `midiAccess.onstatechange`. Someone will kick a USB cable at some
point. Reconnect must restore ring state without a page reload.

## Suggested map (Layer A) — Frank can revise
| Control | Function |
|---|---|
| Enc 1 | master brightness |
| Enc 2 | video deck opacity |
| Enc 3 | shader opacity |
| Enc 4 | **feedback decay** (trail length) — the most performative control you have |
| Enc 5 | feedback zoom/warp |
| Enc 6 | bloom intensity |
| Enc 7 | shader param A (first entry of the active shader's `INPUTS`) |
| Enc 8 | time/speed multiplier |
| Fader | video ↔ shader crossfade |
| Btn 1–8 | scene presets |
| Btn 9–16 | FX toggles: trails, bloom, CA, strobe, invert, mirror, freeze, blackout |

Drive the LED rings from actual engine state, not from the last received value —
otherwise a scene recall leaves the rings lying about where the parameters are.

## Observable
On the Pi, headless: every encoder moves its parameter with visible effect on the
TV, rings track the values, all 16 buttons fire, the fader crossfades, and
unplug/replug recovers. Report the actual CC/note numbers observed in
`../RESULTS.md` — they may differ from the table above.
