// gig/src/audio.js
// Web Audio 5-Band Analyser + Kick Detector (G-05) + Tempo Clock (G-13)

import { TempoClock } from './choreo.js';

export class AudioEngine {
  constructor() {
    this._audioCtx = null;
    this._analyser = null;
    this._gainNode = null;
    this._mediaStream = null;
    this._sourceNode = null;
    this._freqData = null;

    this._deviceName = '';
    this._ok = false;
    this._lastDeviceLabelHint = 'Sound Blaster';

    // Envelope followers (tau in seconds)
    // sub: attack 10ms (0.010s), release 150ms (0.150s)
    // others: attack 10ms (0.010s), release 200ms (0.200s)
    this._envSub = { value: 0.0, tauAttack: 0.010, tauRelease: 0.150 };
    this._envBass = { value: 0.0, tauAttack: 0.010, tauRelease: 0.200 };
    this._envLowMid = { value: 0.0, tauAttack: 0.010, tauRelease: 0.200 };
    this._envHighMid = { value: 0.0, tauAttack: 0.010, tauRelease: 0.200 };
    this._envTreble = { value: 0.0, tauAttack: 0.010, tauRelease: 0.200 };
    this._envEnergy = { value: 0.0, tauAttack: 0.010, tauRelease: 0.200 };

    // Onset / Kick detection state
    this._lastOnsetSec = -1000.0;
    this._onsetDecay = 0.0;
    this._lastSampleSec = null;

    // Tempo clock (G-13)
    this.clock = new TempoClock();
  }

  get ok() {
    return this._ok;
  }

  get deviceName() {
    return this._deviceName;
  }

  /**
   * Start audio capture and analysis.
   * @param {string} [deviceLabelHint='Sound Blaster']
   * @returns {Promise<boolean>}
   */
  async start(deviceLabelHint = 'Sound Blaster') {
    this._lastDeviceLabelHint = deviceLabelHint;
    return await this._initStream(deviceLabelHint);
  }

  /**
   * Re-run device selection.
   * @returns {Promise<boolean>}
   */
  async reacquire() {
    return await this._initStream(this._lastDeviceLabelHint);
  }

  /**
   * Stop audio capture and release resources.
   */
  stop() {
    this._ok = false;
    if (this._mediaStream) {
      try {
        const tracks = this._mediaStream.getTracks();
        for (const track of tracks) {
          track.stop();
        }
      } catch (_) {}
      this._mediaStream = null;
    }
    if (this._sourceNode) {
      try {
        this._sourceNode.disconnect();
      } catch (_) {}
      this._sourceNode = null;
    }
    if (this._gainNode) {
      try {
        this._gainNode.disconnect();
      } catch (_) {}
      this._gainNode = null;
    }
    if (this._analyser) {
      try {
        this._analyser.disconnect();
      } catch (_) {}
      this._analyser = null;
    }
    if (this._audioCtx) {
      try {
        this._audioCtx.close();
      } catch (_) {}
      this._audioCtx = null;
    }
    this._freqData = null;
    this._deviceName = '';
  }

