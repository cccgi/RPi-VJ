// Deck: Dual Video Deck -> WebGL Texture for RPi-VJ
// ES Module, zero dependencies.

export class Deck {
  /**
   * @param {WebGL2RenderingContext} gl
   * @param {'A' | 'B'} id
   */
  constructor(gl, id) {
    if (!gl) {
      throw new Error('Deck requires a WebGL2RenderingContext');
    }
    this.gl = gl;
    this.id = id;

    this.video = document.createElement('video');
    // Clip audio is NEVER wanted: it would reach the PA over HDMI, and it must
    // never influence the analyser. Only the booth feed drives the effects.
    this.video.muted = true;
    this.video.defaultMuted = true;
    this.video.volume = 0;
    this.video.setAttribute('muted', '');
    this.video.disableRemotePlayback = true;
    this.video.playsInline = true;
    this.video.loop = true;
    // preload='auto' told Chromium to buffer as much of the file as it liked.
    // The clip library is 5-10 minute loops of 230-290MB each, on a 2GB Pi.
    // Pressing a clip therefore asked for a multi-hundred-megabyte buffer fill
    // in one go, which is why the crash reproduced as "idle, press a clip,
    // Aw Snap" -- a renderer OOM, not a random fault.
    // 'metadata' still starts playback promptly from a localhost server.
    this.video.preload = 'metadata';
    this.video.crossOrigin = 'anonymous';
    this.video.style.display = 'none';
    document.body.appendChild(this.video);

    /** @type {WebGLTexture|null} */
    this._texture = null;
    this._hasNewFrame = false;
    this._rvfcCallbackId = null;
    this._lastUploadedTime = -1;
    this._supportsRVFC = typeof this.video.requestVideoFrameCallback === 'function';
    this._ready = false;
    this._loadGeneration = 0;
    this._pendingLoad = null;
    this._texWidth = 0;
    this._texHeight = 0;

    this._setupRVFC();
  }

  _setupRVFC() {
    if (this._supportsRVFC) {
      const onFrame = () => {
        this._hasNewFrame = true;
        if (this.video) {
          this._rvfcCallbackId = this.video.requestVideoFrameCallback(onFrame);
        }
      };
      this._rvfcCallbackId = this.video.requestVideoFrameCallback(onFrame);
    }
  }

  /**
   * Loads a video URL and returns a Promise that resolves when the first frame is decodable.
   * @param {string} url
   * @returns {Promise<void>}
   */
  async load(url) {
    const generation = ++this._loadGeneration;
    if (this._pendingLoad) {
      try { this._pendingLoad.reject(new DOMException('Superseded by newer clip', 'AbortError')); } catch (_) {}
      try { this._pendingLoad.cleanup(); } catch (_) {}
      this._pendingLoad = null;
    }
    this._ready = false;
    this._hasNewFrame = false;
    this._lastUploadedTime = -1;

    return new Promise((resolve, reject) => {
      let settled = false;

      let timer = null;
      const cleanup = () => {
        this.video.removeEventListener('loadeddata', onLoadedData);
        this.video.removeEventListener('error', onError);
        if (timer !== null) clearTimeout(timer);
      };

      const onLoadedData = () => {
        if (settled || generation !== this._loadGeneration) return;
        settled = true;
        cleanup();
        this._pendingLoad = null;
        this._ready = true;
        this._hasNewFrame = true;
        this._ensureTexture();
        this._uploadTexture();
        resolve();
      };

      const onError = (e) => {
        if (settled || generation !== this._loadGeneration) return;
        settled = true;
        cleanup();
        this._pendingLoad = null;
        const err = this.video.error;
        reject(new Error(`Deck ${this.id} failed to load video: ${url} (${err ? err.message : 'unknown error'})`));
      };

      this.video.addEventListener('loadeddata', onLoadedData);
      this.video.addEventListener('error', onError);
      this._pendingLoad = { reject, cleanup };
      timer = setTimeout(() => {
        if (settled || generation !== this._loadGeneration) return;
        settled = true;
        cleanup();
        this._pendingLoad = null;
        reject(new Error(`Deck ${this.id} timed out loading video: ${url}`));
      }, 12000);

      // RELEASE THE PREVIOUS CLIP FIRST.
      // Assigning a new src does not promptly free the old buffered ranges;
      // clearing the source and calling load() does. Without this, switching
      // clips stacked one large buffer on top of another until the renderer
      // died.
      try {
        if (this.video.src) {
          this.video.pause();
          this.video.removeAttribute('src');
          this.video.load();
        }
      } catch (_) {}

      this.video.src = url;
      this.video.load();
    });
  }

  _ensureTexture() {
    if (!this._texture && this.gl) {
      const gl = this.gl;
      this._texture = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, this._texture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.bindTexture(gl.TEXTURE_2D, null);
    }
  }

