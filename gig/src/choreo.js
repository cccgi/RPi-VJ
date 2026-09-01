// gig/src/choreo.js
// Beat Choreography / Modulation Matrix & Tempo Clock (G-13)
// ES Module, zero external dependencies.

/**
 * TempoClock handles manual tap tempo, auto-BPM onset tracking, manual nudging,
 * and phase integration for beatPhase and barPhase.
 */
export class TempoClock {
  /**
   * @param {Object} [options]
   * @param {number} [options.bpm=128] Initial default BPM
   */
  constructor(options = {}) {
    this.bpm = options.bpm || 128;
    this.phaseBeats = 0.0; // Continuous beat accumulator
    this.lastTimeSec = null;

    // Tap tempo state
    this.tapTimes = [];
    this.lastManualTapSec = -1000.0; // Timestamp of last manual tap

    // Auto-BPM state
    this.onsetTimes = []; // Timestamps of detected kick onsets
    this.lastKickState = false;

    // Phase-lock (PLL) state
    this.phaseLockGain = 0.25;  // fraction of phase error corrected per onset
    this.lastLockSec = -1000.0; // last time an onset corrected the phase
    // Widened 4.0 -> 6.0s 2026-08-26 (HANDOFF item 3): the old 4s window read
    // "unlocked" through ordinary breakdown/fill gaps between real onsets,
    // even though the tempo estimate itself was still fine. See update().
    this.lockDecaySec = 6.0;
  }

  /**
   * Tap tempo: average last 4 intervals, reject outliers beyond ±30% of median.
   * Priority: locks manual tempo and sets lastManualTapSec.
   * @param {number} [nowSec]
   * @returns {number} Current BPM
   */
  tap(nowSec = performance.now() * 0.001) {
    this.lastManualTapSec = nowSec;

    // Filter taps older than 3.0 seconds
    this.tapTimes = this.tapTimes.filter(t => (nowSec - t) < 3.0);
    this.tapTimes.push(nowSec);

    if (this.tapTimes.length >= 2) {
      // Calculate intervals between consecutive taps
      const intervals = [];
      for (let i = 1; i < this.tapTimes.length; i++) {
        intervals.push(this.tapTimes[i] - this.tapTimes[i - 1]);
      }

      // Keep up to last 4 intervals
      const recentIntervals = intervals.slice(-4);

      if (recentIntervals.length === 1) {
        const bpm = 60.0 / recentIntervals[0];
        if (bpm >= 40 && bpm <= 240) {
          this.bpm = bpm;
        }
      } else {
        // Median of recent intervals
        const sorted = [...recentIntervals].sort((a, b) => a - b);
        const mid = Math.floor(sorted.length / 2);
        const medianInterval = sorted.length % 2 === 0
          ? (sorted[mid - 1] + sorted[mid]) / 2
          : sorted[mid];

        // Reject outliers beyond ±30% of median
        const validIntervals = recentIntervals.filter(
          inv => Math.abs(inv - medianInterval) <= 0.30 * medianInterval
        );

        if (validIntervals.length > 0) {
          const avgInterval = validIntervals.reduce((a, b) => a + b, 0) / validIntervals.length;
          const calculatedBpm = 60.0 / avgInterval;
          if (calculatedBpm >= 40 && calculatedBpm <= 240) {
            this.bpm = calculatedBpm;
          }
        }
      }
    }

    return this.bpm;
  }

  /**
   * Manual nudge: ± deltaBpm (e.g. +0.1 or -0.1).
   * @param {number} deltaBpm
   */
  nudge(deltaBpm) {
    this.bpm = Math.max(40, Math.min(240, this.bpm + deltaBpm));
  }

  /**
   * Phase reset: snap downbeat now (barPhase = 0, beatPhase = 0).
   */
  downbeat() {
    this.phaseBeats = 0.0;
  }

  /**
   * Reset phase to specific beat offset.
   * @param {number} [beats=0.0]
   */
  resetPhase(beats = 0.0) {
    this.phaseBeats = beats;
  }

  /**
   * Set BPM explicitly.
   * @param {number} bpm
   * @param {boolean} [isManual=true]
   */
  setBpm(bpm, isManual = true) {
    if (bpm >= 40 && bpm <= 240) {
      this.bpm = bpm;
      if (isManual) {
        this.lastManualTapSec = performance.now() * 0.001;
      }
    }
  }

