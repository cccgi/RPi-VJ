/**
 * gig/src/post.js — Post-processing chain for RPi-VJ
 * 
 * Pipeline order:
 *   inputTexture -> [1 feedback/trails] -> [2 bloom (1/4 res, RGBA8)] -> [3 finish: CA+grain+vignette] -> target/screen
 *
 * Measured hardware constraints:
 *   - OES_texture_float_linear is NOT available on Pi 5.
 *   - All FBOs and blur chains MUST use RGBA8 with LINEAR filtering.
 *   - Total post budget ~5ms at 1080p.
 *   - Quality tiers: 'off' | 'light' (feedback + finish) | 'full' (all).
 */

const FULLSCREEN_VS = `#version 300 es
in vec2 a_position;
out vec2 v_uv;
void main() {
    v_uv = a_position * 0.5 + 0.5;
    gl_Position = vec4(a_position, 0.0, 1.0);
}
`;

const FEEDBACK_FS = `#version 300 es
precision highp float;

uniform sampler2D u_current;
uniform sampler2D u_prev;
uniform vec2 u_resolution;
uniform float u_decay;      // 0.85 .. 0.98 (clamped)
uniform float u_zoom;       // ~0.002 .. 0.02
uniform float u_rotation;   // slow angle in radians
uniform int u_mode;         // 0: max, 1: add, 2: mix

in vec2 v_uv;
out vec4 fragColor;

vec2 rotateUV(vec2 uv, float angle) {
    float s = sin(angle);
    float c = cos(angle);
    return vec2(uv.x * c - uv.y * s, uv.x * s + uv.y * c);
}

void main() {
    vec4 current = texture(u_current, v_uv);
    
    // Zoom & rotation around center (0.5, 0.5)
    vec2 centered = v_uv - 0.5;
    centered *= (1.0 - u_zoom);
    centered = rotateUV(centered, u_rotation);
    vec2 prevUV = centered + 0.5;
    
    vec4 prev = vec4(0.0);
    if (prevUV.x >= 0.0 && prevUV.x <= 1.0 && prevUV.y >= 0.0 && prevUV.y <= 1.0) {
        prev = texture(u_prev, prevUV);
    }
    
    // Clamp decay based on mode to prevent runaway whiteouts
    float safeDecay = u_decay;
    if (u_mode == 1) { // add mode
        safeDecay = min(u_decay, 0.92);
    } else {
        safeDecay = min(u_decay, 0.98);
    }
    
    vec3 prevDecayed = prev.rgb * safeDecay;
    vec3 outColor;
    
    if (u_mode == 0) {
        // max() keeps trails bright
        outColor = max(current.rgb, prevDecayed);
    } else if (u_mode == 1) {
        // add
        outColor = current.rgb + prevDecayed;
    } else {
        // mix / blend
        outColor = mix(current.rgb, prevDecayed, safeDecay * 0.5);
    }
    
    fragColor = vec4(outColor, 1.0);
}
`;

const BLOOM_THRESHOLD_DOWNSAMPLE_FS = `#version 300 es
precision highp float;

uniform sampler2D u_image;
uniform float u_threshold; // 0.6 .. 0.9

in vec2 v_uv;
out vec4 fragColor;

void main() {
    vec4 color = texture(u_image, v_uv);
    float luma = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    
    // Soft knee threshold
    float knee = 0.1;
    float soft = clamp((luma - u_threshold + knee) / (2.0 * knee), 0.0, 1.0);
    float factor = max(soft * soft, step(u_threshold, luma));
    
    vec3 bright = color.rgb * factor;
    fragColor = vec4(bright, 1.0);
}
`;

