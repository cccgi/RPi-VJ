/**
 * G-10 — show.json -> 32-Key Binding + Blackout
 * Manages show state, banks, 32-key grid bindings, dynamic pad icon rendering,
 * and keyboard mirrors.
 */

export const KEY_COLORS = {
  clip: '#05ffa1',     // Green
  shader: '#ff2a6d',   // Magenta
  scene: '#a855f7',    // Purple / Violet
  system: '#ffb800',   // Amber
  empty: '#111118',    // Dark background for unmapped pads
  blackout: '#ff0033'  // Red accent for blackout
};

export const DEFAULT_KEY_MAP = {
  // Row 0 (Keys 0-7) -> 1 2 3 4 5 6 7 8
  'Digit1': 0, 'Digit2': 1, 'Digit3': 2, 'Digit4': 3, 'Digit5': 4, 'Digit6': 5, 'Digit7': 6, 'Digit8': 7,
  // Row 1 (Keys 8-15) -> Q W E R T Y U I
  'KeyQ': 8, 'KeyW': 9, 'KeyE': 10, 'KeyR': 11, 'KeyT': 12, 'KeyY': 13, 'KeyU': 14, 'KeyI': 15,
  // Row 2 (Keys 16-23) -> A S D F G H J K
  'KeyA': 16, 'KeyS': 17, 'KeyD': 18, 'KeyF': 19, 'KeyG': 20, 'KeyH': 21, 'KeyJ': 22, 'KeyK': 23,
  // Row 3 (Keys 24-31) -> Z X C V B N M Space/Escape
  'KeyZ': 24, 'KeyX': 25, 'KeyC': 26, 'KeyV': 27, 'KeyB': 28, 'KeyN': 29, 'KeyM': 30, 'Space': 31, 'Escape': 31
};

export class ShowManager {
  /**
   * @param {Object} [options]
   * @param {Function} [options.onTrigger] Callback for clip/shader triggers (pad, active)
   * @param {Function} [options.onBlackout] Callback when blackout state changes (isBlackout)
   * @param {Function} [options.onBankChange] Callback when active bank changes (bankIndex, bank)
   * @param {Function} [options.onBpmChange] Callback when BPM changes (bpm)
   * @param {Function} [options.onKeyUpdate] Callback when key display/state updates (keyIndex, imageBlob)
   * @param {Function} [options.fileChecker] Optional custom async file verification function (src) => Promise<boolean>
   */
  constructor(options = {}) {
    this.options = options;
    this.showData = null;
    this.currentBankIndex = 0;
    this.isBlackout = false;
    // Gig-safe default. Thumbnail decoding and JPEG transfers cost memory and
    // USB time, and the operator explicitly disabled them. A show may opt in
    // with `streamDeckThumbnails: true`; restarts must not silently re-enable.
    this.disableThumbs = true;
    this.bpm = 120;
    this.tapTimes = [];
    this.activeKeys = new Set();
    this.keyBindings = new Array(32).fill(null);
    this.missingFiles = [];
    this.keyMap = { ...DEFAULT_KEY_MAP };
    this.heartbeatIntervalMs = options.heartbeatIntervalMs || 2000;
    this.heartbeatTimer = null;
    this.stateUrl = options.stateUrl || '/state';
    this.heartbeatUrl = options.heartbeatUrl || '/heartbeat';

    // Offscreen canvas for icon generation (96x96 is standard Stream Deck XL key resolution)
    this.canvas = typeof document !== 'undefined' ? document.createElement('canvas') : null;
    if (this.canvas) {
      this.canvas.width = 96;
      this.canvas.height = 96;
      this.ctx = this.canvas.getContext('2d');
    }
  }

  /**
   * Fallback empty show structure with working blackout key.
   */
  static getFallbackShow() {
    return {
      name: "Fallback Show",
      bpmDefault: 120,
      banks: [
        {
          name: "Default",
          pads: []
        }
      ],
      system: [
        { key: 24, action: "bankPrev", label: "< BANK" },
        { key: 25, action: "bankNext", label: "BANK >" },
        { key: 30, action: "tapTempo", label: "TAP" },
        { key: 31, action: "blackout", label: "BLACK" }
      ]
    };
  }