  /**
   * Register kick onset for Auto-BPM detection.
   * Auto-BPM: track inter-onset intervals from kick, histogram into bins over rolling 8s window,
   * pick mode, fold into 70-180 BPM.
   * Never overrides manual tap for at least 30s.
   * @param {number} kickValue (0..1)
   * @param {number} nowSec
   */
  trackKick(kickValue, nowSec) {
    // Detect rising edge of kick
    const isKick = kickValue > 0.6;
    const wasKick = this.lastKickState;
    this.lastKickState = isKick;

    if (isKick && !wasKick) {
      this.onsetTimes.push(nowSec);
      // PHASE LOCK. Previously the clock only estimated BPM; phaseBeats
      // free-ran from session start, so beatPhase/barPhase drifted
      // arbitrarily against the music and every phase-driven shader
      // "moved at its own pace". Pull the accumulator toward the nearest
      // whole beat on each detected onset (a soft PLL, so a spurious
      // onset cannot yank the whole show).
      const nearest = Math.round(this.phaseBeats);
      const err = nearest - this.phaseBeats;      // -0.5 .. +0.5 beats
      if (Math.abs(err) < 0.5) {
        this.phaseBeats += err * this.phaseLockGain;
      }
      this.lastLockSec = nowSec;
    }

    // Keep rolling 8s window of onsets
    this.onsetTimes = this.onsetTimes.filter(t => (nowSec - t) <= 8.0);

    // If manual tap was made within last 30s, do not let auto-BPM override
    if (nowSec - this.lastManualTapSec < 30.0) {
      return;
    }

    if (this.onsetTimes.length < 4) {
      return;
    }

    // Calculate IOIs (inter-onset intervals)
    const iois = [];
    for (let i = 1; i < this.onsetTimes.length; i++) {
      const dt = this.onsetTimes[i] - this.onsetTimes[i - 1];
      if (dt >= 0.2 && dt <= 2.0) { // filter out extreme noise (30 to 300 BPM)
        iois.push(dt);
      }
    }

    if (iois.length < 3) return;

    // Histogram IOIs into bins of 20ms (0.02s)
    const binSize = 0.02;
    const bins = new Map();
    for (const ioi of iois) {
      const binKey = Math.round(ioi / binSize) * binSize;
      bins.set(binKey, (bins.get(binKey) || 0) + 1);
    }

    // Pick mode (highest count)
    let bestBin = null;
    let maxCount = 0;
    for (const [binKey, count] of bins.entries()) {
      if (count > maxCount) {
        maxCount = count;
        bestBin = binKey;
      }
    }

    if (bestBin && maxCount >= 2) {
      let rawBpm = 60.0 / bestBin;
      // Fold into 70–180 BPM range
      while (rawBpm < 70.0) rawBpm *= 2.0;
      while (rawBpm > 180.0) rawBpm /= 2.0;

      // Smooth auto-BPM adjustment towards detected mode (IIR low-pass)
      this.bpm += 0.05 * (rawBpm - this.bpm);
    }
  }

  /**
   * Advances clock by dt seconds and returns current tempo features.
   * @param {number} dt Time step in seconds
   * @param {number} [nowSec]
   * @returns {{ bpm: number, beatPhase: number, barPhase: number, beats: number }}
   */
  update(dt, nowSec = performance.now() * 0.001) {
    if (dt > 0) {
      const beatsDelta = (this.bpm / 60.0) * dt;
      this.phaseBeats += beatsDelta;
    }

    // beatPhase = fract(beats), barPhase = fract(beats/4)
    const beats = this.phaseBeats;
    const beatPhase = ((beats % 1.0) + 1.0) % 1.0;
    const barPhase = (((beats / 4.0) % 1.0) + 1.0) % 1.0;

    // 1.0 while onsets are actively correcting the phase, decaying to 0
    // over LOCK_DECAY_SEC of silence. Shaders can use this to fall back to
    // audio-envelope motion when there is no usable beat.
    //
    // Checked against a real captured show.log (2026-08-26): during a
    // steady four-on-the-floor stretch, onsets landed every beat and locked
    // correctly held 0.85-1.00. The 0.06-1.00 swing HANDOFF flagged came
    // from breakdown-style passages with NO band transient at all (hitLow
    // read 0.000 for a 12s+ stretch, not just the kick/subN test) -- there
    // was nothing to lock onto, so decaying toward 0 was the correct
    // behaviour, not a PLL bug. phaseLockGain (above) does not touch this
    // number at all -- it only scales each individual phase correction, not
    // how long "recently corrected" is remembered. This constant is the
    // actual lever: raise it if visuals should keep reading "locked" through
    // longer natural gaps (fills, breakdowns) before falling back to
    // envelope-only motion.
    const lockAge = nowSec - this.lastLockSec;
    const locked = Math.max(0.0, Math.min(1.0, 1.0 - lockAge / this.lockDecaySec));

    return {
      bpm: this.bpm,
      beatPhase,
      barPhase,
      beats,
      locked
    };
  }
}