// 9-tap separable Gaussian blur
const BLUR_9TAP_FS = `#version 300 es
precision highp float;

uniform sampler2D u_image;
uniform vec2 u_direction; // e.g. vec2(1.0/w, 0.0) or vec2(0.0, 1.0/h)
uniform float u_radius;

in vec2 v_uv;
out vec4 fragColor;

// Gaussian weights for 9 taps (sigma ~ 2.5)
const float weights[5] = float[5](0.227027, 0.1945946, 0.1216216, 0.054054, 0.016216);

void main() {
    vec2 stepOffset = u_direction * u_radius;
    vec3 result = texture(u_image, v_uv).rgb * weights[0];
    
    for (int i = 1; i < 5; i++) {
        vec2 off = stepOffset * float(i);
        result += texture(u_image, v_uv + off).rgb * weights[i];
        result += texture(u_image, v_uv - off).rgb * weights[i];
    }
    
    fragColor = vec4(result, 1.0);
}
`;

const FINISH_PASS_FS = `#version 300 es
precision highp float;

uniform sampler2D u_scene;
uniform sampler2D u_bloom;
uniform bool u_has_bloom;
uniform float u_bloom_intensity;

uniform vec2 u_resolution;
uniform float u_time;
uniform float u_kick;          // audio kick pulse 0.0 .. 1.0

// Chromatic Aberration
uniform float u_ca_amount;     // base CA offset

// Grain
uniform float u_grain_amount;  // 0.02 .. 0.06

// Vignette
uniform float u_vignette_amount; // 0.0 .. 1.0

in vec2 v_uv;
out vec4 fragColor;

// High quality pseudo-random hash
float hash(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

void main() {
    vec2 center = vec2(0.5);
    vec2 toCenter = v_uv - center;
    float dist = length(toCenter);
    
    // 1. Chromatic Aberration modulated by kick & radial distance
    float caOffset = (u_ca_amount + u_kick * 0.025) * dist;
    vec2 caDir = dist > 0.0001 ? normalize(toCenter) : vec2(0.0);
    
    float r = texture(u_scene, v_uv - caDir * caOffset).r;
    float g = texture(u_scene, v_uv).g;
    float b = texture(u_scene, v_uv + caDir * caOffset).b;
    vec3 color = vec3(r, g, b);
    
    // 2. Additive Bloom
    if (u_has_bloom) {
        vec3 bloomColor = texture(u_bloom, v_uv).rgb * u_bloom_intensity;
        color += bloomColor;
    }
    
    // 3. Film Grain
    if (u_grain_amount > 0.0) {
        float noise = hash(v_uv * u_resolution + fract(u_time * 17.13)) * 2.0 - 1.0;
        color += noise * u_grain_amount;
    }
    
    // 4. Vignette
    if (u_vignette_amount > 0.0) {
        float vig = 1.0 - smoothstep(0.4, 1.4, dist * (1.0 + u_vignette_amount * 0.5));
        color *= vig;
    }
    
    fragColor = vec4(clamp(color, 0.0, 1.0), 1.0);
}
`;

function createShader(gl, type, source) {
    const shader = gl.createShader(type);
    gl.shaderSource(shader, source);
    gl.compileShader(shader);
    if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
        const info = gl.getShaderInfoLog(shader);
        gl.deleteShader(shader);
        throw new Error(`Shader compile error (${type === gl.VERTEX_SHADER ? 'VS' : 'FS'}): ${info}`);
    }
    return shader;
}

function createProgram(gl, vsSource, fsSource) {
    const vs = createShader(gl, gl.VERTEX_SHADER, vsSource);
    const fs = createShader(gl, gl.FRAGMENT_SHADER, fsSource);
    const program = gl.createProgram();
    gl.attachShader(program, vs);
    gl.attachShader(program, fs);
    gl.linkProgram(program);
    if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        const info = gl.getProgramInfoLog(program);
        gl.deleteProgram(program);
        throw new Error(`Program link error: ${info}`);
    }
    return program;
}