  /**
   * Loads and validates show configuration from JSON object or string.
   * Ensures key 31 blackout fallback is always active if JSON parsing fails.
   * @param {string|Object} rawJson
   * @returns {Promise<{success: boolean, missingFiles: Array<string>, errors: Array<string>}>}
   */
  async loadShow(rawJson) {
    const errors = [];
    this.missingFiles = [];
    let parsedData = null;

    if (typeof rawJson === 'string') {
      try {
        parsedData = JSON.parse(rawJson);
      } catch (err) {
        errors.push(`JSON Parse Error: ${err.message}`);
        console.error('Failed to parse show.json, loading fallback show with blackout.', err);
        parsedData = ShowManager.getFallbackShow();
      }
    } else if (typeof rawJson === 'object' && rawJson !== null) {
      parsedData = rawJson;
    } else {
      errors.push('Invalid show data provided, loading fallback show.');
      parsedData = ShowManager.getFallbackShow();
    }

    // Validate and structure parsed show
    if (!parsedData.banks || !Array.isArray(parsedData.banks) || parsedData.banks.length === 0) {
      parsedData.banks = [{ name: 'Default', pads: [] }];
    }

    if (!parsedData.system || !Array.isArray(parsedData.system)) {
      parsedData.system = [];
    }

    // Ensure key 31 blackout fallback is always present
    const hasBlackout = parsedData.system.some(s => s.key === 31 && s.action === 'blackout');
    if (!hasBlackout) {
      parsedData.system = parsedData.system.filter(s => s.key !== 31);
      parsedData.system.push({ key: 31, action: 'blackout', label: 'BLACK' });
    }

    this.showData = parsedData;
    this.disableThumbs = parsedData.streamDeckThumbnails !== true;
    this.bpm = parsedData.bpmDefault || 120;
    this.currentBankIndex = 0;

    // Check missing files across all banks at load time
    await this.verifyFiles();

    // Restore previous bank/state if available
    await this.restoreLastState();

    // Rebuild active key bindings for current bank
    this.rebuildKeyBindings();

    // Start watchdog heartbeat
    this.startHeartbeat();

    return {
      success: errors.length === 0 && this.missingFiles.length === 0,
      missingFiles: this.missingFiles,
      errors: errors
    };
  }

  /**
   * Verifies that all src files referenced in banks exist.
   * Reports missing files at load time.
   */
  async verifyFiles() {
    this.missingFiles = [];
    if (!this.showData || !this.showData.banks) return;

    for (const bank of this.showData.banks) {
      if (!Array.isArray(bank.pads)) continue;
      for (const pad of bank.pads) {
        if (pad && pad.src) {
          const exists = await this.checkFileExists(pad.src);
          if (!exists) {
            this.missingFiles.push(pad.src);
            console.warn(`[ShowManager] Missing file referenced in show: "${pad.src}" (Bank: ${bank.name}, Key: ${pad.key})`);
          }
        }
      }
    }
  }

  /**
   * Helper to check if a file path exists via fetch or custom fileChecker.
   * @param {string} src
   * @returns {Promise<boolean>}
   */
  async checkFileExists(src) {
    if (this.options.fileChecker) {
      try {
        return await this.options.fileChecker(src);
      } catch {
        return false;
      }
    }
    if (typeof fetch === 'function') {
      try {
        const res = await fetch(src, { method: 'HEAD' });
        return res.ok;
      } catch {
        // Fallback GET check if HEAD not supported
        try {
          const res = await fetch(src, { method: 'GET' });
          return res.ok;
        } catch {
          return false;
        }
      }
    }
    return true; // If outside browser/fetch environment and no checker provided
  }

