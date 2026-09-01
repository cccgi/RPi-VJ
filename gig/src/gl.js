// WebGL2 Fullscreen-Quad Shader Host for RPi-VJ
// Zero dependencies, ES module.

const VERTEX_SHADER_SRC = `#version 300 es
in vec2 a_pos;
void main(){ gl_Position = vec4(a_pos, 0.0, 1.0); }
`;

export class ShaderHost {
  /**
   * @param {HTMLCanvasElement} canvas
   * @param {{ scale?: number }} [options]
   */
  constructor(canvas, { scale = 1.0 } = {}) {
    if (!canvas) {
      throw new Error('ShaderHost requires a canvas element');
    }
    this.canvas = canvas;
    this.scale = scale;
    this.width = canvas.width || 300;
    this.height = canvas.height || 150;

    /** @type {WebGL2RenderingContext|null} */
    this.gl = null;
    /** @type {WebGLProgram|null} */
    this.currentProgram = null;
    /** @type {WebGLVertexArrayObject|null} */
    this.vao = null;
    /** @type {WebGLBuffer|null} */
    this.vbo = null;

    /** @type {WeakMap<WebGLProgram, Map<string, {location: WebGLUniformLocation, type: number, size: number, texUnit?: number}>>} */
    this.programUniforms = new WeakMap();

    /** @type {number[]} Frame time deltas in ms (rolling window of up to 60) */
    this._frameDeltas = [];
    this._lastRenderTime = 0;

    // Track context lost state
    this._isContextLost = false;

    // Bind event handlers for context loss/restoration
    this._onContextLost = this._onContextLost.bind(this);
    this._onContextRestored = this._onContextRestored.bind(this);

    this.canvas.addEventListener('webglcontextlost', this._onContextLost, false);
    this.canvas.addEventListener('webglcontextrestored', this._onContextRestored, false);

    this._initGL();
  }

  _onContextLost(e) {
    e.preventDefault();
    this._isContextLost = true;
    this.gl = null;
    this.vao = null;
    this.vbo = null;
    this.currentProgram = null;

    // A LOST CONTEXT MUST STOP THE HEARTBEAT.
    //
    // When Chromium's GPU process dies the page itself survives: rAF keeps
    // firing, mixer keeps stamping window.lastFrameAt at the top of every
    // frame, and every GL call below silently no-ops. The heartbeat therefore
    // kept reporting health while the TV showed nothing, and the watchdog had
    // no reason to act. Flag it globally so show.js stops beating and the
    // watchdog restarts the kiosk -- a proven recovery path, unlike an
    // in-page reload, which usually fails once the GPU process is gone.
    if (typeof window !== 'undefined') {
      window.__vjContextLost = true;
      if (window.VJ_DIAG) window.VJ_DIAG('WEBGL CONTEXT LOST - heartbeat stopped, expect kiosk restart');
    }
  }

  _onContextRestored() {
    this._isContextLost = false;
    if (typeof window !== 'undefined') {
      window.__vjContextLost = false;
      if (window.VJ_DIAG) window.VJ_DIAG('WEBGL CONTEXT RESTORED');
    }
    this._initGL();
  }