  /**
   * Sample audio features for current frame.
   * @returns {{ subBass: number, bass: number, lowMid: number, highMid: number, treble: number, energy: number, kick: number, bpm: number, beatPhase: number, barPhase: number, ok: boolean }}
   */
  sample() {
    const nowSec = performance.now() * 0.001;
    const dt = this._lastSampleSec === null ? 0.016 : Math.max(0.0001, Math.min(0.1, nowSec - this._lastSampleSec));
    this._lastSampleSec = nowSec;

    if (!this._ok || !this._analyser || !this._audioCtx || !this._freqData) {
      // Linear decay of kick even when not ok
      this._onsetDecay = Math.max(0.0, this._onsetDecay - dt / 0.100);
      const clockState = this.clock.update(dt, nowSec);
      return {
        subBass: 0,
        bass: 0,
        lowMid: 0,
        highMid: 0,
        treble: 0,
        energy: 0,
        kick: 0,
        hitLow: 0,
        hitMid: 0,
        hitHigh: 0,
        bpm: clockState.bpm,
        beatPhase: clockState.beatPhase,
        barPhase: clockState.barPhase,
        locked: clockState.locked,
        ok: false,
      };
    }

    // Resume AudioContext if suspended (e.g. browser autoplay policies)
    if (this._audioCtx.state === 'suspended') {
      this._audioCtx.resume().catch(() => {});
    }

    this._analyser.getByteFrequencyData(this._freqData);

    const sampleRate = this._audioCtx.sampleRate;
    const binCount = this._analyser.frequencyBinCount;
    const hzToBin = (hz) => (hz / (sampleRate / 2)) * binCount;

    const subBassRaw = this._bandLevel(hzToBin(20), hzToBin(75));
    const bassRaw = this._bandLevel(hzToBin(75), hzToBin(200));
    const lowMidRaw = this._bandLevel(hzToBin(200), hzToBin(1000));
    const highMidRaw = this._bandLevel(hzToBin(1000), hzToBin(5000));
    const trebleRaw = this._bandLevel(hzToBin(5000), hzToBin(16000));

    // ---- PER-BAND ADAPTIVE NORMALISATION -------------------------------
    // Music has a roughly pink spectrum: raw energy at 8kHz is a tiny fraction
    // of energy at 60Hz. Feeding raw band values to shaders means u_bass swings
    // 0->0.9 while u_treble shuffles around 0.03, so EVERYTHING looks
    // bass-driven even though the upper bands are moving. Each band gets its
    // own slowly-decaying running peak and is scaled against it, so every band
    // uses the full 0..1 range and can drive a visual layer on its own.
    if (!this._peaks) this._peaks = { sub: 0.15, bass: 0.15, lowMid: 0.1, highMid: 0.07, treble: 0.05 };
    if (!this._noise) this._noise = this._loadNoiseFloor();

    const rawByKey = { sub: subBassRaw, bass: bassRaw, lowMid: lowMidRaw,
                       highMid: highMidRaw, treble: trebleRaw };

    // ---- NOISE FLOOR CALIBRATION ---------------------------------------
    // An unplugged 3.5mm input still carries hiss from the USB bus. Worse,
    // normalising against a decaying peak AMPLIFIES that hiss: on silence the
    // reference falls to its floor and noise divides up to half scale. Gate
    // against a measured floor before normalising, or the visuals dance to
    // nothing.
    if (this._calUntil && nowSec < this._calUntil) {
      for (const k in rawByKey) {
        this._calPeak[k] = Math.max(this._calPeak[k] || 0, rawByKey[k]);
        this._calSum[k] = (this._calSum[k] || 0) + rawByKey[k];
      }
      this._calSamples++;
    } else if (this._calUntil && nowSec >= this._calUntil) {
      this._noise = {};
      const n = Math.max(1, this._calSamples);
      for (const k in this._calPeak) {
        const peak = this._calPeak[k];
        const mean = (this._calSum[k] || 0) / n;
        // Peak captures worst case; blend a little toward the mean so a single
        // stray spike during calibration cannot deafen the band all night.
        this._noise[k] = Math.max(peak * 0.85 + mean * 0.15, 0.004);
      }
      this._noise.__gain = this.getGain ? this.getGain() : 0.25;   // gain at calibration time
      this._saveNoiseFloor(this._noise);
      this._calUntil = 0;
      if (typeof window !== 'undefined' && window.VJ_DIAG) {
        window.VJ_DIAG('noise floor calibrated over ' + this._calSamples + ' samples: ' +
          Object.keys(this._noise).map(k => k + '=' + this._noise[k].toFixed(4)).join(' '));
      }
    }

    // MEASURED on this rig (2026-08-25, gain x0.25, cable in, nothing playing):
    //   sub 0.038-0.052, bass 0.014-0.020, lowMid/highMid/treble EXACTLY 0.
    // The noise is narrow-band mains hum in the sub range; time-domain peak sits
    // at 1 LSB. Calibration stores the PEAK, so a large extra margin just makes
    // the gate deaf to quiet passages. 1.25x clears measured noise with headroom.
    const GATE_MARGIN = 1.25;

    // ---- FIXED CALIBRATED RANGE (replaces the adaptive AGC) -------------
    // The old per-band AGC divided each band by its own running peak, so
    // raising the input gain raised the reference too and the two cancelled --
    // the fader appeared to do nothing. Worse, it flattened dynamics: a quiet
    // passage was boosted to look as loud as a drop.
    //
    // Instead map each band linearly between two MEASURED anchors:
    //   floor   = noise, measured with nothing playing
    //   ceiling = the loudest you will actually play, measured at the FLX10's
    //             clipping threshold
    // Anything above the ceiling is soft-kneed rather than hard clipped.
    const curGain = this.getGain ? this.getGain() : 0.25;

    // Hard mute: fader at the bottom means silence, not -34dB.
    if (curGain <= 0.0001) {
      this._onsetDecay = 0;
      const cs0 = this.clock.update(dt, nowSec);
      return { subBass: 0, bass: 0, lowMid: 0, highMid: 0, treble: 0,
               energy: 0, kick: 0, hitLow: 0, hitMid: 0, hitHigh: 0,
               bpm: cs0.bpm, beatPhase: cs0.beatPhase, barPhase: cs0.barPhase, ok: true };
    }

    const calGain = (this._noise && this._noise.__gain) ? this._noise.__gain : curGain;
    const gainRatio = calGain > 0.0001 ? (curGain / calGain) : 1.0;

    if (!this._maxRef) this._maxRef = this._loadMaxRef();
    // The ceiling is FIXED, deliberately. Scaling it by the current gain (as an
    // earlier version did) made numerator and denominator move together, so the
    // bands came out gain-invariant and the fader did nothing to the effects --
    // the same cancellation the old AGC caused. The fader's job is to scale the
    // signal against a fixed reference.

    // ---- MAX CALIBRATION -------------------------------------------------
    if (this._maxUntil && nowSec < this._maxUntil) {
      for (const k in rawByKey) this._maxPeak[k] = Math.max(this._maxPeak[k] || 0, rawByKey[k]);
      this._maxSamples++;
    } else if (this._maxUntil && nowSec >= this._maxUntil) {
      this._maxRef = {};
      for (const k in this._maxPeak) this._maxRef[k] = Math.max(this._maxPeak[k], 0.02);
      this._maxRef.__gain = curGain;
      this._saveMaxRef(this._maxRef);
      this._maxUntil = 0;
      if (typeof window !== 'undefined' && window.VJ_DIAG)
        window.VJ_DIAG('MAX reference set over ' + this._maxSamples + ' samples: ' +
          Object.keys(this._maxRef).map(k => k + '=' + Number(this._maxRef[k]).toFixed(4)).join(' '));
    }

    // Soft knee: ease into the ceiling instead of slamming into it, so an
    // over-loud moment compresses rather than pinning every band to 1.0.
    const softKnee = (x) => (x <= 0.85 ? x : 0.85 + 0.15 * Math.tanh((x - 0.85) / 0.15));
    // NOTE: `raw` here is already a dB reading (0..1 across minDecibels..
    // maxDecibels = -60..0 dB). Applying 20*log10() to it was a DOUBLE
    // logarithm, which compressed every level into out=0.95..0.98. Map it
    // linearly -- linear in this space IS linear in dB.
    const dB = (v) => (v * 60) - 60;   // 0..1 -> -60..0 dB, for logging only

    // Map in the dB domain, not linear amplitude. Linear amplitude sits near
    // zero most of the time and then spikes, which reads as "nothing, nothing,
    // MAX". dB spreads the musical range evenly and matches the IN meter, so
    // the bars and the effects respond the way the meter looks.
    // The noise gate and the mapping window are DIFFERENT jobs and must not
    // share a number. Using the noise floor as the bottom of the range gave a
    // window of only ~20dB (floor 0.06 -> ceiling 0.6), while the fader spans
    // 40dB -- so ~15% of fader travel consumed the whole range and the bands
    // slammed from nothing to full.
    //
    //   gate   : reject anything at or below measured noise (hard zero)
    //   window : a FIXED musical dynamic range below the calibrated ceiling
    const BAND_RANGE_DB = this.getBandRangeDb();

    if (!this._dbg) this._dbg = {};
    const norm = (key, raw) => {
      const floorBase = (this._noise && this._noise[key] != null) ? this._noise[key] : 0.010;
      const gateAt = floorBase * gainRatio * GATE_MARGIN;   // noise tracks gain
      const ceilDbg = (this._maxRef && this._maxRef[key] != null) ? this._maxRef[key] : 0.45;
      this._dbg[key] = { raw: raw, gate: gateAt, ceil: ceilDbg,
                         hi: dB(ceilDbg),
                         lo: dB(Math.max(0, ceilDbg - BAND_RANGE_DB / 60)),
                         rawDb: dB(raw) };
      // Track this band's slow peak BEFORE the gate. It used to be computed
      // after the early return, so a band that was gated could never build a
      // peak, and therefore could never un-gate: treble logged exactly 0.000
      // on every line because the Sound Blaster's hiss sets a treble noise
      // floor comparable to real hats.
      if (!this._bandPeak) this._bandPeak = {};
      const pk = Math.max(raw, (this._bandPeak[key] || 0) * Math.pow(0.5, dt / 8.0));
      this._bandPeak[key] = pk;

      // Gate on the measured floor, but let a band through when it is clearly
      // loud relative to its own recent peak -- otherwise a hissy input mutes
      // an entire frequency range for the whole night.
      if (raw <= gateAt && raw < pk * 0.35) {
        this._dbg[key].out = 0; this._dbg[key].why = 'GATED'; return 0;
      }

      // ADAPTIVE CEILING. treb=0.000 and himid=0.005 on EVERY log line: the
      // calibrated ceiling was measured on bass-heavy material, so the window
      // [ceil-36dB .. ceil] sat entirely above where hats and mids actually
      // live, and those bands returned BELOW-WINDOW forever. Each band now
      // also tracks its own slow peak (8s half-life) and the window anchors to
      // whichever is higher, so no band can be permanently dead while the
      // calibrated value still sets the floor of the range.
      const ceilCal = (this._maxRef && this._maxRef[key] != null) ? this._maxRef[key] : 0.80;
      const ceil = Math.max(pk, ceilCal * 0.35);
      // Window is BAND_RANGE_DB wide, expressed in this 0..1 dB space
      // (1.0 span == 60 dB), anchored under the calibrated ceiling.
      const span = Math.max(0.05, BAND_RANGE_DB / 60);
      const lo = Math.max(0, ceil - span);
      const v = (raw - lo) / Math.max(0.02, ceil - lo);
      if (v <= 0) { this._dbg[key].out = 0; this._dbg[key].why = 'BELOW-WINDOW'; return 0; }
      const outv = Math.min(1, softKnee(v));
      this._dbg[key].out = outv; this._dbg[key].why = 'ok';
      return outv;
    };
    const subN     = norm('sub',     subBassRaw);
    const bassN    = norm('bass',    bassRaw);
    const lowMidN  = norm('lowMid',  lowMidRaw);
    const highMidN = norm('highMid', highMidRaw);
    const trebleN  = norm('treble',  trebleRaw);

    // Master gate: if nothing clears its floor, output silence outright so no
    // residual envelope keeps the visuals twitching.
    const anySignal = (subN + bassN + lowMidN + highMidN + trebleN) > 0.02;

    // ---- 32-BIN SPECTRUM -------------------------------------------------
    // Five bands cannot make a spectrum look like anything but five blocks.
    // These are log-spaced bins across 30Hz..16kHz, gated and windowed the
    // same way the bands are, so they share the fader's response.
    if (!this._spec) this._spec = new Float32Array(32);
    if (!this._specPeak) this._specPeak = new Float32Array(32);
    {
      // WAS BROKEN: every one of the 32 bins was gated and normalised against
      // the BASS band's noise floor and ceiling. In dance music the bass band
      // sits 20-40 dB above everything else, so almost every mid and treble
      // bin fell under the bass gate and read exactly 0. That is why the two
      // spectrum-driven shaders were dead: they were being handed 32 zeros.
      //
      // Each bin now normalises against ITS OWN slow-decaying peak, which is
      // what makes a spectrum analyser readable across the whole range. The
      // fader is not bypassed: the per-bin shape is multiplied by the global
      // level, so the fader still governs how hard the whole thing responds
      // and silence still reads as silence.
      const peakDecay = Math.pow(0.5, dt / 6.0);   // 6 s half-life
      const overall = Math.min(1, ((subN + bassN + lowMidN + highMidN + trebleN) / 5.0) * 1.6);
      const f0 = 30, f1 = 16000;
      for (let i = 0; i < 32; i++) {
        const a0 = f0 * Math.pow(f1 / f0, i / 32);
        const a1 = f0 * Math.pow(f1 / f0, (i + 1) / 32);
        const raw = this._averageBand(hzToBin(a0), hzToBin(a1));

        const pk = Math.max(raw, this._specPeak[i] * peakDecay, 1e-4);
        this._specPeak[i] = pk;

        let v = 0;
        if (anySignal) {
          const lo = pk * 0.10;                     // per-bin noise shelf
          v = (raw - lo) / Math.max(1e-5, pk - lo);
          v = Math.min(1, Math.max(0, v)) * overall;
        }
        // light smoothing so bins do not flicker frame to frame
        this._spec[i] = this._spec[i] * 0.55 + v * 0.45;
      }
    }

    if (this._measUntil && nowSec < this._measUntil) {
      if (nowSec >= (this._measNext || 0)) {
        this._measNext = nowSec + 0.4;
        const L = this.levels ? this.levels() : { rms: 0, peak: 0 };
        if (typeof window !== 'undefined' && window.VJ_DIAG) {
          const d = this._dbg || {};
          const fmt = (k) => {
            const x = d[k]; if (!x) return k + '=?';
            return k + '[raw=' + x.raw.toFixed(4) + '/' + x.rawDb.toFixed(1) + 'dB' +
                   ' gate=' + x.gate.toFixed(4) +
                   ' win=' + x.lo.toFixed(1) + '..' + x.hi.toFixed(1) + 'dB' +
                   ' out=' + (x.out != null ? x.out.toFixed(2) : '?') + ' ' + (x.why || '') + ']';
          };
          window.VJ_DIAG('MAP gain=x' + (this.getGain ? this.getGain().toFixed(3) : '?') +
            ' rangeDb=' + BAND_RANGE_DB + ' gainRatio=' + gainRatio.toFixed(2) +
            ' rms=' + L.rms.toFixed(4));
          window.VJ_DIAG('    ' + fmt('sub'));
          window.VJ_DIAG('    ' + fmt('bass'));
          window.VJ_DIAG('    ' + fmt('highMid'));
        }
      }
    } else if (this._measUntil) { this._measUntil = 0; }

    // Update envelopes on the NORMALISED values.
    const subBass = this._updateEnvelope(this._envSub, subN, dt);
    const bass = this._updateEnvelope(this._envBass, bassN, dt);
    const lowMid = this._updateEnvelope(this._envLowMid, lowMidN, dt);
    const highMid = this._updateEnvelope(this._envHighMid, highMidN, dt);
    const treble = this._updateEnvelope(this._envTreble, trebleN, dt);

    // ---- PER-BAND ONSETS ------------------------------------------------
    // A kick is not the only transient worth reacting to. Snares/claps live in
    // the mids, hats in the treble. Separate detectors give layers independent
    // triggers instead of every element flashing on the same beat.
    // SLOW REFERENCE for transient detection. Every onset test compared a band
    // against its own fast envelope -- which tracks the band so closely that
    // the ratio can essentially never reach 1.3, so onset and all three hit
    // envelopes logged 0.000 on every single line. A 400ms reference is slow
    // enough for a transient to stand out against.
    if (!this._slowRef) this._slowRef = {};
    const slowRef = (k, v) => {
      const prev = this._slowRef[k] === undefined ? v : this._slowRef[k];
      const a = 1 - Math.exp(-dt / 0.400);
      this._slowRef[k] = prev + a * (v - prev);
      return prev;                                  // compare against the PAST
    };

    if (!this._bandOnset) this._bandOnset = { low: 0, mid: 0, high: 0 };
    if (!this._bandLast) this._bandLast = { low: 0, mid: 0, high: 0 };
    const bandHit = (key, rawN, avg, thresh, refractory) => {
      if (rawN > avg * thresh + 0.03 && (nowSec - this._bandLast[key]) > refractory) {
        this._bandLast[key] = nowSec;
        this._bandOnset[key] = 1.0;
      } else {
        this._bandOnset[key] = Math.max(0, this._bandOnset[key] - dt / 0.090);
      }
      return this._bandOnset[key];
    };
    const hitLow  = anySignal ? bandHit('low',  bassN,    slowRef('hl', bassN),    1.25, 0.120) : 0;
    const hitMid  = anySignal ? bandHit('mid',  highMidN, slowRef('hm', highMidN), 1.20, 0.080) : 0;
    const hitHigh = anySignal ? bandHit('high', trebleN,  slowRef('hh', trebleN),  1.18, 0.060) : 0;

    // Energy: overall RMS / spectrum energy smoothed
    // WAS a flat mean of five bands. With treble and highMid reading 0, a track
    // with bass at 0.76 produced energy=0.14 -- and every shader's master gain
    // and "relief" term is driven by energy, which is exactly why Beat Tunnel
    // and Liquid Metal looked alive on the bench (fed a constant 0.6) and flat
    // in performance. Weight the loudest band heavily so one strong band reads
    // as real energy, while the mean still rewards a full mix.
    const bandMean = (subN + bassN + lowMidN + highMidN + trebleN) / 5.0;
    const bandMax  = Math.max(subN, bassN, lowMidN, highMidN, trebleN);
    const totalRaw = 0.6 * bandMax + 0.4 * bandMean;
    const energy = this._updateEnvelope(this._envEnergy, totalRaw, dt);

    // Kick detection
    // onset threshold : subBassRaw > smoothed_sub * 1.35 + 0.02
    // refractory      : 120 ms (0.120s)
    // onset decay     : 100 ms to zero, linear
    const refractorySec = 0.120;
    let kick = 0.0;

    if (
      anySignal &&
      subN > slowRef('kick', subN) * 1.25 + 0.03 &&
      (nowSec - this._lastOnsetSec) > refractorySec
    ) {
      this._lastOnsetSec = nowSec;
      this._onsetDecay = 1.0;
    } else {
      this._onsetDecay = Math.max(0.0, this._onsetDecay - dt / 0.100);
    }
    kick = this._onsetDecay;

    // Track kick with clock and advance tempo
    this.clock.trackKick(kick, nowSec);
    const clockState = this.clock.update(dt, nowSec);

    return {
      subBass,
      bass,
      lowMid,
      highMid,
      treble,
      energy,
      kick,
      spectrum: this._spec,   // 32 log-spaced bins, 0..1
      hitLow,          // bass/kick transient
      hitMid,          // snare / clap region
      hitHigh,         // hats / percussion
      bpm: clockState.bpm,
      beatPhase: clockState.beatPhase,
      barPhase: clockState.barPhase,
        locked: clockState.locked,
      ok: true,
    };
  }