  /**
   * Rebuilds the 32-key binding array for the active bank + system actions.
   */
  rebuildKeyBindings() {
    this.keyBindings = new Array(32).fill(null);

    const currentBank = this.showData?.banks?.[this.currentBankIndex];
    if (currentBank && Array.isArray(currentBank.pads)) {
      for (const pad of currentBank.pads) {
        if (pad && pad.key >= 0 && pad.key < 32 && pad.key !== 24 && pad.key !== 25 && pad.key !== 30 && pad.key !== 31) {
          this.keyBindings[pad.key] = { ...pad, isSystem: false };
        }
      }
    }

    // Keys 24-31 are owned by the host page's per-page system row, NOT by
    // show.json. A `system` block here used to overwrite it on every rebuild --
    // including every bank change -- so the host's labels appeared briefly and
    // were then clobbered. Legacy shows may still carry one; ignore it.
    if (typeof this.onRebuild === 'function') {
      try { this.onRebuild(); } catch (e) {}
    }

    // Guaranteed blackout at key 31 if nothing else claimed it.
    if (!this.keyBindings[31]) {
      this.keyBindings[31] = { key: 31, action: 'blackout', label: 'BLACK', type: 'system', isSystem: true };
    }
  }

  /**
   * Handles pad press by index (0-31).
   * @param {number} keyIndex
   * @returns {Object|null}
   */
  triggerKey(keyIndex) {
    if (keyIndex < 0 || keyIndex >= 32) return null;

    const binding = this.keyBindings[keyIndex];

    // Handle system actions
    if (binding && binding.isSystem) {
      this.handleSystemAction(binding.action, binding);
      return binding;
    }

    // Key 31 hardcoded blackout fallback check
    if (keyIndex === 31) {
      this.toggleBlackout();
      return { key: 31, action: 'blackout', type: 'system' };
    }

    // Normal clip/shader pad trigger
    if (binding) {
      const isActive = !this.activeKeys.has(keyIndex);
      if (isActive) {
        this.activeKeys.add(keyIndex);
      } else {
        this.activeKeys.delete(keyIndex);
      }

      if (this.options.onTrigger) {
        this.options.onTrigger(binding, isActive);
      }
      return { ...binding, active: isActive };
    }

    return null;
  }

  /**
   * Handles system action executions.
   * @param {string} action
   * @param {Object} item
   */
  handleSystemAction(action, item) {
    switch (action) {
      case 'blackout':
        this.toggleBlackout();
        break;
      case 'bankPrev':
        this.prevBank();
        break;
      case 'bankNext':
        this.nextBank();
        break;
      case 'tapTempo':
        this.handleTapTempo();
        break;
      default:
        console.warn(`Unknown system action: ${action}`);
    }
  }

  /**
   * Toggles blackout state.
   */
  toggleBlackout() {
    this.isBlackout = !this.isBlackout;
    if (this.options.onBlackout) {
      this.options.onBlackout(this.isBlackout);
    }
    return this.isBlackout;
  }

  /**
   * Switches to previous bank.
   */
  prevBank() {
    if (!this.showData?.banks?.length) return;
    const count = this.showData.banks.length;
    this.currentBankIndex = (this.currentBankIndex - 1 + count) % count;
    this.rebuildKeyBindings();
    this.persistState();
    if (this.options.onBankChange) {
      this.options.onBankChange(this.currentBankIndex, this.showData.banks[this.currentBankIndex]);
    }
  }

  /**
   * Switches to next bank.
   */
  nextBank() {
    if (!this.showData?.banks?.length) return;
    const count = this.showData.banks.length;
    this.currentBankIndex = (this.currentBankIndex + 1) % count;
    this.rebuildKeyBindings();
    this.persistState();
    if (this.options.onBankChange) {
      this.options.onBankChange(this.currentBankIndex, this.showData.banks[this.currentBankIndex]);
    }
  }

  /**
   * Returns current active state snapshot.
   */
  getState() {
    return {
      bankIndex: this.currentBankIndex,
      bankName: this.showData?.banks?.[this.currentBankIndex]?.name || '',
      bpm: this.bpm,
      isBlackout: this.isBlackout,
      timestamp: Date.now()
    };
  }

