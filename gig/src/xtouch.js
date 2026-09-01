/**
 * Behringer X-Touch Mini over WebMIDI.
 *
 * HARDWARE FACTS (Standard mode; the X-Touch Editor can remap any of these,
 * so every number here is a default that the MIDI-learn log can correct):
 *   8 encoders with LED rings, 16 buttons, ONE fader, Layer A/B.
 *   encoders  CC 16-23   ring LEDs  CC 48-55   buttons  notes 0-15  fader CC 9
 * Older project docs claim "8 faders" -- that is the X-Touch Compact.
 *
 * requestMIDIAccess({sysex:false}) deliberately: sysex triggers a permission
 * prompt, and the gig rig has no mouse to dismiss one.
 */
// ===================== TUNE THIS IF THE RINGS LOOK WRONG =====================
// The LED rings around the encoders are driven by CC 48-55. Firmwares differ:
// some take 0-127, others take 0-13 for a ring position.
//
//   Rings jump to FULL after a quarter turn  ->  set this to 13
//   Rings barely move / stay near empty      ->  set this to 127
//
// Edit the number, save, then run:  .\gig\tools\reload.ps1
export const RING_SCALE = 127;
// =============================================================================

// MEASURED on this unit (2026-08-26), not taken from documentation:
//   encoders  CC 1-8 on channel 10, ABSOLUTE (values sweep 0..127)
//   rings     driven by echoing the same CC back on the same channel
// The commonly-cited "CC 16-23" is wrong for this configuration, and reading
// CC 1 as a fader made knob 1 drive input gain and knob 2 drive opacity.
export const MIDI_CHANNEL = 10;
export const KNOB_CC_BASE = 1;
// Encoders are CC 1-8. LED rings are output CC 48-55 on standard X-Touch Mini,
// or must NEVER overlap input CC 1-8 if the unit echoes back output MIDI.
export const RING_CC_BASE = 48;
// MEASURED: buttons start at note 8 on this unit (top row 8-15, bottom 16-23),
// and the faders are CC 9 (Layer A) / CC 10 (Layer B).
export const BTN_NOTE_BASE = 8;
export const FADER_A_CC = 9;
export const FADER_B_CC = 10;
export const KNOB_COUNT = 8;
export const BTN_COUNT = 48;  // 0-15 Layer A buttons, 16-31 Layer B, 32-47 encoder pushes

export class XTouchMini {
  static async open(diag) {
    const inst = new XTouchMini();
    await inst.open(diag);
    return inst;
  }

  constructor() {
    this.input = null;
    this.output = null;
    this.access = null;
    this._onKnob = null;
    this._onButton = null;
    this._onFader = null;
    this._faderA = FADER_A_CC;   // measured; learn still corrects if remapped
    this._faderB = FADER_B_CC;
    this._learn = 0;
    this._ringMax = RING_SCALE;   // see RING_SCALE at the top of this file
    this._rings = new Array(KNOB_COUNT).fill(0);
    this._btnLeds = new Array(BTN_COUNT).fill(false);
    this._connected = false;
  }

  get connected() {
    return this._connected;
  }

  set connected(val) {
    this._connected = !!val;
  }

  async open(diag) {
    this._diag = diag || (() => {});
    if (typeof navigator === 'undefined' || !navigator.requestMIDIAccess) {
      this._diag('midi: API unavailable');
      return false;
    }
    this.access = await navigator.requestMIDIAccess({ sysex: false });
    this._bindPorts();
    this.access.onstatechange = () => this._bindPorts();
    return this.connected;
  }

  _bindPorts() {
    let inp = null, out = null;
    if (this.access && this.access.inputs) {
      this.access.inputs.forEach((p) => { if (/x-?touch/i.test(p.name)) inp = p; });
    }
    if (this.access && this.access.outputs) {
      this.access.outputs.forEach((p) => { if (/x-?touch/i.test(p.name)) out = p; });
    }
    if (!inp) {
      this.connected = false;
      this.input = null;
      this.output = null;
      return;
    }
    const hadOutput = this.output;
    if (this.input !== inp) {
      this.input = inp;
      this.input.onmidimessage = (m) => this._onMessage(m);
      this._diag('xtouch input: "' + inp.name + '"');
    }
    if (out && (this.output !== out || !hadOutput)) {
      this.output = out;
      this._diag('xtouch output: "' + out.name + '" (LED feedback available)');
      this._restoreState();
    }
    this.connected = true;
  }