  // ---- Internal helper methods ----

  async _initStream(deviceLabelHint) {
    // Declared OUTSIDE the try so the catch can release it. Inside the try it
    // is not in scope in the catch, and referencing it there would throw a
    // ReferenceError -- turning a recoverable audio failure into a hard one.
    let stream = null;
    try {
      this.stop();

      const AudioContextClass = window.AudioContext || window.webkitAudioContext;
      if (!AudioContextClass) {
        this._ok = false;
        return false;
      }

      this._audioCtx = new AudioContextClass();

      // Step 1: getUserMedia with raw audio settings (no echo cancellation, noise suppression, AGC)
      const baseConstraints = {
        audio: {
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false,
          channelCount: 2,
        },
        video: false,
      };

      stream = await navigator.mediaDevices.getUserMedia(baseConstraints);

      // Step 2: enumerate devices and find device matching deviceLabelHint
      let targetDeviceId = null;
      let matchedLabel = '';
      try {
        const devices = await navigator.mediaDevices.enumerateDevices();
        const audioInputs = devices.filter((d) => d.kind === 'audioinput');
        const hintLower = (deviceLabelHint || '').toLowerCase();

        for (const dev of audioInputs) {
          if (hintLower && dev.label.toLowerCase().includes(hintLower)) {
            targetDeviceId = dev.deviceId;
            matchedLabel = dev.label;
            break;
          }
        }

        // If target device found and not already the active stream's device track
        if (targetDeviceId) {
          const activeTrack = stream.getAudioTracks()[0];
          const activeSettings = activeTrack ? activeTrack.getSettings() : null;
          if (!activeSettings || activeSettings.deviceId !== targetDeviceId) {
            // Stop temporary stream and open the specific device
            activeTrack?.stop();
            const specificConstraints = {
              audio: {
                echoCancellation: false,
                noiseSuppression: false,
                autoGainControl: false,
                channelCount: 2,
                deviceId: { exact: targetDeviceId },
              },
              video: false,
            };
            stream = await navigator.mediaDevices.getUserMedia(specificConstraints);
          }
        }
      } catch (_) {
        // Fall back to stream already acquired if enumerate/re-open fails
      }

      this._mediaStream = stream;
      const primaryTrack = stream.getAudioTracks()[0];
      this._deviceName = matchedLabel || (primaryTrack ? primaryTrack.label : 'Audio Input') || 'Audio Input';

      // Setup Web Audio graph
      this._sourceNode = this._audioCtx.createMediaStreamSource(this._mediaStream);

      // Input gain: 0.25 (FLX10 booth output is line level and hot)
      this._gainNode = this._audioCtx.createGain();
      this._gainNode.gain.value = this._inputGain != null ? this._inputGain : 0.25;

      // AnalyserNode: fftSize = 2048, smoothingTimeConstant = 0
      this._analyser = this._audioCtx.createAnalyser();
      this._analyser.fftSize = 2048;
      this._analyser.smoothingTimeConstant = 0;
      // getByteFrequencyData returns DECIBELS mapped across [minDecibels,
      // maxDecibels]. The defaults are -100..-30 dB, and music sits well above
      // -30, so every band pinned at the top of the scale before the fader did
      // anything -- raw only moved 0.78 -> 0.97 across an 18 dB gain change.
      // -60..0 dB puts the musical range across the full 0..1 span.
      this._analyser.minDecibels = -60;
      this._analyser.maxDecibels = 0;

      this._freqData = new Uint8Array(this._analyser.frequencyBinCount);

      // Block DC and sub-20Hz rumble before analysis. Kick fundamentals live
      // at 40-60Hz, so a 22Hz high-pass is safely below anything musical.
      this._hpf = this._audioCtx.createBiquadFilter();
      this._hpf.type = 'highpass';
      this._hpf.frequency.value = 22;
      this._hpf.Q.value = 0.707;

      this._sourceNode.connect(this._gainNode);
      this._gainNode.connect(this._hpf);
      this._hpf.connect(this._analyser);

      this._ok = true;
      return true;
    } catch (_) {
      // RELEASE WHAT WE ALREADY TOOK.
      // The failure can happen after getUserMedia succeeded but before the
      // stream reached this._mediaStream, in which case stop() on the next
      // retry cannot find it. An AudioContext was also already constructed.
      // Leaving both behind on every failed attempt is a real leak on a
      // 2GB device, and this path runs repeatedly while the USB interface
      // is still held by a dying Chromium after a restart.
      try {
        if (stream && stream.getTracks) {
          for (const t of stream.getTracks()) { try { t.stop(); } catch (_e) {} }
        }
      } catch (_e) {}
      try {
        if (this._audioCtx && this._audioCtx.state !== 'closed') {
          this._audioCtx.close();
        }
      } catch (_e) {}
      this._audioCtx = null;
      this._mediaStream = null;
      this._sourceNode = null;
      this._ok = false;
      this._deviceName = '';
      return false;
    }
  }