/**
 * Choreo Modulation Matrix
 * LFO rates are expressed in BARS (1/4, 1/2, 1, 2, 4, 8, 16) and driven from barPhase,
 * so movement lands on musical time for free.
 */
export class Choreo {
  /**
   * @param {Object} [options]
   * @param {TempoClock} [options.clock] Optional shared TempoClock instance
   */
  constructor(options = {}) {
    this.clock = options.clock || new TempoClock(options);
    this.mods = []; // Array of active modulators { id, source, target, depth, mode, rate, phaseOffset, ... }
    this._nextModId = 1;

    // Random S&H state cache per mod ID
    this._randomLfoState = new Map();
  }

  /**
   * Add a modulation routing.
   * @param {Object} mod
   * @param {string} mod.source 'lfoSine'|'lfoTri'|'lfoSaw'|'lfoSquare'|'lfoRandom'|'bass'|'treble'|'energy'|'kick'|'beatPhase'|'barPhase'
   * @param {string} mod.target Uniform name in active shader or post param
   * @param {number} [mod.depth=1.0] Modulation depth / scale factor
   * @param {string} [mod.mode='add'] 'add' | 'multiply' | 'replace'
   * @param {number} [mod.rate=1.0] Rate in BARS (e.g. 1/4, 1/2, 1, 2, 4, 8, 16). Default 1 bar.
   * @param {number} [mod.phaseOffset=0.0] Phase offset in 0..1
   * @param {number} [mod.min] Optional clamp min
   * @param {number} [mod.max] Optional clamp max
   * @returns {number} Modulator ID
   */
  addMod({ source, target, depth = 1.0, mode = 'add', rate = 1.0, phaseOffset = 0.0, min, max }) {
    const id = this._nextModId++;
    this.mods.push({
      id,
      source,
      target,
      depth: Number(depth) || 0.0,
      mode: mode || 'add',
      rate: Number(rate) > 0 ? Number(rate) : 1.0,
      phaseOffset: Number(phaseOffset) || 0.0,
      min: min !== undefined ? Number(min) : undefined,
      max: max !== undefined ? Number(max) : undefined
    });
    return id;
  }

  /**
   * Remove modulator by ID.
   * @param {number} id
   * @returns {boolean}
   */
  removeMod(id) {
    const idx = this.mods.findIndex(m => m.id === id);
    if (idx !== -1) {
      this.mods.splice(idx, 1);
      this._randomLfoState.delete(id);
      return true;
    }
    return false;
  }

  /**
   * Remove all modulators targeting a given property name.
   * @param {string} target
   */
  removeModsForTarget(target) {
    this.mods = this.mods.filter(m => {
      if (m.target === target) {
        this._randomLfoState.delete(m.id);
        return false;
      }
      return true;
    });
  }

  /**
   * Clear all active modulators.
   */
  clearMods() {
    this.mods = [];
    this._randomLfoState.clear();
  }

  /**
   * Set list of modulators directly (e.g. when loading scene presets).
   * @param {Array<Object>} modList
   */
  setMods(modList = []) {
    this.clearMods();
    if (Array.isArray(modList)) {
      for (const m of modList) {
        this.addMod(m);
      }
    }
  }

  /**
   * Evaluates an LFO waveform at continuous bar position (bars = beats / 4).
   * @param {string} type 'lfoSine' | 'lfoTri' | 'lfoSaw' | 'lfoSquare' | 'lfoRandom'
   * @param {number} rateBars LFO cycle length in bars (e.g. 8 for an 8-bar LFO)
   * @param {number} totalBars Total continuous bars elapsed (beats / 4)
   * @param {number} phaseOffset Phase offset (0..1)
   * @param {number} modId Modulator ID (for S&H state)
   * @returns {number} Value in [0, 1] or [-1, 1] normalized waveform
   */
  _evalLfo(type, rateBars, totalBars, phaseOffset, modId) {
    const cyclePos = (totalBars / rateBars) + phaseOffset;
    // Fractional cycle in [0, 1)
    const phase = ((cyclePos % 1.0) + 1.0) % 1.0;

    switch (type) {
      case 'lfoSine':
        // Sine wave in [0, 1] (0 at phase 0, peaks at 0.25, etc.)
        return 0.5 + 0.5 * Math.sin(phase * 2.0 * Math.PI);

      case 'lfoTri':
        // Triangle wave in [0, 1] (0 -> 1 -> 0)
        return phase < 0.5 ? phase * 2.0 : 2.0 - phase * 2.0;

      case 'lfoSaw':
        // Ramp up saw wave in [0, 1]
        return phase;

      case 'lfoSquare':
        // Square wave in [0, 1] (1 for first half, 0 for second half)
        return phase < 0.5 ? 1.0 : 0.0;

      case 'lfoRandom': {
        // Sample & Hold stepped random value per cycle/step
        const cycleIndex = Math.floor(cyclePos);
        let state = this._randomLfoState.get(modId);
        if (!state || state.cycleIndex !== cycleIndex) {
          // Deterministic pseudorandom float in [0, 1] based on cycleIndex and modId
          const seed = (cycleIndex * 1973 + modId * 9277 + 1013904223) >>> 0;
          const val = ((seed * 1664525 + 1013904223) >>> 0) / 4294967296.0;
          state = { cycleIndex, val };
          this._randomLfoState.set(modId, state);
        }
        return state.val;
      }

      default:
        return 0.0;
    }
  }