  _uploadTexture() {
    if (!this.gl || !this._texture || !this.video || this.video.readyState < 2) {
      return;
    }
    const gl = this.gl;
    gl.bindTexture(gl.TEXTURE_2D, this._texture);
    const w = this.video.videoWidth | 0;
    const h = this.video.videoHeight | 0;
    if (w <= 0 || h <= 0) { gl.bindTexture(gl.TEXTURE_2D, null); return; }
    // Allocate only when dimensions change, then update in-place. The former
    // texImage2D(video) path re-specified texture storage every video frame.
    if (this._texWidth !== w || this._texHeight !== h) {
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
      this._texWidth = w;
      this._texHeight = h;
    }
    gl.texSubImage2D(gl.TEXTURE_2D, 0, 0, 0, gl.RGBA, gl.UNSIGNED_BYTE, this.video);
    gl.bindTexture(gl.TEXTURE_2D, null);
    this._hasNewFrame = false;
    this._lastUploadedTime = this.video.currentTime;
  }

  /**
   * Starts playback.
   * @returns {Promise<void>}
   */
  async play() {
    try {
      // stop() clears _ready; restore it when we start again.
      if (this.video.readyState >= 2) this._ready = true;
      await this.video.play();
      this._ready = this.video.readyState >= 2;
    } catch (err) {
      // Browser autoplay policy or abort
      console.warn(`Deck ${this.id} play() failed:`, err);
    }
  }

  /**
   * Pauses playback.
   */
  pause() {
    this.video.pause();
  }

  /**
   * Stops playback and resets to beginning.
   */
  stop() {
    ++this._loadGeneration;
    if (this._pendingLoad) {
      try { this._pendingLoad.reject(new DOMException('Clip stopped', 'AbortError')); } catch (_) {}
      try { this._pendingLoad.cleanup(); } catch (_) {}
      this._pendingLoad = null;
    }
    this.video.pause();
    try { this.video.currentTime = 0; } catch (e) {}
    // CRITICAL: pausing does NOT clear the WebGL texture. `ready` stayed true
    // (readyState is still >=2 with data buffered), so the mixer kept drawing
    // the last uploaded frame forever -- a stopped clip remained frozen on
    // screen and RESET could not remove it. Drop out of ready and blank the
    // texture so the mixer falls back to its black fallback.
    this._ready = false;
    this._hasNewFrame = false;
    this._lastUploadedTime = -1;
    this.clearTexture();
  }

  /** Overwrite the texture with a single black pixel. */
  clearTexture() {
    const gl = this.gl;
    if (!gl || !this._texture) return;
    try {
      gl.bindTexture(gl.TEXTURE_2D, this._texture);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGB, 1, 1, 0, gl.RGB, gl.UNSIGNED_BYTE,
        new Uint8Array([0, 0, 0]));
      this._texWidth = 1;
      this._texHeight = 1;
      gl.bindTexture(gl.TEXTURE_2D, null);
    } catch (e) {}
  }

  /**
   * Sets looping behavior.
   * @param {boolean} bool
   */
  set loop(bool) {
    this.video.loop = Boolean(bool);
  }

  /**
   * Gets looping behavior.
   * @returns {boolean}
   */
  get loop() {
    return this.video.loop;
  }

  /**
   * Returns WebGLTexture, or null if nothing loaded.
   * @returns {WebGLTexture|null}
   */
  get texture() {
    return this._texture;
  }

  /**
   * Returns whether a presentable frame is ready.
   * @returns {boolean}
   */
  get ready() {
    return this._ready && this.video.readyState >= 2;
  }

  /**
   * Returns aspect ratio (width / height) or 1.0 default.
   * @returns {number}
   */
  get aspect() {
    if (this.video && this.video.videoHeight > 0) {
      return this.video.videoWidth / this.video.videoHeight;
    }
    return 1.0;
  }

  /**
   * Call once per frame; uploads current frame ONLY if new.
   */
  update() {
    if (!this._ready || !this.video || this.video.readyState < 2) {
      return;
    }

    let isNew = false;
    if (this._supportsRVFC) {
      isNew = this._hasNewFrame;
    } else {
      isNew = this.video.currentTime !== this._lastUploadedTime;
    }

    if (isNew) {
      this._ensureTexture();
      this._uploadTexture();
    }
  }

  /**
   * Cleans up video element and WebGL texture.
   */
  destroy() {
    ++this._loadGeneration;
    if (this._pendingLoad) {
      try { this._pendingLoad.reject(new DOMException('Deck destroyed', 'AbortError')); } catch (_) {}
      try { this._pendingLoad.cleanup(); } catch (_) {}
      this._pendingLoad = null;
    }
    if (this._supportsRVFC && this._rvfcCallbackId !== null && typeof this.video.cancelVideoFrameCallback === 'function') {
      this.video.cancelVideoFrameCallback(this._rvfcCallbackId);
      this._rvfcCallbackId = null;
    }
    this.video.pause();
    this.video.removeAttribute('src');
    this.video.load();
    if (this.video.parentNode) {
      this.video.parentNode.removeChild(this.video);
    }
    if (this.gl && this._texture) {
      this.gl.deleteTexture(this._texture);
      this._texture = null;
    }
    this._ready = false;
  }
}