  /**
   * Level for one of the five wide musical bands.
   *
   * _averageBand() is a flat mean, which is right for the 32 narrow spectrum
   * bins but wrong here. treble spans 5k-16k: in a byte-FFT dB domain an empty
   * bin reads 0, and most of 8k-16k is empty on nearly all material, so the
   * mean is dragged to nothing by the silent top of the band. That -- not the
   * noise gate -- is why treble logged exactly 0.000 forever while bass (a
   * narrow 75-200Hz band) read fine. highMid (1k-5k) suffered the same way,
   * more mildly, which matches it hovering around 0.005.
   *
   * Weighting the loudest bin in the band is standard band metering: what a
   * listener hears as "hats" is a few loud bins, not the average of 11kHz.
   */
  _bandLevel(startBinFloat, endBinFloat) {
    const mean = this._averageBand(startBinFloat, endBinFloat);
    const peak = this._peakBand(startBinFloat, endBinFloat);
    return 0.35 * mean + 0.65 * peak;
  }

  _peakBand(startBinFloat, endBinFloat) {
    if (!this._freqData) return 0.0;
    const binCount = this._freqData.length;
    let minBin = Math.max(0, Math.floor(startBinFloat));
    let maxBin = Math.min(binCount - 1, Math.ceil(endBinFloat));
    if (minBin > maxBin) { const t = minBin; minBin = maxBin; maxBin = t; }
    let peak = 0.0;
    for (let i = minBin; i <= maxBin; i++) {
      if (this._freqData[i] > peak) peak = this._freqData[i];
    }
    return peak / 255.0;
  }