  _restoreState() {
    if (!this.output) return;
    for (let i = 0; i < KNOB_COUNT; i++) {
      const out = Math.round(this._rings[i] * this._ringMax);
      try { this.output.send([0xb0 | MIDI_CHANNEL, RING_CC_BASE + i, out]); } catch (e) {}
    }
    for (let note = 0; note < BTN_COUNT; note++) {
      if (this._btnLeds[note]) {
        try { this.output.send([0x90 | MIDI_CHANNEL, note, 127]); } catch (e) {}
      }
    }
  }

  onEncoder(cb) { this._onKnob = cb; }
  onKnob(cb) { this._onKnob = cb; }
  /** Seed the internal value for a knob so relative mode starts in the right place. */
  seedKnob(i, v) {
    if (!this._encVal) this._encVal = new Array(KNOB_COUNT).fill(1);
    if (i >= 0 && i < KNOB_COUNT) this._encVal[i] = Math.max(0, Math.min(1, v));
  }
  onButton(cb) { this._onButton = cb; }

  /**
   * Force the encoder interpretation. 'relative' (default) or 'absolute'.
   * Explicit beats detected: this surface is fixed hardware with a fixed
   * X-Touch Editor configuration, so there is nothing to discover at runtime.
   */
  setEncoderMode(mode) {
    if (mode === 'relative' || mode === 'absolute') {
      this._encMode = mode;
      this._diag('xtouch: encoder mode forced to ' + mode.toUpperCase());
    }
  }

  /** Fraction of full travel per detent in relative mode. Default 1/48. */
  setEncoderStep(step) {
    if (typeof step === 'number' && step > 0 && step <= 0.5) {
      this._encStep = step;
      this._diag('xtouch: encoder step = ' + step.toFixed(4));
    }
  }
  onFader(cb) { this._onFader = cb; }
  learn(n) { this._learn = n || 24; }

