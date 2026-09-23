// gig/src/streamdeck.js
// WebHID Stream Deck XL Driver (G-08)
// Zero dependencies, ES module.

export const VENDOR_ID = 0x0fd9; // Elgato
export const PRODUCT_IDS = [
  0x006c, // Stream Deck XL
  0x008f, // Stream Deck XL V2
];

export const KEY_COUNT = 32;
export const ROWS = 4;
export const COLS = 8;
export const ICON_SIZE = 96; // 96 x 96 JPEG
export const REPORT_SIZE = 1024;
export const HEADER_SIZE = 8;
export const PAYLOAD_SIZE = REPORT_SIZE - HEADER_SIZE; // 1016 bytes
// WebHID's sendReport(reportId, data) prepends the report ID itself, so the
// DATA buffer excludes it: 1023 bytes, with a 7-byte header before the payload.
export const DATA_SIZE = REPORT_SIZE - 1;      // 1023
export const DATA_HEADER = HEADER_SIZE - 1;    // 7

// Helper to compute a fast 32-bit FNV-1a hash of a Uint8Array
function hashBytes(bytes) {
  if (!bytes || bytes.length === 0) return 0;
  let hash = 0x811c9dc5;
  for (let i = 0; i < bytes.length; i++) {
    hash ^= bytes[i];
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

export class StreamDeckXL {
  /**
   * Request user permission and select a Stream Deck XL device.
   * Must be called from a user gesture.
   * @returns {Promise<StreamDeckXL|null>}
   */
  static async request() {
    if (typeof navigator === 'undefined' || !navigator.hid) {
      return null;
    }
    const filters = PRODUCT_IDS.map((pid) => ({
      vendorId: VENDOR_ID,
      productId: pid,
    }));
    try {
      const devices = await navigator.hid.requestDevice({ filters });
      if (devices && devices.length > 0) {
        return new StreamDeckXL(devices[0]);
      }
    } catch (_) {}
    return null;
  }

  /**
   * Enumerate previously paired devices and return an instance if found.
   * Does not require a user gesture; suitable on reconnect/startup.
   * @returns {Promise<StreamDeckXL|null>}
   */
  static async getPaired() {
    if (typeof navigator === 'undefined' || !navigator.hid) {
      return null;
    }
    try {
      const devices = await navigator.hid.getDevices();
      for (const dev of devices) {
        if (dev.vendorId === VENDOR_ID && PRODUCT_IDS.includes(dev.productId)) {
          return new StreamDeckXL(dev);
        }
      }
    } catch (_) {}
    return null;
  }

  /**
   * @param {HIDDevice} [device]
   */
  constructor(device = null) {
    this.device = device;
    this.opened = false;
    this._listeners = new Set();

    // Key state tracking for edge triggering
    this._keyState = new Uint8Array(KEY_COUNT);

    // Icon caching and write queue
    this._iconHashes = new Uint32Array(KEY_COUNT);
    this._writeQueue = []; // Array of { index, jpegBytes, resolve, reject }
    this._isFlushingQueue = false;
    this._autoPollTimer = null;
    this._reconnectListeners = new Set();
    // Column mirroring was applied to INPUT only, never to setIcon output, so
    // labels landed on one key while presses reported its mirror (0<->7, 1<->6).
    // Stream Deck XL key 0 is top-left and sequential; no flip is needed.
    this._lastColFlip = false;

    this._onInputReport = this._onInputReport.bind(this);
    this._onDisconnect = this._onDisconnect.bind(this);

    if (typeof navigator !== 'undefined' && navigator.hid) {
      navigator.hid.addEventListener('disconnect', this._onDisconnect);
    }
  }

  /**
   * Set or get whether column mirroring is applied (col = 7 - col).
   * Default is true based on Stream Deck XL physical button mapping.
   */
  get columnMirror() {
    return this._lastColFlip;
  }

  set columnMirror(enabled) {
    this._lastColFlip = !!enabled;
  }

  /**
   * Register a key state change callback: cb(index: number, pressed: boolean).
   * Edge-triggered only.
   * @param {(index: number, pressed: boolean) => void} cb
   * @returns {() => void} Unsubscribe function
   */
  onKey(cb) {
    if (typeof cb === 'function') {
      this._listeners.add(cb);
    }
    return () => {
      this._listeners.delete(cb);
    };
  }

  /** Called after a physical reconnect has reopened the HID device. */
  onReconnect(cb) {
    if (typeof cb === 'function') this._reconnectListeners.add(cb);
    return () => this._reconnectListeners.delete(cb);
  }

  /**
   * Open the HID device and start listening for input reports.
   * @returns {Promise<boolean>}
   */
  async open() {
    if (!this.device) {
      const paired = await StreamDeckXL.getPaired();
      if (paired && paired.device) {
        this.device = paired.device;
      } else {
        return false;
      }
    }

    try {
      if (!this.device.opened) {
        await this.device.open();
      }
      // open() may be called by startup and again by reconnect. DOM event
      // listeners are not a reliable deduplication boundary across WebHID
      // implementations, so remove explicitly before adding.
      this.device.removeEventListener('inputreport', this._onInputReport);
      this.device.addEventListener('inputreport', this._onInputReport);
      if (typeof window !== 'undefined' && window.VJ_DIAG)
        window.VJ_DIAG('inputreport listener ATTACHED to "' + (this.device.productName || '?') + '"');
      this.opened = true;
      return true;
    } catch (_) {
      this.opened = false;
      return false;
    }
  }

  /**
   * Close the device and cleanup listeners.
   */
  close() {
    this.opened = false;
    if (this._autoPollTimer) {
      clearInterval(this._autoPollTimer);
      this._autoPollTimer = null;
    }
    if (this.device) {
      try {
        this.device.removeEventListener('inputreport', this._onInputReport);
      } catch (_) {}
      if (this.device.opened) {
        try {
          this.device.close().catch(() => {});
        } catch (_) {}
      }
    }
    if (typeof navigator !== 'undefined' && navigator.hid) {
      navigator.hid.removeEventListener('disconnect', this._onDisconnect);
    }
  }

  /**
   * Sets the icon on a specific key (0-31) using JPEG image bytes.
   * Queued and rate-limited (~2 keys per frame) to avoid choking the USB bus.
   * @param {number} index - Key index (0..31)
   * @param {Uint8Array} jpegBytes - JPEG image data
   * @returns {Promise<void>}
   */
  async setIcon(index, jpegBytes) {
    if (index < 0 || index >= KEY_COUNT) {
      return;
    }

    const hash = hashBytes(jpegBytes);
    if (this._iconHashes[index] === hash && hash !== 0) {
      // Content has not changed, skip transmission
      return;
    }

    return new Promise((resolve, reject) => {
      // Remove any existing pending write for this specific key
      const existingIdx = this._writeQueue.findIndex((item) => item.index === index);
      if (existingIdx >= 0) {
        // A queued write owns a Promise. Dropping it without settling that
        // Promise deadlocks its caller until an outer timeout and was the
        // direct cause of partial/black page repaints when two refreshes
        // overlapped. Supersession is success: the newer image is authoritative.
        const superseded = this._writeQueue.splice(existingIdx, 1)[0];
        try { superseded.resolve({ superseded: true }); } catch (_) {}
      }

      this._writeQueue.push({
        index,
        jpegBytes: jpegBytes ? new Uint8Array(jpegBytes) : null,
        hash,
        resolve,
        reject,
      });

      this._scheduleQueueFlush();
    });
  }

  /**
   * Clears the icon for a key (sets to black / empty).
   * @param {number} index - Key index (0..31)
   * @returns {Promise<void>}
   */
  async clear(index) {
    if (index < 0 || index >= KEY_COUNT) {
      return;
    }
    return this.setIcon(index, null);
  }

  /** Mark cached key images dirty without clearing the hardware first. */
  invalidateIcons() {
    this._iconHashes.fill(0);
  }

  /** Resolve and discard writes that have not started; used before a new page. */
  supersedePendingWrites() {
    const pending = this._writeQueue.splice(0);
    for (const item of pending) {
      try { item.resolve({ superseded: true }); } catch (_) {}
    }
    return pending.length;
  }

  /**
   * Sets the global panel brightness (0 - 100%).
   * Command report: 0x03, 0x08, pct.
   * @param {number} pct - Brightness percentage (0 to 100)
   * @returns {Promise<void>}
   */
  async setBrightness(pct) {
    if (!this.opened || !this.device) return;
    const clamped = Math.max(0, Math.min(100, Math.round(pct)));
    const report = new Uint8Array(32);
    report[0] = 0x08; // Set Brightness command
    report[1] = clamped;

    try {
      await this.device.sendFeatureReport(0x03, report);
    } catch (_) {
      try {
        await this.device.sendReport(0x03, report);
      } catch (_) {}
    }
  }

  /**
   * Resets the Stream Deck XL to initial state and clears all keys.
   * @returns {Promise<void>}
   */
  async reset() {
    this._writeQueue = [];
    this._iconHashes.fill(0);

    if (this.opened && this.device) {
      // Send reset command (Feature report 0x03, payload 0x02)
      const resetReport = new Uint8Array(32);
      resetReport[0] = 0x02;
      try {
        await this.device.sendFeatureReport(0x03, resetReport);
      } catch (_) {
        try {
          await this.device.sendReport(0x03, resetReport);
        } catch (_) {}
      }
    }

    // Also issue clear for all 32 keys
    for (let i = 0; i < KEY_COUNT; i++) {
      await this.clear(i);
    }
  }

  // ---- Internal report sending & queue management ----

  _scheduleQueueFlush() {
    if (this._isFlushingQueue) return;
    this._isFlushingQueue = true;

    // Flush in animation frames / microtasks, cap at ~2 keys per frame (~16ms)
    const run = async () => {
      if (!this.opened || !this.device) {
        this._isFlushingQueue = false;
        return;
      }

      const batchLimit = 2;
      let count = 0;

      while (this._writeQueue.length > 0 && count < batchLimit) {
        const item = this._writeQueue.shift();
        try {
          await this._sendImageReports(item.index, item.jpegBytes);
          this._iconHashes[item.index] = item.hash;
          item.resolve();
        } catch (err) {
          item.reject(err);
        }
        count++;
      }

      if (this._writeQueue.length > 0) {
        // HID recovery must not depend on the graphics frame loop. If the GPU
        // or rAF is stalled, the Stream Deck is the operator's recovery UI.
        setTimeout(run, 8);
      } else {
        this._isFlushingQueue = false;
      }
    };

    setTimeout(run, 0);
  }

  /**
   * Split JPEG data across 1024-byte reports and transmit.
   * @param {number} keyIndex
   * @param {Uint8Array|null} jpegBytes
   */
  async _sendImageReports(keyIndex, jpegBytes) {
    if (!this.opened || !this.device) return;

    if (!jpegBytes || jpegBytes.length === 0) {
      // Send 1 empty payload chunk to clear key image
      const report = new Uint8Array(DATA_SIZE);
      report[0] = 0x07; // Set Image  (report ID 0x02 is passed separately)
      report[1] = keyIndex;
      report[2] = 0x01; // Is last chunk
      report[3] = 0;    // Length low
      report[4] = 0;    // Length high
      report[5] = 0;    // Seq low
      report[6] = 0;    // Seq high

      await this.device.sendReport(0x02, report);
      return;
    }

    const totalLen = jpegBytes.length;
    let offset = 0;
    let chunkSeq = 0;

    while (offset < totalLen) {
      const remaining = totalLen - offset;
      const chunkSize = Math.min(DATA_SIZE - DATA_HEADER, remaining); // 1016
      const isLast = (offset + chunkSize >= totalLen) ? 0x01 : 0x00;

      const report = new Uint8Array(DATA_SIZE);
      report[0] = 0x07; // Set Image  (report ID 0x02 passed separately)
      report[1] = keyIndex;
      report[2] = isLast;
      report[3] = chunkSize & 0xff;
      report[4] = (chunkSize >> 8) & 0xff;
      report[5] = chunkSeq & 0xff;
      report[6] = (chunkSeq >> 8) & 0xff;

      report.set(jpegBytes.subarray(offset, offset + chunkSize), DATA_HEADER);

      try {
        await this.device.sendReport(0x02, report);
      } catch (err) {
        if (typeof window !== 'undefined' && window.VJ_DIAG)
          window.VJ_DIAG('sendReport failed key ' + keyIndex + ' seq ' + chunkSeq + ': ' + err.message);
        // Never mark a failed image as cached. The caller/reconnect path must
        // be able to retry it instead of preserving a blank key forever.
        throw err;
      }

      offset += chunkSize;
      chunkSeq++;
    }
  }

  // ---- Input report handler ----

  _onInputReport(event) {
    const { data } = event;

    // FIRST THING: unconditional dump. Any filtering above this hides the very
    // information we need when the mapping is wrong.
    if (typeof window !== 'undefined' && window.VJ_DIAG) {
      if (this._rawDumps === undefined) this._rawDumps = 14;
      if (this._rawDumps > 0) {
        this._rawDumps--;
        if (!data) {
          window.VJ_DIAG('HIDRAW reportId=' + event.reportId + ' data=NULL');
        } else {
          const hot = [];
          for (let b = 0; b < data.byteLength; b++) {
            const v = data.getUint8(b);
            if (v !== 0) hot.push(b + ':' + v);
          }
          window.VJ_DIAG('HIDRAW id=' + event.reportId + ' len=' + data.byteLength +
                         ' nonzero=[' + (hot.join(' ') || 'none') + ']');
        }
      }
    }

    if (!data) return;

    // Stream Deck XL input report: 4 byte header followed by 32 button state bytes (0 or 1)
    // WebHID's event.data EXCLUDES the report ID, so a hardcoded 4-byte header
    // skipped one byte too many and every key read as its neighbour (which also
    // made BLACK land on RESET). Derive the offset from the actual length so it
    // is correct whether or not the ID is present.
    // MEASURED on the actual device (2026-08-25), not assumed:
    //   HIDRAW id=1 len=511 nonzero=[1:32 27:1]
    // The report is 511 bytes, NOT 36. Layout after WebHID strips the report
    // ID:  [0]=0x00  [1]=0x20 (32 = key count)  [2]=0x00  [3..34]=key states.
    // Deriving the offset from byteLength gave 479 and read 32 always-zero
    // bytes off the end, so every press was silently discarded.
    let offset = 3;
    if (data.byteLength >= 2 && data.getUint8(1) !== KEY_COUNT) {
      // Layout differs from the measured one; fall back to a scan for the
      // key-count marker rather than guessing.
      let found = -1;
      for (let b = 0; b < Math.min(8, data.byteLength); b++) {
        if (data.getUint8(b) === KEY_COUNT) { found = b; break; }
      }
      offset = found >= 0 ? found + 2 : 3;
      if (typeof window !== 'undefined' && window.VJ_DIAG && !this._offWarn) {
        this._offWarn = true;
        window.VJ_DIAG('key-count marker not at byte 1; using offset ' + offset);
      }
    }
    if (data.byteLength < offset + KEY_COUNT) {
      if (typeof window !== 'undefined' && window.VJ_DIAG && !this._shortWarn) {
        this._shortWarn = true;
        window.VJ_DIAG('report too short: len=' + data.byteLength + ' needs ' + (offset + KEY_COUNT));
      }
      return;
    }



    for (let rawIdx = 0; rawIdx < KEY_COUNT; rawIdx++) {
      const rawVal = data.getUint8(offset + rawIdx) !== 0 ? 1 : 0;

      // Handle physical panel mirroring if enabled:
      // Stream Deck XL rows = 4, cols = 8.
      // row = floor(rawIdx / 8), col = rawIdx % 8
      let logicalIdx = rawIdx;
      if (this._lastColFlip) {
        const row = Math.floor(rawIdx / COLS);
        const col = rawIdx % COLS;
        const flippedCol = (COLS - 1) - col;
        logicalIdx = row * COLS + flippedCol;
      }

      const prev = this._keyState[logicalIdx];
      if (prev !== rawVal) {
        this._keyState[logicalIdx] = rawVal;
        const isPressed = rawVal === 1;
        if (isPressed && typeof window !== 'undefined' && window.VJ_DIAG)
          window.VJ_DIAG('KEY EMIT -> index ' + logicalIdx + ' (raw slot ' + rawIdx + ')');
        this._emitKey(logicalIdx, isPressed);
      }
    }
  }

  _emitKey(index, pressed) {
    for (const listener of this._listeners) {
      try {
        listener(index, pressed);
      } catch (_) {}
    }
  }

  // ---- Disconnect & Auto-reconnect handling ----

  _onDisconnect(event) {
    if (this.device && event.device === this.device) {
      this.opened = false;
      this._keyState.fill(0);
      this._startAutoReconnect();
    }
  }

  _startAutoReconnect() {
    if (this._autoPollTimer) return;
    this._autoPollTimer = setInterval(async () => {
      if (this.opened) {
        clearInterval(this._autoPollTimer);
        this._autoPollTimer = null;
        return;
      }
      try {
        const paired = await StreamDeckXL.getPaired();
        if (paired && paired.device) {
          this.device = paired.device;
          const ok = await this.open();
          if (ok) {
            clearInterval(this._autoPollTimer);
            this._autoPollTimer = null;
            // Mark all cached hashes dirty so reconnecting repopulates
            this._iconHashes.fill(0);
            for (const cb of this._reconnectListeners) {
              try { cb(); } catch (_) {}
            }
          }
        }
      } catch (_) {}
    }, 2000);
  }
}