  _averageBand(startBinFloat, endBinFloat) {
    if (!this._freqData) return 0.0;
    const binCount = this._freqData.length;
    let minBin = Math.max(0, Math.floor(startBinFloat));
    let maxBin = Math.min(binCount - 1, Math.ceil(endBinFloat));

    if (minBin > maxBin) {
      const tmp = minBin;
      minBin = maxBin;
      maxBin = tmp;
    }

    let sum = 0.0;
    let count = 0;
    for (let i = minBin; i <= maxBin; i++) {
      sum += this._freqData[i];
      count++;
    }

    if (count === 0) return 0.0;
    const rawAvg = sum / count / 255.0; // 0..1
    const noiseFloor = 0.001;
    return rawAvg > noiseFloor ? rawAvg : 0.0;
  }

  _updateEnvelope(env, input, dt) {
    const tau = input > env.value ? env.tauAttack : env.tauRelease;
    const alpha = tau <= 0.0 ? 1.0 : 1.0 - Math.exp(-dt / tau);
    env.value += alpha * (input - env.value);
    return Math.max(0.0, Math.min(1.0, env.value));
  }
  /** Input gain multiplier. 1.0 = unity. FLX10 booth out is hot, default 0.25. */
  setGain(g) {
    this._inputGain = Math.max(0.0, Math.min(4.0, Number(g) || 0));
    if (this._gainNode) this._gainNode.gain.value = this._inputGain;
    return this._inputGain;
  }