  _initGL() {
    const gl = this.canvas.getContext('webgl2', {
      alpha: false,
      antialias: false,
      depth: false,
      stencil: false,
      powerPreference: 'high-performance',
      preserveDrawingBuffer: false,
    });

    if (!gl) {
      return;
    }

    this.gl = gl;

    // Create fullscreen quad: two triangles covering [-1, 1]^2
    // Positions: (-1, -1), (1, -1), (-1, 1), (-1, 1), (1, -1), (1, 1)
    const positions = new Float32Array([
      -1, -1,
       1, -1,
      -1,  1,
      -1,  1,
       1, -1,
       1,  1,
    ]);

    this.vao = gl.createVertexArray();
    gl.bindVertexArray(this.vao);

    this.vbo = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, this.vbo);
    gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);

    // a_pos is attribute location 0 by default or via layout/getAttribLocation
    gl.enableVertexAttribArray(0);
    gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);

    gl.bindVertexArray(null);
    gl.bindBuffer(gl.ARRAY_BUFFER, null);

    // Update viewport/size
    this.resize(this.width, this.height);
  }

  /**
   * Compiles a fragment shader with the fixed vertex shader.
   * NEVER throws. A bad shader must not kill the show.
   * @param {string} fragSrc
   * @returns {{ok: true, program: WebGLProgram} | {ok: false, log: string}}
   */
  compile(fragSrc) {
    if (!this.gl || this._isContextLost) {
      return { ok: false, log: 'WebGL2 context not available' };
    }

    const gl = this.gl;

    // Vertex shader
    const vs = gl.createShader(gl.VERTEX_SHADER);
    gl.shaderSource(vs, VERTEX_SHADER_SRC);
    gl.compileShader(vs);
    if (!gl.getShaderParameter(vs, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(vs) || 'Vertex shader compilation failed';
      gl.deleteShader(vs);
      return { ok: false, log };
    }

    // Fragment shader
    const fs = gl.createShader(gl.FRAGMENT_SHADER);
    gl.shaderSource(fs, fragSrc);
    gl.compileShader(fs);
    if (!gl.getShaderParameter(fs, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(fs) || 'Fragment shader compilation failed';
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      return { ok: false, log };
    }

    // Program
    const program = gl.createProgram();
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);

    // Explicitly bind a_pos attribute to index 0 matching vertex array setup
    gl.bindAttribLocation(program, 0, 'a_pos');

    gl.linkProgram(program);

    // Shaders can be marked for deletion after linking
    gl.deleteShader(vs);
    gl.deleteShader(fs);

    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
      const log = gl.getProgramInfoLog(program) || 'Program link failed';
      gl.deleteProgram(program);
      return { ok: false, log };
    }

    // Cache active uniforms
    const uniformMap = new Map();
    const activeUniforms = gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS);
    let nextTexUnit = 0;

    for (let i = 0; i < activeUniforms; i++) {
      const info = gl.getActiveUniform(program, i);
      if (!info) continue;

      // In case uniform is an array, uniform name might end with '[0]'
      const name = info.name.replace(/\[0\]$/, '');
      const location = gl.getUniformLocation(program, info.name);
      if (location !== null) {
        const entry = {
          location,
          type: info.type,
          size: info.size,
        };
        if (info.type === gl.SAMPLER_2D || info.type === gl.SAMPLER_CUBE || info.type === gl.SAMPLER_2D_ARRAY) {
          entry.texUnit = nextTexUnit++;
        }
        uniformMap.set(name, entry);
      }
    }

    this.programUniforms.set(program, uniformMap);

    return { ok: true, program };
  }

  /**
   * Sets the active shader program.
   * @param {WebGLProgram} program
   */
  use(program) {
    if (!this.gl || this._isContextLost) return;
    this.currentProgram = program;
    this.gl.useProgram(program);
  }

  /**
   * Sets uniform values on the current program.
   * Silently ignores names absent from the program.
   * @param {Record<string, number | number[] | Float32Array | Int32Array | WebGLTexture>} obj
   */
  setUniforms(obj) {
    if (!this.gl || this._isContextLost || !this.currentProgram || !obj) return;

    const gl = this.gl;
    const uniformMap = this.programUniforms.get(this.currentProgram);
    if (!uniformMap) return;

    for (const [name, val] of Object.entries(obj)) {
      const entry = uniformMap.get(name);
      if (!entry) continue;

      const { location, type, texUnit } = entry;

      switch (type) {
        case gl.FLOAT:
          if (typeof val === 'number') {
            gl.uniform1f(location, val);
          } else if (Array.isArray(val) || val instanceof Float32Array) {
            gl.uniform1fv(location, val);
          }
          break;

        case gl.FLOAT_VEC2:
          if (Array.isArray(val) || val instanceof Float32Array) {
            gl.uniform2fv(location, val);
          }
          break;

        case gl.FLOAT_VEC3:
          if (Array.isArray(val) || val instanceof Float32Array) {
            gl.uniform3fv(location, val);
          }
          break;

        case gl.FLOAT_VEC4:
          if (Array.isArray(val) || val instanceof Float32Array) {
            gl.uniform4fv(location, val);
          }
          break;

        case gl.INT:
          if (typeof val === 'number') {
            gl.uniform1i(location, val);
          } else if (Array.isArray(val) || val instanceof Int32Array) {
            gl.uniform1iv(location, val);
          }
          break;

        case gl.INT_VEC2:
          if (Array.isArray(val) || val instanceof Int32Array) {
            gl.uniform2iv(location, val);
          }
          break;

        case gl.INT_VEC3:
          if (Array.isArray(val) || val instanceof Int32Array) {
            gl.uniform3iv(location, val);
          }
          break;

        case gl.INT_VEC4:
          if (Array.isArray(val) || val instanceof Int32Array) {
            gl.uniform4iv(location, val);
          }
          break;

        case gl.BOOL:
          if (typeof val === 'boolean' || typeof val === 'number') {
            gl.uniform1i(location, val ? 1 : 0);
          } else if (Array.isArray(val) || val instanceof Int32Array) {
            gl.uniform1iv(location, val);
          }
          break;

        case gl.BOOL_VEC2:
          if (Array.isArray(val) || val instanceof Int32Array) {
            gl.uniform2iv(location, val);
          }
          break;

        case gl.BOOL_VEC3:
          if (Array.isArray(val) || val instanceof Int32Array) {
            gl.uniform3iv(location, val);
          }
          break;

        case gl.BOOL_VEC4:
          if (Array.isArray(val) || val instanceof Int32Array) {
            gl.uniform4iv(location, val);
          }
          break;

        case gl.SAMPLER_2D:
          if (texUnit !== undefined) {
            gl.activeTexture(gl.TEXTURE0 + texUnit);
            if (val instanceof WebGLTexture) {
              gl.bindTexture(gl.TEXTURE_2D, val);
            } else if (val === null) {
              gl.bindTexture(gl.TEXTURE_2D, null);
            }
            gl.uniform1i(location, texUnit);
          }
          break;

        default:
          // For unhandled types or matrices if needed, attempt generic handling if array
          if (Array.isArray(val) || val instanceof Float32Array) {
            if (type === gl.FLOAT_MAT2) gl.uniformMatrix2fv(location, false, val);
            else if (type === gl.FLOAT_MAT3) gl.uniformMatrix3fv(location, false, val);
            else if (type === gl.FLOAT_MAT4) gl.uniformMatrix4fv(location, false, val);
          }
          break;
      }
    }
  }

  /**
   * Draws the fullscreen quad and updates fps history.
   */
  render() {
    if (!this.gl || this._isContextLost || !this.vao) return;

    const now = (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();
    if (this._lastRenderTime > 0) {
      const delta = now - this._lastRenderTime;
      if (delta > 0) {
        this._frameDeltas.push(delta);
        if (this._frameDeltas.length > 60) {
          this._frameDeltas.shift();
        }
      }
    }
    this._lastRenderTime = now;

    const gl = this.gl;
    gl.bindVertexArray(this.vao);
    gl.drawArrays(gl.TRIANGLES, 0, 6);
    gl.bindVertexArray(null);
  }

  /**
   * Resizes backing store to CSS size * scale, rounded.
   * @param {number} cssW
   * @param {number} cssH
   */
  resize(cssW, cssH) {
    this.width = cssW;
    this.height = cssH;
    const targetW = Math.max(1, Math.round(cssW * this.scale));
    const targetH = Math.max(1, Math.round(cssH * this.scale));

    if (this.canvas.width !== targetW || this.canvas.height !== targetH) {
      this.canvas.width = targetW;
      this.canvas.height = targetH;
    }

    if (this.gl && !this._isContextLost) {
      this.gl.viewport(0, 0, targetW, targetH);
    }
  }

  /**
   * Live-adjustable scale; reallocates backing store buffers.
   * @param {number} s
   */
  setScale(s) {
    this.scale = s;
    this.resize(this.width, this.height);
  }

  /**
   * Median of last 60 frame times, expressed as frames per second.
   * @returns {number}
   */
  get fps() {
    if (this._frameDeltas.length === 0) return 0;
    const sorted = [...this._frameDeltas].sort((a, b) => a - b);
    const mid = Math.floor(sorted.length / 2);
    const medianDeltaMs = sorted.length % 2 === 0
      ? (sorted[mid - 1] + sorted[mid]) / 2
      : sorted[mid];

    if (medianDeltaMs <= 0) return 0;
    return 1000 / medianDeltaMs;
  }
}