function createTexture(gl, width, height) {
    const tex = gl.createTexture();
    gl.bindTexture(gl.TEXTURE_2D, tex);
    gl.texImage2D(
        gl.TEXTURE_2D, 0, gl.RGBA,
        width, height, 0,
        gl.RGBA, gl.UNSIGNED_BYTE, null
    );
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.bindTexture(gl.TEXTURE_2D, null);
    return tex;
}

function createFBO(gl, width, height) {
    const tex = createTexture(gl, width, height);
    const fbo = gl.createFramebuffer();
    gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
    gl.framebufferTexture2D(
        gl.FRAMEBUFFER,
        gl.COLOR_ATTACHMENT0,
        gl.TEXTURE_2D,
        tex,
        0
    );
    const status = gl.checkFramebufferStatus(gl.FRAMEBUFFER);
    if (status !== gl.FRAMEBUFFER_COMPLETE) {
        throw new Error(`Framebuffer incomplete: 0x${status.toString(16)}`);
    }
    gl.bindFramebuffer(gl.FRAMEBUFFER, null);
    return { fbo, tex, width, height };
}

export class PostProcessingChain {
    constructor(gl, width, height) {
        this.gl = gl;
        this.width = width;
        this.height = height;

        // Quality: 'off' | 'light' (feedback + finish) | 'full' (all)
        this.quality = 'full';

        // Stage enables
        this.enableFeedback = true;
        this.enableBloom = true;
        this.enableFinish = true;

        // Feedback / Trails Parameters
        this.feedback = {
            decay: 0.92,
            zoom: 0.005,
            rotation: 0.0,
            mode: 0 // 0: max, 1: add, 2: mix
        };

        // Bloom Parameters
        this.bloom = {
            threshold: 0.7,
            intensity: 0.8,
            radius: 1.5
        };

        // Finish Pass Parameters
        this.finish = {
            caAmount: 0.005,
            grainAmount: 0.035,
            vignetteAmount: 0.4
        };

        // Per-stage metrics (milliseconds)
        this.metrics = {
            feedbackMs: 0,
            bloomMs: 0,
            finishMs: 0,
            totalMs: 0
        };

        this._initQuad();
        this._initPrograms();
        this._initFBOs();
    }

    _initQuad() {
        const gl = this.gl;
        this.quadVao = gl.createVertexArray();
        gl.bindVertexArray(this.quadVao);

        const posBuffer = gl.createBuffer();
        gl.bindBuffer(gl.ARRAY_BUFFER, posBuffer);
        const positions = new Float32Array([
            -1, -1,
             1, -1,
            -1,  1,
            -1,  1,
             1, -1,
             1,  1,
        ]);
        gl.bufferData(gl.ARRAY_BUFFER, positions, gl.STATIC_DRAW);
        gl.enableVertexAttribArray(0);
        gl.vertexAttribPointer(0, 2, gl.FLOAT, false, 0, 0);
        gl.bindVertexArray(null);
    }