  getGain() {
    return this._inputGain != null ? this._inputGain : 0.25;
  }

  /**
   * True post-gain peak and RMS of the current buffer, 0..1.
   * Peak is what tells you about clipping; RMS is what you set levels by.
   */
  levels() {
    if (!this._analyser) return { peak: 0, rms: 0, clip: false };
    const n = this._analyser.fftSize;
    if (!this._lvlBuf || this._lvlBuf.length !== n) this._lvlBuf = new Uint8Array(n);
    this._analyser.getByteTimeDomainData(this._lvlBuf);
    let peak = 0, sum = 0;
    for (let i = 0; i < n; i++) {
      const v = (this._lvlBuf[i] - 128) / 128;
      const a = v < 0 ? -v : v;
      if (a > peak) peak = a;
      sum += v * v;
    }
    return { peak, rms: Math.sqrt(sum / n), clip: peak > 0.99 };
  }

  /**
   * Measure the noise floor. Run with the booth feed SILENT (or unplugged):
   * whatever is present is treated as noise and gated out from then on.
   * Persisted, so it survives reboots.
   */
  calibrateNoiseFloor(seconds = 3) {
    this._calPeak = {};
    this._calSum = {};
    this._calSamples = 0;
    this._calUntil = (performance.now() * 0.001) + seconds;
    if (typeof window !== 'undefined' && window.VJ_DIAG)
      window.VJ_DIAG('calibrating noise floor for ' + seconds + 's — keep the input SILENT');
    return seconds;
  }