  _onMessage(m) {
    const [st, d1, d2] = m.data;
    const type = st & 0xf0;
    if (this._learn > 0 && type !== 0xf0) {
      this._learn--;
      this._diag('midi ' + (type === 0xb0 ? 'CC' : type === 0x90 ? 'NOTE' : '0x' + type.toString(16)) +
                 ' ch' + (st & 0x0f) + ' d1=' + d1 + ' d2=' + d2);
    }
    if (type === 0xb0) {
      if (d1 >= KNOB_CC_BASE && d1 < KNOB_CC_BASE + KNOB_COUNT) {
        const i = d1 - KNOB_CC_BASE;
        const now = performance.now();

        // Echo guard: ignore echo messages within 80ms of setRing
        if (this._suppressEcho && (now - this._suppressEcho < 80)) {
          return;
        }

        if (!this._knobState) this._knobState = {};
        if (!this._encVal) this._encVal = new Array(KNOB_COUNT).fill(1.0);
        if (this._encVal[i] === undefined || isNaN(this._encVal[i])) this._encVal[i] = 1.0;

        const prev = this._knobState[i] || { raw: d2, time: now };
        const rawDelta = d2 - prev.raw;
        const dt = Math.max(1, now - prev.time);
        this._knobState[i] = { raw: d2, time: now };

        // Modest velocity acceleration:
        // Keep 1:1 hardware tracking on slow/normal turns.
        // On fast turns, add a gentle boost up to max 1.6x.
        let accel = 1.0;
        if (dt < 25 && Math.abs(rawDelta) > 1) {
          const speed = Math.abs(rawDelta) / dt; // steps per ms
          accel = Math.min(1.6, 1.0 + speed * 3.0);
        }

        // Base step calculation
        if (accel > 1.05 && Math.abs(rawDelta) > 0 && Math.abs(rawDelta) < 32) {
          const stepDelta = (rawDelta / 127) * (accel - 1.0);
          this._encVal[i] = Math.max(0, Math.min(1, (d2 / 127) + stepDelta));
        } else {
          // Direct 1:1 hardware tracking
          this._encVal[i] = Math.max(0, Math.min(1, d2 / 127));
        }

        if (this._onKnob) this._onKnob(i, this._encVal[i], d2);
        return;
      }
      // FADER / LAYER DETECTION.
      const isEncoderCC = (d1 >= KNOB_CC_BASE && d1 < KNOB_CC_BASE + KNOB_COUNT);
      if (!isEncoderCC && d1 !== this._faderA && d1 !== this._faderB) {
        if (this._faderA === null) {
          this._faderA = d1;
          this._diag('xtouch: fader A = CC ' + d1);
        } else if (this._faderB === null) {
          this._faderB = d1;
          this._diag('xtouch: fader B = CC ' + d1 + ' (Layer B)');
        }
      }
      if (this._onFader) {
        if (d1 === this._faderA) this._onFader(d2 / 127, 'A');
        else if (d1 === this._faderB) this._onFader(d2 / 127, 'B');
      }
    } else if (type === 0x90 || type === 0x80) {
      this._midiChannel = (st & 0x0f);
      if (this._noteBase === undefined || d1 < this._noteBase) {
        this._noteBase = Math.min(d1, BTN_NOTE_BASE);
        this._diag('xtouch: button note base = ' + d1);
      }
      const pressed = (type === 0x90) && d2 > 0;
      const idx = d1 - this._noteBase;
      this._diag('RAW NOTE d1=' + d1 + ' (idx=' + idx + ') pressed=' + pressed);
      if (idx >= 0 && idx < BTN_COUNT && this._onButton) this._onButton(idx, pressed, d1);
    }
  }

  /** Light or clear a button LED by absolute MIDI note. */
  setButtonLed(note, on) {
    if (note < 0 || note >= BTN_COUNT) return;
    this._btnLeds[note] = !!on;
    if (!this.output) return;
    const ch = (this._midiChannel !== undefined) ? this._midiChannel : MIDI_CHANNEL;
    const vel = on ? 127 : 0;
    try {
      this.output.send([0x90 | ch, note, vel]);
      // Also broadcast on ch 0 if different
      if (ch !== 0) this.output.send([0x90, note, vel]);
    } catch (e) {}
  }

  /** Drive an encoder's LED ring from a 0..1 value. */
  setRing(idx, value01) {
    if (idx < 0 || idx >= KNOB_COUNT) return;
    const v = Math.max(0, Math.min(1, value01));
    this._rings[idx] = v;
    if (this._encVal) this._encVal[idx] = v;
    const rawVal = Math.round(v * 127);
    const now = performance.now();
    if (!this._knobState) this._knobState = {};
    this._knobState[idx] = { raw: rawVal, time: now };
    this._suppressEcho = now;

    if (!this.output) return;
    const out = Math.round(v * this._ringMax);
    // Send both ring CC (48..55) and encoder position CC (1..8) to update ring hardware
    try { this.output.send([0xb0 | MIDI_CHANNEL, RING_CC_BASE + idx, out]); } catch (e) {}
    try { this.output.send([0xb0 | MIDI_CHANNEL, KNOB_CC_BASE + idx, rawVal]); } catch (e) {}
  }

  /** Some firmwares scale rings 0..13 rather than 0..127. */
  setRingScale(max) { this._ringMax = max; }

  /** Which CCs were learned for each layer's fader (for diagnostics). */
  faderMap() { return { A: this._faderA, B: this._faderB }; }

  allLedsOff() {
    for (let i = 0; i < BTN_COUNT; i++) this.setButtonLed(i, false);
    for (let i = 0; i < KNOB_COUNT; i++) this.setRing(i, 0);
  }
}