    _initPrograms() {
        const gl = this.gl;
        // 1. Feedback
        this.feedbackProg = createProgram(gl, FULLSCREEN_VS, FEEDBACK_FS);
        this.uFeedback = {
            current: gl.getUniformLocation(this.feedbackProg, 'u_current'),
            prev: gl.getUniformLocation(this.feedbackProg, 'u_prev'),
            resolution: gl.getUniformLocation(this.feedbackProg, 'u_resolution'),
            decay: gl.getUniformLocation(this.feedbackProg, 'u_decay'),
            zoom: gl.getUniformLocation(this.feedbackProg, 'u_zoom'),
            rotation: gl.getUniformLocation(this.feedbackProg, 'u_rotation'),
            mode: gl.getUniformLocation(this.feedbackProg, 'u_mode')
        };

        // 2. Bloom passes
        this.bloomDownsampleProg = createProgram(gl, FULLSCREEN_VS, BLOOM_THRESHOLD_DOWNSAMPLE_FS);
        this.uBloomDown = {
            image: gl.getUniformLocation(this.bloomDownsampleProg, 'u_image'),
            threshold: gl.getUniformLocation(this.bloomDownsampleProg, 'u_threshold')
        };

        this.blurProg = createProgram(gl, FULLSCREEN_VS, BLUR_9TAP_FS);
        this.uBlur = {
            image: gl.getUniformLocation(this.blurProg, 'u_image'),
            direction: gl.getUniformLocation(this.blurProg, 'u_direction'),
            radius: gl.getUniformLocation(this.blurProg, 'u_radius')
        };

        // 3. Finish pass
        this.finishProg = createProgram(gl, FULLSCREEN_VS, FINISH_PASS_FS);
        this.uFinish = {
            scene: gl.getUniformLocation(this.finishProg, 'u_scene'),
            bloom: gl.getUniformLocation(this.finishProg, 'u_bloom'),
            hasBloom: gl.getUniformLocation(this.finishProg, 'u_has_bloom'),
            bloomIntensity: gl.getUniformLocation(this.finishProg, 'u_bloom_intensity'),
            resolution: gl.getUniformLocation(this.finishProg, 'u_resolution'),
            time: gl.getUniformLocation(this.finishProg, 'u_time'),
            kick: gl.getUniformLocation(this.finishProg, 'u_kick'),
            caAmount: gl.getUniformLocation(this.finishProg, 'u_ca_amount'),
            grainAmount: gl.getUniformLocation(this.finishProg, 'u_grain_amount'),
            vignetteAmount: gl.getUniformLocation(this.finishProg, 'u_vignette_amount')
        };
    }

    _initFBOs() {
        const gl = this.gl;
        this._destroyFBOs();

        const w = this.width;
        const h = this.height;

        // Feedback ping-pong buffers (full res)
        this.feedbackFBOs = [
            createFBO(gl, w, h),
            createFBO(gl, w, h)
        ];
        this.feedbackIndex = 0;

        // Bloom buffers (1/4 resolution for Pi V3D budget)
        const bw = Math.max(1, Math.floor(w / 4));
        const bh = Math.max(1, Math.floor(h / 4));
        this.bloomFBOs = [
            createFBO(gl, bw, bh), // Downsampled & H-Blur
            createFBO(gl, bw, bh)  // V-Blur / Final Bloom
        ];
    }

    _destroyFBOs() {
        const gl = this.gl;
        const deleteFBOObj = (obj) => {
            if (obj) {
                if (obj.fbo) gl.deleteFramebuffer(obj.fbo);
                if (obj.tex) gl.deleteTexture(obj.tex);
            }
        };
        if (this.feedbackFBOs) {
            this.feedbackFBOs.forEach(deleteFBOObj);
            this.feedbackFBOs = null;
        }
        if (this.bloomFBOs) {
            this.bloomFBOs.forEach(deleteFBOObj);
            this.bloomFBOs = null;
        }
    }

    resize(width, height) {
        if (this.width === width && this.height === height) return;
        this.width = width;
        this.height = height;
        this._initFBOs();
    }

    setQuality(quality) {
        if (quality !== 'off' && quality !== 'light' && quality !== 'full') {
            console.warn(`[Post] Unknown quality '${quality}', defaulting to 'full'`);
            quality = 'full';
        }
        this.quality = quality;
    }