  get calibrating() {
    return !!(this._calUntil && (performance.now() * 0.001) < this._calUntil);
  }

  getNoiseFloor() { return this._noise || this._loadNoiseFloor(); }

  clearNoiseFloor() {
    this._noise = { sub: 0.010, bass: 0.010, lowMid: 0.008, highMid: 0.006, treble: 0.005 };
    this._saveNoiseFloor(this._noise);
    return this._noise;
  }

  _loadNoiseFloor() {
    try {
      const raw = localStorage.getItem('vj.noiseFloor');
      if (raw) {
        const v = JSON.parse(raw);
        if (v && typeof v.sub === 'number') return v;
      }
    } catch (e) {}
    // Conservative defaults until a real calibration is run.
    return { sub: 0.010, bass: 0.010, lowMid: 0.008, highMid: 0.006, treble: 0.005 };
  }

  _saveNoiseFloor(v) {
    try { localStorage.setItem('vj.noiseFloor', JSON.stringify(v)); } catch (e) {}
  }

  /**
   * Log raw (pre-normalisation) band levels for a few seconds so the actual
   * noise floor can be read rather than guessed at.
   */
  measure(seconds = 5) {
    this._measUntil = (performance.now() * 0.001) + seconds;
    this._measNext = 0;
    if (typeof window !== 'undefined' && window.VJ_DIAG)
      window.VJ_DIAG('MEASURING raw input for ' + seconds + 's (gain x' +
        (this.getGain ? this.getGain().toFixed(2) : '?') + ')');
    return seconds;
  }