  /**
   * Applies modulation matrix to uniform set based on audio features and tempo clock.
   * @param {Object} uniforms Current uniform map / parameters
   * @param {Object} [features] Audio features { subBass, bass, lowMid, highMid, treble, energy, kick, bpm, beatPhase, barPhase, ok }
   * @param {number} [dt=0.016] Delta time in seconds
   * @returns {Object} Modulated copy of uniform set
   */
  apply(uniforms = {}, features = {}, dt = 0.016) {
    const nowSec = performance.now() * 0.001;

    // Track kick for auto-BPM if available
    if (features && features.kick !== undefined) {
      this.clock.trackKick(features.kick, nowSec);
    }

    // Update tempo clock
    const clockState = this.clock.update(dt, nowSec);

    // Merge clock tempo fields into features
    const feat = {
      subBass: features?.subBass || 0.0,
      bass: features?.bass || 0.0,
      lowMid: features?.lowMid || 0.0,
      highMid: features?.highMid || 0.0,
      treble: features?.treble || 0.0,
      energy: features?.energy || 0.0,
      kick: features?.kick || 0.0,
      bpm: clockState.bpm,
      beatPhase: clockState.beatPhase,
      barPhase: clockState.barPhase,
      beats: clockState.beats,
      ok: features?.ok ?? true
    };

    // Shallow clone uniforms object to avoid mutating input directly
    const out = { ...uniforms };

    // Total bars elapsed (beats / 4)
    const totalBars = feat.beats / 4.0;

    for (const mod of this.mods) {
      let rawSourceValue = 0.0;

      // Extract modulation source value
      if (mod.source.startsWith('lfo')) {
        rawSourceValue = this._evalLfo(mod.source, mod.rate, totalBars, mod.phaseOffset, mod.id);
      } else {
        switch (mod.source) {
          case 'bass':
            rawSourceValue = feat.bass;
            break;
          case 'subBass':
            rawSourceValue = feat.subBass;
            break;
          case 'treble':
            rawSourceValue = feat.treble;
            break;
          case 'energy':
            rawSourceValue = feat.energy;
            break;
          case 'kick':
            rawSourceValue = feat.kick;
            break;
          case 'beatPhase':
            rawSourceValue = feat.beatPhase;
            break;
          case 'barPhase':
            rawSourceValue = feat.barPhase;
            break;
          case 'lowMid':
            rawSourceValue = feat.lowMid;
            break;
          case 'highMid':
            rawSourceValue = feat.highMid;
            break;
          default:
            rawSourceValue = 0.0;
        }
      }

      const modAmount = rawSourceValue * mod.depth;
      const baseVal = out[mod.target] !== undefined ? out[mod.target] : 0.0;

      let finalVal;
      if (typeof baseVal === 'number') {
        if (mod.mode === 'multiply') {
          finalVal = baseVal * modAmount;
        } else if (mod.mode === 'replace') {
          finalVal = modAmount;
        } else {
          // default: 'add'
          finalVal = baseVal + modAmount;
        }

        if (mod.min !== undefined) finalVal = Math.max(mod.min, finalVal);
        if (mod.max !== undefined) finalVal = Math.min(mod.max, finalVal);

        out[mod.target] = finalVal;
      } else if (Array.isArray(baseVal)) {
        // If target is a vector (e.g. vec2, vec3, vec4), apply to elements
        out[mod.target] = baseVal.map(v => {
          let r = v;
          if (mod.mode === 'multiply') r = v * modAmount;
          else if (mod.mode === 'replace') r = modAmount;
          else r = v + modAmount;
          if (mod.min !== undefined) r = Math.max(mod.min, r);
          if (mod.max !== undefined) r = Math.min(mod.max, r);
          return r;
        });
      } else {
        // Uniform not present yet, initialize with modulation value
        out[mod.target] = modAmount;
      }
    }

    return out;
  }
}