  /**
   * Persists current state to ~/vj-state.json via POST /state and localStorage.
   */
  persistState() {
    const state = this.getState();
    if (typeof localStorage !== 'undefined') {
      try {
        localStorage.setItem('vj_last_state', JSON.stringify(state));
      } catch (e) {}
    }
    if (typeof fetch === 'function') {
      fetch(this.stateUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(state)
      }).catch(() => {});
    }
  }

  /**
   * Restores last state from GET /state or localStorage.
   */
  async restoreLastState() {
    let savedState = null;
    if (typeof fetch === 'function') {
      try {
        const res = await fetch(this.stateUrl);
        if (res.ok) {
          savedState = await res.json();
        }
      } catch (e) {}
    }
    if (!savedState || typeof savedState.bankIndex !== 'number') {
      if (typeof localStorage !== 'undefined') {
        try {
          const raw = localStorage.getItem('vj_last_state');
          if (raw) savedState = JSON.parse(raw);
        } catch (e) {}
      }
    }
    if (savedState && typeof savedState.bankIndex === 'number') {
      const bankCount = this.showData?.banks?.length || 1;
      if (savedState.bankIndex >= 0 && savedState.bankIndex < bankCount) {
        this.currentBankIndex = savedState.bankIndex;
      }
      if (typeof savedState.bpm === 'number' && savedState.bpm >= 40 && savedState.bpm <= 240) {
        this.bpm = savedState.bpm;
      }
    }
  }

  /**
   * Starts periodic heartbeat POST to /heartbeat every intervalMs.
   * Posts ONLY if performance.now() - lastFrameAt < 1000. Otherwise skips silently.
   */
  startHeartbeat(intervalMs = this.heartbeatIntervalMs) {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
    if (typeof fetch !== 'function') return;

    const sendBeat = () => {
      const lastFrame = (typeof this.options?.getLastFrameAt === 'function')
        ? this.options.getLastFrameAt()
        : ((typeof window !== 'undefined' && window.lastFrameAt !== undefined)
            ? window.lastFrameAt
            : (this.lastFrameAt || 0));

      const now = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
      // A lost GL context means frames are still being "drawn" into nothing.
      // Withhold the beat so the watchdog recovers us. See gl.js _onContextLost.
      const ctxLost = (typeof window !== 'undefined') && window.__vjContextLost === true;
      if (!ctxLost && now - lastFrame < 1000) {
        fetch(this.heartbeatUrl, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(this.getState())
        }).catch(() => {});
      }
    };

    sendBeat();
    this.heartbeatTimer = setInterval(sendBeat, intervalMs);
  }

  /**
   * Stops heartbeat timer.
   */
  stopHeartbeat() {
    if (this.heartbeatTimer) {
      clearInterval(this.heartbeatTimer);
      this.heartbeatTimer = null;
    }
  }

  /**
   * Tap tempo calculator with averaging over last 4 taps within 2.5s.
   */
  handleTapTempo() {
    const now = performance.now();
    this.tapTimes = this.tapTimes.filter(t => (now - t) < 2500);
    this.tapTimes.push(now);

    if (this.tapTimes.length >= 2) {
      let totalInterval = 0;
      for (let i = 1; i < this.tapTimes.length; i++) {
        totalInterval += (this.tapTimes[i] - this.tapTimes[i - 1]);
      }
      const avgIntervalMs = totalInterval / (this.tapTimes.length - 1);
      if (avgIntervalMs > 0) {
        const calculatedBpm = Math.round(60000 / avgIntervalMs);
        if (calculatedBpm >= 40 && calculatedBpm <= 240) {
          this.bpm = calculatedBpm;
          if (this.options.onBpmChange) {
            this.options.onBpmChange(this.bpm);
          }
        }
      }
    }
  }

  /**
   * Attaches keyboard listener for mirroring 32 keys.
   * @param {Window|Document|HTMLElement} [target]
   * @returns {Function} cleanup function
   */
  bindKeyboard(target = typeof window !== 'undefined' ? window : null) {
    if (!target) return () => {};

    const handleKeyDown = (e) => {
      // Don't intercept when user is typing in form inputs
      if (e.target && (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA')) {
        return;
      }

      const keyIndex = this.keyMap[e.code];
      if (keyIndex !== undefined) {
        e.preventDefault();
        this.triggerKey(keyIndex);
      }
    };

    target.addEventListener('keydown', handleKeyDown);
    return () => target.removeEventListener('keydown', handleKeyDown);
  }

  /**
   * Renders a key icon to a canvas and exports JPEG Blob for Stream Deck hardware.
   * Colors: clip = green (#05ffa1), shader = magenta (#ff2a6d), system = amber (#ffb800)
   * Active pad gets an inverted/bright variant.
   * @param {number} keyIndex
   * @param {Object} [options]
   * @param {number} [options.width=96]
   * @param {number} [options.height=96]
   * @returns {Promise<Blob|null>}
   */
  async renderKeyIcon(keyIndex, { width = 96, height = 96 } = {}) {
    if (!this.canvas && typeof document !== 'undefined') {
      this.canvas = document.createElement('canvas');
      this.ctx = this.canvas.getContext('2d');
    }
    if (!this.canvas || !this.ctx) return null;

    this.canvas.width = width;
    this.canvas.height = height;
    const ctx = this.ctx;
    // Stream Deck XL keys are physically inverted; icons must be rotated 180.
    // Set window.__vjIconRotate = false if this unit renders upside down.
    const rotate180 = (typeof window === 'undefined') || window.__vjIconRotate !== false;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    if (rotate180) { ctx.translate(width, height); ctx.rotate(Math.PI); }
    const binding = this.keyBindings[keyIndex];
    const isActive = this.activeKeys.has(keyIndex);
    const isBlackoutKey = keyIndex === 31 || binding?.action === 'blackout';

    let baseColor = KEY_COLORS.empty;
    let label = '';
    let sublabel = '';
    // A binding may override the swatch and request a large centred glyph
    // (page numbers, the shutdown X) instead of the normal label layout.
    const bigText = binding && binding.bigText ? String(binding.bigText) : null;
    // Thumbnails make a 28-clip library navigable at a glance. Images are
    // cached after first decode so bank changes stay cheap.
    const thumbSrc = (binding && binding.thumb && !this.disableThumbs) ? binding.thumb : null;
    if (thumbSrc) {
      // Must AWAIT the decode. Previously this kicked off the load and then
      // checked img.complete in the same tick -- always false on first render,
      // so the key drew as plain text and only picked up its thumbnail on a
      // later redraw (i.e. after you pressed it).
      await this.loadThumb(thumbSrc);
    }
    const colorOverride = binding && binding.color ? binding.color : null;

    if (binding) {
      label = binding.label || binding.name || (binding.action ? binding.action.toUpperCase() : '');
      if (binding.type === 'clip') {
        baseColor = KEY_COLORS.clip;
        // Numbers read far faster than words on a 96px key. Thumbnails will
        // replace these once the real library is transcoded (G-16).
        const m = String(label).match(/(\d+)\s*$/);
        if (m) label = String(parseInt(m[1], 10));
        sublabel = '';
      } else if (binding.type === 'shader') {
        baseColor = KEY_COLORS.shader;
        sublabel = 'FX';
      } else if (binding.type === 'scene') {
        baseColor = KEY_COLORS.scene;
        sublabel = 'SCENE';
      } else if (binding.type === 'system') {
        if (binding.sublabel !== undefined) {
          sublabel = binding.sublabel;
        } else if (bigText) {
          sublabel = '';
        } else {
          sublabel = 'SYS';
        }
        baseColor = isBlackoutKey ? (this.isBlackout ? KEY_COLORS.blackout : '#ff5500') : KEY_COLORS.system;
      }
    } else if (isBlackoutKey) {
      baseColor = this.isBlackout ? KEY_COLORS.blackout : '#ff5500';
      label = 'BLACK';
      sublabel = '';
    }
    if (binding && binding.sublabel !== undefined) sublabel = binding.sublabel;
    if (colorOverride) baseColor = colorOverride;

    // Draw background
    if (isActive || (isBlackoutKey && this.isBlackout)) {
      ctx.fillStyle = baseColor;
      ctx.fillRect(0, 0, width, height);

      // INVERTED active state, matching the thumbnail keys: white outer edge
      // with a black inner line, which reads against any fill colour.
      ctx.strokeStyle = '#ffffff';
      ctx.lineWidth = 8;
      ctx.strokeRect(4, 4, width - 8, height - 8);
      ctx.strokeStyle = '#000000';
      ctx.lineWidth = 2;
      ctx.strokeRect(9, 9, width - 18, height - 18);

      // Active text: bold neon green. Black-on-dark was illegible on pads
      // whose base colour is already dark (the whole video-FX page).
      ctx.fillStyle = '#05ffa1';
      ctx.shadowColor = 'rgba(0,0,0,0.85)';
      ctx.shadowBlur = 4;
    } else {
      ctx.shadowBlur = 0;
      // Normal state: dark background with colored border and text
      ctx.fillStyle = '#0a0a12';
      ctx.fillRect(0, 0, width, height);

      if (binding || isBlackoutKey) {
        // Top accent bar
        ctx.fillStyle = baseColor;
        ctx.fillRect(0, 0, width, 6);

        // Frame border
        ctx.strokeStyle = baseColor;
        ctx.lineWidth = 2;
        ctx.strokeRect(1, 1, width - 2, height - 2);

        ctx.fillStyle = '#ffffff';
      } else {
        // Empty pad border
        ctx.strokeStyle = '#222233';
        ctx.lineWidth = 1;
        ctx.strokeRect(1, 1, width - 2, height - 2);
        ctx.fillStyle = '#444455';
      }
    }

    // 2026-08-27: small corner readout (destination page number on the two
    // bottom-left nav keys). Drawn before any of the label/bigText/thumbnail
    // branches below because several of them return early with the finished
    // blob -- this has to land on the canvas first so it survives whichever
    // branch runs.
    if (binding && binding.corner) {
      ctx.save();
      ctx.font = 'bold 15px sans-serif';
      ctx.textBaseline = 'top';
      if (binding.cornerPos === 'tr') {
        ctx.textAlign = 'right';
        ctx.fillStyle = '#ffffff';
        ctx.shadowColor = 'rgba(0,0,0,0.85)';
        ctx.shadowBlur = 3;
        ctx.fillText(String(binding.corner), width - 8, 6);
      } else {
        ctx.textAlign = 'left';
        ctx.fillStyle = '#ffffff';
        ctx.shadowColor = 'rgba(0,0,0,0.85)';
        ctx.shadowBlur = 3;
        ctx.fillText(String(binding.corner), 8, 6);
      }
      ctx.restore();
    }

    // Render Label text
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    if (bigText) {
      // Big centred glyph: page numbers and warnings must read at arm's
      // length in the dark, so the word label is dropped entirely (or drawn small at top if provided).
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      if (label) {
        ctx.font = 'bold 15px sans-serif';
        ctx.fillText(label, width / 2, 15);
      }
      ctx.font = 'bold ' + (bigText.length > 1 ? 40 : 52) + 'px sans-serif';
      ctx.fillText(bigText, width / 2, height / 2 + 8);
      return new Promise((resolve) => {
        this.canvas.toBlob((blob) => resolve(blob), 'image/jpeg', 0.9);
      });
    }

    if (label) {
      // Draw the thumbnail first, dimmed, so overlaid text stays readable.
      if (thumbSrc && this._thumbCache) {
        const img = this._thumbCache.get(thumbSrc);
        if (img && img.complete && img.naturalWidth > 0) {
          try {
            ctx.drawImage(img, 0, 0, width, height);
            ctx.fillStyle = isActive ? 'rgba(5,255,161,0.30)' : 'rgba(0,0,0,0.42)';
            ctx.fillRect(0, 0, width, height);
            ctx.strokeStyle = isActive ? '#05ffa1' : baseColor;
            ctx.lineWidth = isActive ? 6 : 3;
            ctx.strokeRect(1.5, 1.5, width - 3, height - 3);
            ctx.fillStyle = '#ffffff';
            ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
            ctx.shadowColor = 'rgba(0,0,0,0.9)'; ctx.shadowBlur = 6;
            ctx.shadowBlur = 0;
            return new Promise((resolve) => {
              this.canvas.toBlob((blob) => resolve(blob), 'image/jpeg', 0.85);
            });
          } catch (e) { /* fall through to text rendering */ }
        }
      }

      ctx.font = 'bold 17px sans-serif';
      // Word wrap if label contains space
      const parts = label.split(' ');
      if (parts.length > 1 && label.length > 8) {
        ctx.fillText(parts[0], width / 2, height / 2 - 10);
        ctx.font = 'bold 15px sans-serif';
        ctx.fillText(parts.slice(1).join(' '), width / 2, height / 2 + 11);
      } else {
        ctx.fillText(label, width / 2, height / 2 - 2);
      }
    } else {
      ctx.font = 'bold 15px monospace';
      ctx.fillText(String(keyIndex), width / 2, height / 2);
    }

    // Render sublabel / type badge
    if (sublabel) {
      ctx.font = 'bold 11px sans-serif';
      ctx.globalAlpha = 0.75;
      ctx.fillText(sublabel, width / 2, height - 12);
      ctx.globalAlpha = 1.0;
    }

    return new Promise((resolve) => {
      this.canvas.toBlob((blob) => {
        resolve(blob);
      }, 'image/jpeg', 0.9);
    });
  }
  /** Load (and cache) one thumbnail, resolving once it is actually decodable. */
  loadThumb(src) {
    if (!this._thumbCache) this._thumbCache = new Map();
    const hit = this._thumbCache.get(src);
    if (hit && hit.complete && hit.naturalWidth > 0) return Promise.resolve(hit);
    if (this._thumbPending && this._thumbPending.has(src)) return this._thumbPending.get(src);
    if (!this._thumbPending) this._thumbPending = new Map();

    const img = hit || new Image();
    const pr = new Promise((resolve) => {
      let done = false;
      const finish = () => { if (!done) { done = true; resolve(img); } };
      img.onerror = finish;                 // a missing thumb must not hang a key
      // decode() resolves only when the bitmap is genuinely ready to draw.
      // onload alone fires before decode on some builds, and a bare timeout
      // resolved images that had not finished -- which is why whole rows came
      // up as text while others were fine.
      img.onload = () => {
        if (img.decode) img.decode().then(finish).catch(finish);
        else finish();
      };
      setTimeout(finish, 8000);             // last-resort escape, not a race
      if (!img.src) img.src = src;
      if (img.complete && img.naturalWidth > 0) {
        if (img.decode) img.decode().then(finish).catch(finish); else finish();
      }
    });
    this._thumbCache.set(src, img);
    this._thumbPending.set(src, pr);
    return pr;
  }

  /** True once this thumbnail is decoded and safe to draw. */
  thumbReady(src) {
    const img = this._thumbCache && this._thumbCache.get(src);
    return !!(img && img.complete && img.naturalWidth > 0);
  }

  /** Decode every thumbnail referenced by the current show, in parallel. */
  async preloadThumbs() {
    const srcs = new Set();
    for (const b of (this.keyBindings || [])) {
      if (b && b.thumb) srcs.add(b.thumb);
    }
    for (const bank of ((this.showData && this.showData.banks) || [])) {
      for (const pad of (bank.pads || [])) if (pad && pad.thumb) srcs.add(pad.thumb);
    }
    if (!srcs.size) return 0;
    await Promise.all([...srcs].map((s) => this.loadThumb(s)));
    return srcs.size;
  }

}