    /**
     * Render the post-processing chain.
     * 
     * @param {WebGLTexture} inputTexture - Texture from the mixer/scene
     * @param {WebGLFramebuffer|null} targetFBO - Output destination (null = screen canvas)
     * @param {Object} audioParams - { kick: 0..1, time: seconds }
     */
    render(inputTexture, targetFBO = null, audioParams = {}) {
        const gl = this.gl;
        const time = audioParams.time || (performance.now() * 0.001);
        const kick = audioParams.kick || 0.0;

        const measureStart = performance.now();

        if (this.quality === 'off') {
            // Passthrough to target
            gl.bindFramebuffer(gl.FRAMEBUFFER, targetFBO);
            gl.viewport(0, 0, this.width, this.height);
            gl.useProgram(this.finishProg);
            gl.bindVertexArray(this.quadVao);

            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, inputTexture);
            gl.uniform1i(this.uFinish.scene, 0);
            gl.uniform1i(this.uFinish.hasBloom, 0);
            gl.uniform1f(this.uFinish.caAmount, 0.0);
            gl.uniform1f(this.uFinish.grainAmount, 0.0);
            gl.uniform1f(this.uFinish.vignetteAmount, 0.0);
            gl.uniform1f(this.uFinish.kick, 0.0);
            gl.uniform1f(this.uFinish.time, time);
            gl.uniform2f(this.uFinish.resolution, this.width, this.height);

            gl.drawArrays(gl.TRIANGLES, 0, 6);
            gl.bindVertexArray(null);

            this.metrics.feedbackMs = 0;
            this.metrics.bloomMs = 0;
            this.metrics.finishMs = performance.now() - measureStart;
            this.metrics.totalMs = this.metrics.finishMs;
            return;
        }

        const useFeedback = this.enableFeedback;
        const useBloom = (this.quality === 'full') && this.enableBloom;
        const useFinish = this.enableFinish;

        gl.bindVertexArray(this.quadVao);

        // ----------------------------------------------------
        // Stage 1: Feedback / Trails
        // ----------------------------------------------------
        let currentSceneTex = inputTexture;
        let t0 = performance.now();

        if (useFeedback) {
            const writeIdx = this.feedbackIndex;
            const readIdx = 1 - this.feedbackIndex;
            const target = this.feedbackFBOs[writeIdx];
            const prevTex = this.feedbackFBOs[readIdx].tex;

            gl.bindFramebuffer(gl.FRAMEBUFFER, target.fbo);
            gl.viewport(0, 0, this.width, this.height);
            gl.useProgram(this.feedbackProg);

            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, inputTexture);
            gl.uniform1i(this.uFeedback.current, 0);

            gl.activeTexture(gl.TEXTURE1);
            gl.bindTexture(gl.TEXTURE_2D, prevTex);
            gl.uniform1i(this.uFeedback.prev, 1);

            gl.uniform2f(this.uFeedback.resolution, this.width, this.height);
            gl.uniform1f(this.uFeedback.decay, this.feedback.decay);
            gl.uniform1f(this.uFeedback.zoom, this.feedback.zoom);
            gl.uniform1f(this.uFeedback.rotation, this.feedback.rotation);
            gl.uniform1i(this.uFeedback.mode, this.feedback.mode);

            gl.drawArrays(gl.TRIANGLES, 0, 6);

