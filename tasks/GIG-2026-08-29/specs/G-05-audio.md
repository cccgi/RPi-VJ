# G-05 — Web Audio 5-Band Analyser + Kick Detector
**Model:** cheap · **Blocks:** G-07

## Deliverable
`gig/src/audio.js` — ES module, no dependencies.

## Note before you start
`vj/crates/vj-audio/src/analyzer.rs` claims a 5-band crossover but actually
accumulates 4 time-domain buckets and *synthesises* its spectrum from a
frequency weight — there is no FFT. Do not port that part. In the browser
`AnalyserNode.getByteFrequencyData()` gives a real FFT for free. **Port the
onset logic and the envelope constants only** — those are sound and tuned.

## Constants to reuse verbatim
```
onset threshold : band > smoothed*1.35 + 0.02
refractory      : 120 ms
onset decay     : 100 ms to zero, linear
noise floor     : 0.001
envelope sub    : attack 10 ms, release 150 ms
envelope others : attack 10 ms, release 200 ms
input gain      : 0.25   (FLX10 booth output is line level and hot)
```

## API
```js
export class AudioEngine {
  async start(deviceLabelHint = 'Sound Blaster')  // getUserMedia + pick device
  stop()
  get ok()          // false if no stream — caller falls back to internal clock
  get deviceName()
  sample()          // call once per frame -> Features (below)
  reacquire()       // re-run device selection; bind to a UI button
}
```

`sample()` returns:
```js
{ subBass, bass, lowMid, highMid, treble,  // 0..1, enveloped
  energy,        // EMA of overall RMS, 0..1
  kick,          // 1.0 on onset frame, linear decay to 0 over 100ms
  bpm, beatPhase, barPhase,   // 0 / 0 / 0 for now — G-13 fills these
  ok }
```

## Band edges (Hz) — map FFT bins to these
```
subBass  20   – 75
bass     75   – 200
lowMid   200  – 1000
highMid  1000 – 5000
treble   5000 – 16000
```
Convert Hz→bin with `bin = hz / (sampleRate/2) * analyser.frequencyBinCount`.
Average the bins in each range, divide by 255, then run through the envelope.

## Device selection
1. `getUserMedia({audio:{ echoCancellation:false, noiseSuppression:false,
   autoGainControl:false, channelCount:2 }})` — **all three processing flags
   off**, they will destroy the transient response and ruin kick detection.
2. Then `enumerateDevices()`, find an `audioinput` whose label contains the
   hint (case-insensitive), and if it isn't the one already active, re-open
   with `deviceId: {exact: ...}`.
3. If anything fails, set `ok = false` and return zeros. Never throw.

## AnalyserNode settings
`fftSize = 2048`, `smoothingTimeConstant = 0` (we do our own enveloping —
the built-in smoothing is a low-pass that kills onset detection).

## Observable
A test page with a live 5-bar meter, a kick indicator that flashes, and the
selected device name. Run it **on the Pi with the Sound Blaster plugged in and
audio playing from the FLX10**. Confirm: bars move, kick flashes on kicks and
not on hats, and it survives unplugging and replugging the interface
(→ `reacquire()`). Note the actual device label string in `../RESULTS.md` —
G-15 will need it.