  /**
   * Measure the LOUDEST you will actually play. Run this with a track at the
   * hottest level you would ever push the FLX10 to -- right at the point the
   * booth meters go red. Everything above this reference is soft-kneed.
   */
  calibrateMax(seconds = 8) {
    this._maxPeak = {};
    this._maxSamples = 0;
    this._maxUntil = (performance.now() * 0.001) + seconds;
    if (typeof window !== 'undefined' && window.VJ_DIAG)
      window.VJ_DIAG('measuring MAX reference for ' + seconds + 's — play your loudest now');
    return seconds;
  }

  get calibratingMax() {
    return !!(this._maxUntil && (performance.now() * 0.001) < this._maxUntil);
  }

  getMaxRef() { return this._maxRef || this._loadMaxRef(); }

  _loadMaxRef() {
    try {
      const raw = localStorage.getItem('vj.maxRef');
      if (raw) {
        const v = JSON.parse(raw);
        if (v && typeof v.sub === 'number') return v;
      }
    } catch (e) {}
    // Sensible until measured: typical loud-music band levels.
    return { sub: 0.85, bass: 0.82, lowMid: 0.72, highMid: 0.65, treble: 0.58, __gain: 0.25 };
  }

  _saveMaxRef(v) {
    try { localStorage.setItem('vj.maxRef', JSON.stringify(v)); } catch (e) {}
  }

  /** Musical dynamic range the bands map across, in dB below the calibrated
   *  ceiling. Larger = gentler/wider response, smaller = punchier/more
   *  contrasty. 36 is a reasonable default for club material. */
  setBandRangeDb(db) {
    this._bandRangeDb = Math.max(12, Math.min(60, Number(db) || 36));
    try { localStorage.setItem('vj.bandRangeDb', String(this._bandRangeDb)); } catch (e) {}
    return this._bandRangeDb;
  }

  getBandRangeDb() {
    if (this._bandRangeDb == null) {
      let v = 36;
      try { const r = localStorage.getItem('vj.bandRangeDb'); if (r) v = parseFloat(r); } catch (e) {}
      this._bandRangeDb = (isFinite(v) && v > 0) ? v : 36;
    }
    return this._bandRangeDb;
  }

}