            currentSceneTex = target.tex;
            this.feedbackIndex = readIdx; // advance ping-pong
        }
        let t1 = performance.now();
        this.metrics.feedbackMs = useFeedback ? (t1 - t0) : 0;

        // ----------------------------------------------------
        // Stage 2: Bloom (1/4 res, RGBA8, 2-pass separable Gaussian)
        // ----------------------------------------------------
        let finalBloomTex = null;
        t0 = performance.now();

        if (useBloom) {
            const bw = this.bloomFBOs[0].width;
            const bh = this.bloomFBOs[0].height;

            // 2a. Downsample + Threshold into bloomFBOs[0]
            gl.bindFramebuffer(gl.FRAMEBUFFER, this.bloomFBOs[0].fbo);
            gl.viewport(0, 0, bw, bh);
            gl.useProgram(this.bloomDownsampleProg);

            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, currentSceneTex);
            gl.uniform1i(this.uBloomDown.image, 0);
            gl.uniform1f(this.uBloomDown.threshold, this.bloom.threshold);

            gl.drawArrays(gl.TRIANGLES, 0, 6);

            // 2b. Horizontal blur into bloomFBOs[1]
            gl.bindFramebuffer(gl.FRAMEBUFFER, this.bloomFBOs[1].fbo);
            gl.useProgram(this.blurProg);

            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, this.bloomFBOs[0].tex);
            gl.uniform1i(this.uBlur.image, 0);
            gl.uniform2f(this.uBlur.direction, 1.0 / bw, 0.0);
            gl.uniform1f(this.uBlur.radius, this.bloom.radius);

            gl.drawArrays(gl.TRIANGLES, 0, 6);

            // 2c. Vertical blur into bloomFBOs[0]
            gl.bindFramebuffer(gl.FRAMEBUFFER, this.bloomFBOs[0].fbo);

            gl.activeTexture(gl.TEXTURE0);
            gl.bindTexture(gl.TEXTURE_2D, this.bloomFBOs[1].tex);
            gl.uniform1i(this.uBlur.image, 0);
            gl.uniform2f(this.uBlur.direction, 0.0, 1.0 / bh);
            gl.uniform1f(this.uBlur.radius, this.bloom.radius);

            gl.drawArrays(gl.TRIANGLES, 0, 6);

            finalBloomTex = this.bloomFBOs[0].tex;
        }
        t1 = performance.now();
        this.metrics.bloomMs = useBloom ? (t1 - t0) : 0;

        // ----------------------------------------------------
        // Stage 3: Finish Pass (CA + Bloom Combine + Grain + Vignette)
        // ----------------------------------------------------
        t0 = performance.now();

        gl.bindFramebuffer(gl.FRAMEBUFFER, targetFBO);
        gl.viewport(0, 0, this.width, this.height);
        gl.useProgram(this.finishProg);

        gl.activeTexture(gl.TEXTURE0);
        gl.bindTexture(gl.TEXTURE_2D, currentSceneTex);
        gl.uniform1i(this.uFinish.scene, 0);

        if (useBloom && finalBloomTex) {
            gl.activeTexture(gl.TEXTURE1);
            gl.bindTexture(gl.TEXTURE_2D, finalBloomTex);
            gl.uniform1i(this.uFinish.bloom, 1);
            gl.uniform1i(this.uFinish.hasBloom, 1);
            gl.uniform1f(this.uFinish.bloomIntensity, this.bloom.intensity);
        } else {
            gl.uniform1i(this.uFinish.hasBloom, 0);
        }

        gl.uniform2f(this.uFinish.resolution, this.width, this.height);
        gl.uniform1f(this.uFinish.time, time);
        gl.uniform1f(this.uFinish.kick, kick);

        if (useFinish) {
            gl.uniform1f(this.uFinish.caAmount, this.finish.caAmount);
            gl.uniform1f(this.uFinish.grainAmount, this.finish.grainAmount);
            gl.uniform1f(this.uFinish.vignetteAmount, this.finish.vignetteAmount);
        } else {
            gl.uniform1f(this.uFinish.caAmount, 0.0);
            gl.uniform1f(this.uFinish.grainAmount, 0.0);
            gl.uniform1f(this.uFinish.vignetteAmount, 0.0);
        }

        gl.drawArrays(gl.TRIANGLES, 0, 6);
        gl.bindVertexArray(null);

        t1 = performance.now();
        this.metrics.finishMs = t1 - t0;
        this.metrics.totalMs = performance.now() - measureStart;
    }

    destroy() {
        const gl = this.gl;
        this._destroyFBOs();
        if (this.quadVao) gl.deleteVertexArray(this.quadVao);
        if (this.feedbackProg) gl.deleteProgram(this.feedbackProg);
        if (this.bloomDownsampleProg) gl.deleteProgram(this.bloomDownsampleProg);
        if (this.blurProg) gl.deleteProgram(this.blurProg);
        if (this.finishProg) gl.deleteProgram(this.finishProg);
    }
}
