// RPi-VJ Studio UI Application Logic with Master HDMI Composition Live Preview, Video Playback & Pi 5 Sync

let currentBank = 0;
let draggedSlot = null;
let activePlayingClips = new Set(["cyber_tunnel"]);
let activePlayingShaders = new Set(["neon_tunnel"]);
let popoutWindow = null;

// Audio features state (live or simulated Pioneer FLX-10 128 BPM clock)
let audioFeatures = {
    volume: 0.8,
    bass: 0.9,
    mid: 0.5,
    treble: 0.3,
    onset: 0.0,
    bpm: 128.0,
    beat_phase: 0.0
};

let showData = {
    schema_version: 1,
    name: "Warehouse Rave 2026",
    bpm_default: 128.0,
    beats_per_bar: 4,
    requirements: {
        min_tier: "appliance",
        video_decks: 2,
        shader_layers: 2,
        render_scale: 1.0
    },
    layers: [
        { id: 0, name: "Deck A (HEVC)", opacity: 1.0, blend: "alpha", enabled: true },
        { id: 1, name: "Deck B (HEVC)", opacity: 0.0, blend: "alpha", enabled: true },
        { id: 2, name: "Visualizer 1", opacity: 1.0, blend: "additive", enabled: true },
        { id: 3, name: "Visualizer 2", opacity: 0.5, blend: "screen", enabled: true },
        { id: 4, name: "Master Strobe", opacity: 0.0, blend: "additive", enabled: true },
        { id: 5, name: "Glitch FX", opacity: 0.0, blend: "multiply", enabled: true },
        { id: 6, name: "Audio Equalizer", opacity: 1.0, blend: "alpha", enabled: true },
        { id: 7, name: "Master Output", opacity: 1.0, blend: "alpha", enabled: true }
    ],
    banks: [
        {
            id: 0,
            name: "Bank 1 (Video Loops)",
            grid: Array(4).fill(null).map(() => Array(8).fill(null))
        },
        {
            id: 1,
            name: "Bank 2 (Live Visualizers & Tempo Sync)",
            grid: Array(4).fill(null).map(() => Array(8).fill(null))
        },
        {
            id: 2,
            name: "Bank 3 (Drops & Strobes)",
            grid: Array(4).fill(null).map(() => Array(8).fill(null))
        }
    ]
};

// Seed sample content for Bank 0 (Video Clips)
showData.banks[0].grid[0][0] = { type: "clip", clip_id: "cyber_tunnel", deck: 0, looping: true, title: "Cyber Tunnel Loop", thumb: "⚡" };
showData.banks[0].grid[0][1] = { type: "clip", clip_id: "synthwave_grid", deck: 0, looping: true, title: "Synthwave Grid", thumb: "🌆" };
showData.banks[0].grid[0][2] = { type: "clip", clip_id: "space_warp", deck: 0, looping: true, title: "Deep Space Loop", thumb: "🌌" };
showData.banks[0].grid[0][3] = { type: "clip", clip_id: "neon_city", deck: 0, looping: true, title: "Neon Skyline", thumb: "🌃" };
showData.banks[0].grid[0][7] = { type: "action", action: "blackout", title: "BLACKOUT", color: "#ff2a6d" };

// Seed content for Bank 1 (Visualizer Suite + Page 2 Live Controls)
const visualizers = [
    { id: "vs_dimensional_monolith", title: "3D Brutalist Monoliths", icon: "🏛️" },
    { id: "vs_sacred_polyhedron", title: "3D Sacred Polyhedron", icon: "💎" },
    { id: "vs_cyber_wormhole", title: "3D Relativistic Wormhole", icon: "🌀" },
    { id: "vs_granular_nebula", title: "3D Granular Fluid Nebula", icon: "🌌" },
    { id: "vs_shattered_glitch_core", title: "3D Shattered Glitch Knot", icon: "⚡" },
    { id: "cyber_grid", title: "Retro Cyber Grid & Sun", icon: "🕹️" },
    { id: "neon_tunnel", title: "Neon 3D Speed Tunnel", icon: "🚀" },
    { id: "audio_wave_rings", title: "Sub-Bass Wave Rings", icon: "🪐" },
    { id: "spectrum_bars", title: "5-Band Neon Equalizer", icon: "📊" },
    { id: "kaleidoscope_mirror", title: "Fractal Prism Mirror", icon: "🔮" },
    { id: "particle_warp", title: "Quantum Warp Field", icon: "✨" },
    { id: "audio_oscilloscope", title: "Laser Scope Beam", icon: "〰️" },
    { id: "plasma_flow", title: "Liquid Acid Plasma", icon: "🧪" },
    { id: "vs_dimensional_monolith", title: "Brutalist Laser Core", icon: "🔦" },
    { id: "vs_sacred_polyhedron", title: "Exploding Facet Shards", icon: "💥" },
    { id: "glitch_strobe", title: "Glitch Kick Invert", icon: "⚡" }
];

visualizers.forEach((v, idx) => {
    const row = Math.floor(idx / 8);
    const col = idx % 8;
    if (row < 3) {
        showData.banks[1].grid[row][col] = {
            type: "shader",
            shader_id: v.id,
            layer: 2,
            title: v.title,
            thumb: v.icon
        };
    }
});

// Row 4 of Bank 2: Dedicated Live Performance, Sync & Tempo Controls
showData.banks[1].grid[3][0] = { type: "action", action: "bank_prev", title: "◀ BANK 1", color: "#00d2ff" };
showData.banks[1].grid[3][1] = { type: "action", action: "bpm_down", title: "BPM -1.0", color: "#ffb800" };
showData.banks[1].grid[3][2] = { type: "action", action: "bpm_up", title: "BPM +1.0", color: "#05ffa1" };
showData.banks[1].grid[3][3] = { type: "action", action: "beat_sync", title: "BEAT SYNC", color: "#00d2ff" };
showData.banks[1].grid[3][4] = { type: "action", action: "phase_nudge_half", title: "1/2 BEAT FLIP", color: "#ff9900" };
showData.banks[1].grid[3][5] = { type: "action", action: "tap_tempo", title: "TAP BPM", color: "#ffffff" };
showData.banks[1].grid[3][6] = { type: "action", action: "blackout", title: "BLACKOUT", color: "#ff2a6d" };
showData.banks[1].grid[3][7] = { type: "action", action: "bank_next", title: "BANK 3 ▶", color: "#00d2ff" };

let activeModalSlot = { row: 0, col: 0 };

function init() {
    createToastElement();
    renderBankTabs();
    renderDeckGrid();
    renderMixerStrips();
    setupEventListeners();
    setupTheaterModal();
    setupTransferModal();
    initMasterCompositor();
}

function createToastElement() {
    if (!document.getElementById("toast-notification")) {
        const toast = document.createElement("div");
        toast.id = "toast-notification";
        toast.className = "toast-box";
        document.body.appendChild(toast);
    }
}

function showToast(msg) {
    const toast = document.getElementById("toast-notification");
    if (toast) {
        toast.textContent = msg;
        toast.classList.add("show");
        setTimeout(() => {
            toast.classList.remove("show");
        }, 2400);
    }
}

// ---------------------------------------------------------------------------
// Master HDMI Live Compositor Engine
// ---------------------------------------------------------------------------
let lastFrameTime = performance.now();
let frameCount = 0;
let fps = 60.0;

function initMasterCompositor() {
    const canvas = document.getElementById("master-output-canvas");
    const ctx = canvas.getContext("2d");
    const fpsMeter = document.getElementById("preview-fps");
    const bassFill = document.getElementById("hud-bass-fill");
    const hudDeckA = document.getElementById("hud-deck-a");
    const hudShader = document.getElementById("hud-shader");

    const resizeBtn = document.getElementById("toggle-preview-size-btn");
    const popoutBtn = document.getElementById("popout-hdmi-btn");
    const audioInBtn = document.getElementById("toggle-audio-in-btn");
    const container = document.getElementById("master-canvas-container");

    if (resizeBtn && container) {
        resizeBtn.onclick = () => {
            container.classList.toggle("expanded");
            resizeBtn.textContent = container.classList.contains("expanded") ? "⤡ Compact" : "⤢ Resize";
        };
    }

    if (popoutBtn) {
        popoutBtn.onclick = openPopoutHDMIWindow;
    }

    if (audioInBtn) {
        audioInBtn.onclick = enableMicrophoneAudio;
    }

    function renderLoop(now) {
        const dt = (now - lastFrameTime) / 1000.0;
        lastFrameTime = now;

        frameCount++;
        if (frameCount % 30 === 0) {
            fps = (1.0 / Math.max(dt, 0.001)).toFixed(1);
            if (fpsMeter) fpsMeter.textContent = `${fps} FPS`;
        }

        // Simulate 128 BPM Kick / Audio Pulse if mic not active
        const beatInterval = 60.0 / showData.bpm_default;
        const phase = (now / 1000.0 / beatInterval) % 1.0;
        audioFeatures.beat_phase = phase;

        // Kick impulse at start of each beat (0.0 to 0.15)
        if (phase < 0.12) {
            audioFeatures.onset = 1.0 - (phase / 0.12);
            audioFeatures.bass = 0.6 + audioFeatures.onset * 0.4;
        } else {
            audioFeatures.onset = Math.max(0.0, audioFeatures.onset - dt * 6.0);
            audioFeatures.bass = Math.max(0.15, audioFeatures.bass - dt * 2.0);
        }
        audioFeatures.mid = 0.4 + Math.sin(now * 0.003) * 0.3;
        audioFeatures.treble = 0.3 + Math.cos(now * 0.005) * 0.25;

        if (bassFill) {
            bassFill.style.width = `${Math.round(audioFeatures.bass * 100)}%`;
        }

        // 1. Clear Master Screen
        const w = canvas.width;
        const h = canvas.height;
        ctx.fillStyle = "#000000";
        ctx.fillRect(0, 0, w, h);

        // 2. Draw Layer 0 (Video Deck A)
        let activeVideoEl = document.querySelector(".grid-slot.is-playing .slot-thumbnail-video");
        let activeClipName = "cyber_tunnel";

        if (activePlayingClips.size > 0) {
            activeClipName = Array.from(activePlayingClips)[0];
            if (hudDeckA) hudDeckA.innerHTML = `DECK A: <span>${activeClipName} (100%)</span>`;
        } else {
            if (hudDeckA) hudDeckA.innerHTML = `DECK A: <span>[IDLE]</span>`;
        }

        if (activeVideoEl && !activeVideoEl.paused && activeVideoEl.readyState >= 2) {
            ctx.drawImage(activeVideoEl, 0, 0, w, h);
        } else {
            // High-performance procedural video background synthesis
            drawProceduralVideoBackground(ctx, w, h, now * 0.001, activeClipName);
        }

        // 3. Draw Active Audio-Reactive ISF Shader Layer
        if (activePlayingShaders.size > 0) {
            const shaderId = Array.from(activePlayingShaders)[0];
            if (hudShader) hudShader.innerHTML = `SHADER: <span>${shaderId} (Additive)</span>`;

            ctx.save();
            ctx.globalCompositeOperation = "lighter"; // Additive blending for neon visualizers
            drawShaderLayer(ctx, w, h, now * 0.001, shaderId, audioFeatures);
            ctx.restore();
        } else {
            if (hudShader) hudShader.innerHTML = `SHADER: <span>[OFF]</span>`;
        }

        // Mirror to Popout HDMI Window if open
        if (popoutWindow && !popoutWindow.closed) {
            const popoutCanvas = popoutWindow.document.getElementById("popout-canvas");
            if (popoutCanvas) {
                const pctx = popoutCanvas.getContext("2d");
                pctx.drawImage(canvas, 0, 0, popoutCanvas.width, popoutCanvas.height);
            }
        }

        requestAnimationFrame(renderLoop);
    }

    requestAnimationFrame(renderLoop);
}

function drawProceduralVideoBackground(ctx, w, h, t, clipName) {
    const grad = ctx.createLinearGradient(0, 0, w, h);
    grad.addColorStop(0, "#080d1a");
    grad.addColorStop(0.5, "#140a20");
    grad.addColorStop(1, "#0a1826");
    ctx.fillStyle = grad;
    ctx.fillRect(0, 0, w, h);

    // Dynamic grid motion
    ctx.strokeStyle = "rgba(0, 210, 255, 0.25)";
    ctx.lineWidth = 1.5;
    const gridOffset = (t * 80) % 40;

    for (let x = 0; x < w; x += 40) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, h);
        ctx.stroke();
    }
    for (let y = gridOffset; y < h; y += 40) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(w, y);
        ctx.stroke();
    }
}

function drawShaderLayer(ctx, w, h, t, shaderId, audio) {
    const cx = w / 2;
    const cy = h / 2;

    if (shaderId === "neon_tunnel" || shaderId === "vs_cyber_wormhole") {
        // 3D Audio-Reactive Relativistic Speed Tunnel / Cyber Wormhole
        const rings = 16;
        for (let i = 0; i < rings; i++) {
            const z = (t * (1.5 + audio.bass * 2.5) + i / rings) % 1.0;
            const r = Math.pow(z, 2.8) * (w * 0.7);
            const alpha = z * (0.85 + audio.onset * 0.6);

            ctx.beginPath();
            ctx.arc(cx, cy, r, 0, Math.PI * 2);
            ctx.strokeStyle = `rgba(${Math.round(255 * z + audio.onset * 120)}, 42, ${Math.round(220 * (1 - z))}, ${alpha})`;
            ctx.lineWidth = 2.0 + (1.0 - z) * 8.0 + audio.bass * 5.0;
            ctx.stroke();
        }
    } else if (shaderId === "vs_dimensional_monolith") {
        // 3D Raymarched Brutalist Monoliths & Laser Illumination
        ctx.fillStyle = "rgba(0, 0, 0, 0.4)";
        ctx.fillRect(0, 0, w, h);
        
        // Perspective Monolith Pillars
        for (let i = -3; i <= 3; i++) {
            const px = cx + i * (w * 0.14);
            const pillarH = h * (0.4 + Math.sin(t * 2.0 + i) * 0.2 + audio.bass * 0.3);
            const grad = ctx.createLinearGradient(px, h, px, h - pillarH);
            grad.addColorStop(0, "rgba(0, 210, 255, 0.1)");
            grad.addColorStop(1, "rgba(0, 210, 255, 0.85)");
            ctx.fillStyle = grad;
            ctx.fillRect(px - 18, h - pillarH, 36, pillarH);
            ctx.strokeStyle = "#00d2ff";
            ctx.strokeRect(px - 18, h - pillarH, 36, pillarH);
        }
        // Floating Central Core
        const coreY = cy - 20 + Math.sin(t * 3.0) * 15.0;
        const coreSize = 35 + audio.bass * 30 + audio.onset * 25;
        ctx.save();
        ctx.translate(cx, coreY);
        ctx.rotate(t * 1.5);
        ctx.strokeStyle = "#ff2a6d";
        ctx.lineWidth = 4.0;
        ctx.strokeRect(-coreSize/2, -coreSize/2, coreSize, coreSize);
        ctx.restore();
    } else if (shaderId === "vs_sacred_polyhedron") {
        // 3D Sacred Geometry Icosahedron & Orbiting Shards
        const burst = audio.onset * 40;
        ctx.save();
        ctx.translate(cx, cy);
        ctx.rotate(t);
        
        // 12-vertex Sacred Polyhedron wireframe
        ctx.strokeStyle = `rgba(0, 240, 255, ${0.8 + audio.onset * 0.4})`;
        ctx.lineWidth = 3.0 + audio.bass * 4.0;
        ctx.beginPath();
        for (let a = 0; a < Math.PI * 2; a += Math.PI / 3) {
            const rad = 70 + audio.bass * 30;
            const x = Math.cos(a) * rad;
            const y = Math.sin(a) * rad;
            ctx.lineTo(x, y);
            ctx.moveTo(0, 0);
            ctx.lineTo(x, y);
        }
        ctx.closePath();
        ctx.stroke();

        // Orbiting geometric shards
        for (let s = 0; s < 6; s++) {
            const sa = s * (Math.PI / 3) + t * 2.0;
            const sDist = 120 + burst;
            const sx = Math.cos(sa) * sDist;
            const sy = Math.sin(sa) * sDist;
            ctx.fillStyle = "#ff2a6d";
            ctx.fillRect(sx - 8, sy - 8, 16, 16);
        }
        ctx.restore();
    } else if (shaderId === "vs_shattered_glitch_core") {
        // 3D Fractured Glitch Torus Knot
        ctx.save();
        ctx.translate(cx, cy);
        ctx.rotate(t * 0.8);
        ctx.strokeStyle = "#ff0055";
        ctx.lineWidth = 5.0;
        ctx.beginPath();
        for (let a = 0; a < Math.PI * 2; a += 0.1) {
            const r = 90 + Math.sin(a * 3.0 + t * 4.0) * (30 + audio.bass * 30);
            const gx = (audio.onset > 0.4 && Math.random() > 0.6) ? (Math.random() - 0.5) * 40 : 0;
            ctx.lineTo(Math.cos(a) * r + gx, Math.sin(a) * r);
        }
        ctx.closePath();
        ctx.stroke();
        ctx.restore();
    } else if (shaderId === "vs_granular_nebula" || shaderId === "plasma_flow") {
        // 3D Volumetric Fluid Nebula
        for (let i = 0; i < 40; i++) {
            const pAngle = i * 0.157 + t * 0.5;
            const pRad = (Math.sin(i + t) * 0.5 + 0.5) * (w * 0.35) + audio.bass * 40;
            const px = cx + Math.cos(pAngle) * pRad;
            const py = cy + Math.sin(pAngle) * pRad;
            const grad = ctx.createRadialGradient(px, py, 2, px, py, 45 + audio.bass * 25);
            grad.addColorStop(0, "rgba(255, 42, 109, 0.6)");
            grad.addColorStop(0.5, "rgba(0, 210, 255, 0.3)");
            grad.addColorStop(1, "rgba(0,0,0,0)");
            ctx.fillStyle = grad;
            ctx.beginPath();
            ctx.arc(px, py, 45 + audio.bass * 25, 0, Math.PI * 2);
            ctx.fill();
        }
    } else if (shaderId === "audio_wave_rings") {
        // Concentric Audio Wave Rings
        for (let i = 1; i <= 8; i++) {
            const r = i * 28 + audio.bass * 35;
            ctx.beginPath();
            ctx.arc(cx, cy, r, 0, Math.PI * 2);
            ctx.strokeStyle = `rgba(5, 255, 161, ${0.9 - i * 0.1})`;
            ctx.lineWidth = 3.0 + audio.onset * 4.0;
            ctx.stroke();
        }
    } else if (shaderId === "spectrum_bars") {
        // 5-Band Equalizer
        const barW = w * 0.12;
        const heights = [
            audio.bass * h * 0.7 + audio.onset * 50,
            audio.bass * h * 0.5 + audio.mid * 40,
            audio.mid * h * 0.6,
            audio.mid * h * 0.3 + audio.treble * 50,
            audio.treble * h * 0.7
        ];
        heights.forEach((val, i) => {
            const bx = w * 0.15 + i * (barW + 20);
            const bh = Math.min(val, h * 0.85);
            const grad = ctx.createLinearGradient(0, h, 0, h - bh);
            grad.addColorStop(0, "#05ffa1");
            grad.addColorStop(1, "#ff2a6d");
            ctx.fillStyle = grad;
            ctx.fillRect(bx, h - bh, barW, bh);
        });
    } else if (shaderId === "cyber_grid") {
        // Retro Synthwave Grid & Sun
        const sunR = 70 + audio.bass * 25;
        const sunGrad = ctx.createRadialGradient(cx, cy - 40, 10, cx, cy - 40, sunR);
        sunGrad.addColorStop(0, "#ffb800");
        sunGrad.addColorStop(1, "#ff2a6d");
        ctx.fillStyle = sunGrad;
        ctx.beginPath();
        ctx.arc(cx, cy - 40, sunR, 0, Math.PI * 2);
        ctx.fill();

        ctx.strokeStyle = "#00d2ff";
        ctx.lineWidth = 2.0;
        for (let i = 0; i < 10; i++) {
            const gy = cy + Math.pow(i / 10, 2) * (h - cy);
            ctx.beginPath();
            ctx.moveTo(0, gy);
            ctx.lineTo(w, gy);
            ctx.stroke();
        }
    } else if (shaderId === "particle_warp") {
        // Starfield Particle Warp
        for (let i = 0; i < 60; i++) {
            const angle = (i * 0.1047) + t * 0.2;
            const dist = ((t * (120 + audio.bass * 300) + i * 20) % (w * 0.6));
            const px = cx + Math.cos(angle) * dist;
            const py = cy + Math.sin(angle) * dist;
            ctx.fillStyle = `rgba(255, 255, 255, ${dist / (w * 0.6)})`;
            ctx.beginPath();
            ctx.arc(px, py, 1.5 + (dist / 100), 0, Math.PI * 2);
            ctx.fill();
        }
    } else {
        // Generic dynamic visualizer geometry
        ctx.beginPath();
        ctx.arc(cx, cy, 100 + audio.bass * 80, 0, Math.PI * 2);
        ctx.strokeStyle = "#ff2a6d";
        ctx.lineWidth = 6.0;
        ctx.stroke();
    }
}

function openPopoutHDMIWindow() {
    popoutWindow = window.open("", "RPi-VJ Master HDMI Output", "width=1280,height=720,menubar=no,toolbar=no,location=no,status=no");
    if (popoutWindow) {
        popoutWindow.document.write(`
            <!DOCTYPE html>
            <html>
            <head>
                <title>RPi-VJ — Clean HDMI Projector Feed</title>
                <style>
                    body { margin: 0; background: #000; overflow: hidden; display: flex; justify-content: center; align-items: center; height: 100vh; }
                    canvas { width: 100vw; height: 100vh; object-fit: contain; }
                </style>
            </head>
            <body>
                <canvas id="popout-canvas" width="1920" height="1080"></canvas>
            </body>
            </html>
        `);
        showToast("📺 Opened Clean HDMI Projector Window!");
    }
}

async function enableMicrophoneAudio() {
    try {
        const stream = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
        const audioCtx = new (window.AudioContext || window.webkitAudioContext)();
        const src = audioCtx.createMediaStreamSource(stream);
        const analyzer = audioCtx.createAnalyser();
        analyzer.fftSize = 256;
        src.connect(analyzer);

        const data = new Uint8Array(analyzer.frequencyBinCount);
        const audioInBtn = document.getElementById("toggle-audio-in-btn");
        if (audioInBtn) {
            audioInBtn.textContent = "🎙️ Live Audio Active";
            audioInBtn.style.background = "#05ffa1";
            audioInBtn.style.color = "#000";
        }

        function pollAudio() {
            analyzer.getByteFrequencyData(data);
            let bassSum = 0;
            for (let i = 0; i < 10; i++) bassSum += data[i];
            audioFeatures.bass = Math.min(1.0, (bassSum / 10) / 180.0);
            if (audioFeatures.bass > 0.75) audioFeatures.onset = 1.0;
            requestAnimationFrame(pollAudio);
        }
        pollAudio();
        showToast("🎤 Connected to Sound Card / Microphone Input!");
    } catch (e) {
        showToast("⚠️ Microphone access denied, using internal tempo clock.");
    }
}

// ---------------------------------------------------------------------------
// Bank Tabs & Grid Navigation
// ---------------------------------------------------------------------------
function renderBankTabs() {
    const tabsContainer = document.getElementById("bank-tabs");
    tabsContainer.innerHTML = "";

    showData.banks.forEach((bank, idx) => {
        const btn = document.createElement("button");
        btn.className = `bank-tab ${idx === currentBank ? "active" : ""}`;
        btn.textContent = bank.name;
        btn.onclick = () => {
            currentBank = idx;
            renderBankTabs();
            renderDeckGrid();
        };

        btn.ondragover = (e) => {
            e.preventDefault();
            btn.classList.add("active");
        };
        btn.ondragleave = () => {
            if (idx !== currentBank) btn.classList.remove("active");
        };
        btn.ondrop = (e) => {
            e.preventDefault();
            currentBank = idx;
            renderBankTabs();
            renderDeckGrid();
            showToast(`📂 Switched to ${bank.name}`);
        };

        tabsContainer.appendChild(btn);
    });

    const addBtn = document.createElement("button");
    addBtn.className = "bank-tab add-bank-btn";
    addBtn.textContent = "+ New Bank";
    addBtn.onclick = addNewBank;
    tabsContainer.appendChild(addBtn);
}

function addNewBank() {
    const newId = showData.banks.length;
    showData.banks.push({
        id: newId,
        name: `Bank ${newId + 1}`,
        grid: Array(4).fill(null).map(() => Array(8).fill(null))
    });
    currentBank = newId;
    renderBankTabs();
    renderDeckGrid();
    showToast(`✨ Created Bank ${newId + 1}`);
}

function renderDeckGrid() {
    const gridEl = document.getElementById("deck-grid");
    gridEl.innerHTML = "";

    const bank = showData.banks[currentBank];

    for (let r = 0; r < 4; r++) {
        for (let c = 0; c < 8; c++) {
            const slotData = bank.grid[r][c];
            const slotEl = document.createElement("div");
            slotEl.className = "grid-slot";
            slotEl.dataset.row = r;
            slotEl.dataset.col = c;

            if (slotData) {
                slotEl.draggable = true;
                slotEl.ondragstart = (e) => {
                    draggedSlot = { bank: currentBank, row: r, col: c, data: slotData };
                    slotEl.classList.add("dragging");
                    e.dataTransfer.setData("text/plain", JSON.stringify(draggedSlot));
                };
                slotEl.ondragend = () => {
                    slotEl.classList.remove("dragging");
                    draggedSlot = null;
                };
            }

            setupSlotDropHandlers(slotEl, r, c);

            if (!slotData) {
                slotEl.classList.add("empty");
                slotEl.innerHTML = `<span>+ [R${r+1}:C${c+1}]<br><small style="color:var(--text-muted);font-size:9px;">Drop Clip Here</small></span>`;
                slotEl.onclick = () => openSlotModal(r, c);
            } else {
                const isPlayingClip = slotData.type === "clip" && activePlayingClips.has(slotData.clip_id);
                const isPlayingShader = slotData.type === "shader" && activePlayingShaders.has(slotData.shader_id);
                const isPlaying = isPlayingClip || isPlayingShader;

                if (isPlaying) {
                    slotEl.classList.add("is-playing");
                }

                let badgeClass = "badge-clip";
                let badgeText = "CLIP";
                let icon = slotData.thumb || "▶";

                if (slotData.type === "shader") {
                    slotEl.classList.add("has-shader");
                    badgeClass = isPlaying ? "badge-live-shader" : "badge-shader";
                    badgeText = isPlaying ? "● SHADER" : "ISF";
                } else if (slotData.type === "action") {
                    slotEl.classList.add("has-action");
                    badgeClass = "badge-action";
                    badgeText = "ACT";
                    if (slotData.color) {
                        slotEl.style.borderColor = slotData.color;
                    }
                } else {
                    slotEl.classList.add("has-clip");
                    badgeClass = isPlaying ? "badge-live" : "badge-clip";
                    badgeText = isPlaying ? "● LIVE" : "CLIP";
                }

                let hoverActionsHtml = '';
                if (slotData.type === "clip" || slotData.type === "shader") {
                    const playIcon = isPlaying ? "⏹" : "▶";
                    const playTitle = isPlaying ? "Stop / Pause" : "Play Clip";
                    hoverActionsHtml = `
                        <div class="slot-hover-actions">
                            <button class="slot-action-btn play-toggle-btn" title="${playTitle}" style="${isPlaying ? 'background:#d32f2f;color:#fff;' : ''}">${playIcon}</button>
                            ${slotData.type === 'clip' ? '<button class="slot-action-btn fullscreen-btn" title="Full Screen Theater Preview">⛶</button>' : ''}
                        </div>
                    `;
                }

                let mediaPreviewHtml = '';
                if (slotData.videoUrl) {
                    mediaPreviewHtml = `
                        <div class="slot-thumbnail-container">
                            <video src="${slotData.videoUrl}" class="slot-thumbnail-video" loop muted playsinline ${isPlaying ? 'autoplay' : ''}></video>
                        </div>
                    `;
                } else if (slotData.imgData) {
                    mediaPreviewHtml = `<img src="${slotData.imgData}" class="slot-thumbnail" alt="thumbnail">`;
                } else {
                    mediaPreviewHtml = `<span style="font-size: 20px;">${icon}</span>`;
                }

                slotEl.innerHTML = `
                    <div class="slot-header">
                        <span>R${r+1}:C${c+1}</span>
                        <span class="slot-badge ${badgeClass}">${badgeText}</span>
                    </div>
                    ${hoverActionsHtml}
                    <div class="slot-body">
                        ${mediaPreviewHtml}
                        <span class="slot-title">${slotData.title || slotData.clip_id || slotData.shader_id || slotData.action}</span>
                    </div>
                `;

                const playBtn = slotEl.querySelector(".play-toggle-btn");
                const fsBtn = slotEl.querySelector(".fullscreen-btn");
                const videoEl = slotEl.querySelector(".slot-thumbnail-video");

                if (videoEl && isPlaying) {
                    videoEl.play().catch(() => {});
                }

                if (playBtn) {
                    playBtn.onclick = (e) => {
                        e.stopPropagation();
                        togglePlayback(slotData);
                    };
                }

                if (fsBtn) {
                    fsBtn.onclick = (e) => {
                        e.stopPropagation();
                        openTheaterPreview(slotData);
                    };
                }

                slotEl.onclick = (e) => {
                    if (!e.target.classList.contains("slot-action-btn")) {
                        if (slotData.type === "clip" || slotData.type === "shader") {
                            togglePlayback(slotData);
                        } else {
                            openSlotModal(r, c);
                        }
                    }
                };
            }

            gridEl.appendChild(slotEl);
        }
    }
}

function togglePlayback(slot) {
    if (slot.type === "clip") {
        if (activePlayingClips.has(slot.clip_id)) {
            activePlayingClips.delete(slot.clip_id);
            showToast(`⏹ Stopped: ${slot.title || slot.clip_id}`);
        } else {
            activePlayingClips.clear();
            activePlayingClips.add(slot.clip_id);
            showToast(`▶ LIVE on Deck ${slot.deck === 1 ? 'B' : 'A'}: ${slot.title || slot.clip_id}`);
        }
    } else if (slot.type === "shader") {
        if (activePlayingShaders.has(slot.shader_id)) {
            activePlayingShaders.delete(slot.shader_id);
            showToast(`⏹ Stopped Shader: ${slot.title || slot.shader_id}`);
        } else {
            activePlayingShaders.clear();
            activePlayingShaders.add(slot.shader_id);
            showToast(`✨ LIVE Shader: ${slot.title || slot.shader_id}`);
        }
    }
    renderDeckGrid();
}

function setupSlotDropHandlers(slotEl, row, col) {
    slotEl.ondragenter = (e) => {
        e.preventDefault();
        slotEl.classList.add("drag-over");
    };

    slotEl.ondragover = (e) => {
        e.preventDefault();
        e.dataTransfer.dropEffect = "copy";
        slotEl.classList.add("drag-over");
    };

    slotEl.ondragleave = () => {
        slotEl.classList.remove("drag-over");
    };

    slotEl.ondrop = async (e) => {
        e.preventDefault();
        slotEl.classList.remove("drag-over");

        if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
            handleFilesDropped(e.dataTransfer.files, row, col);
            return;
        }

        if (draggedSlot) {
            const source = draggedSlot;
            const targetData = showData.banks[currentBank].grid[row][col];

            showData.banks[currentBank].grid[row][col] = source.data;
            showData.banks[source.bank].grid[source.row][source.col] = targetData;

            renderDeckGrid();
            showToast(`⚡ Moved slot to [R${row+1}:C${col+1}]`);
            draggedSlot = null;
        }
    };
}

async function handleFilesDropped(files, startRow, startCol) {
    let r = startRow;
    let c = startCol;

    for (let i = 0; i < files.length; i++) {
        const file = files[i];
        const clipId = file.name.replace(/\.[^/.]+$/, "");
        const videoUrl = URL.createObjectURL(file);

        let imgData = null;
        if (file.type.startsWith("video/")) {
            imgData = await generateVideoThumbnail(file);
        }

        const slot = {
            type: "clip",
            clip_id: clipId,
            path: `media/hevc/${clipId}_pi.mp4`,
            raw_filename: file.name,
            videoUrl: videoUrl,
            deck: 0,
            looping: true,
            title: clipId,
            thumb: "🎬",
            imgData: imgData
        };

        showData.banks[currentBank].grid[r][c] = slot;

        c++;
        if (c >= 8) {
            c = 0;
            r++;
            if (r >= 4) break;
        }
    }

    renderDeckGrid();
    showToast(`🎬 Ingested ${files.length} clip(s) with live preview!`);
}

function generateVideoThumbnail(file) {
    return new Promise((resolve) => {
        try {
            const video = document.createElement("video");
            const url = URL.createObjectURL(file);
            video.src = url;
            video.muted = true;
            video.playsInline = true;
            video.currentTime = 1.0;

            video.onloadeddata = () => {
                video.currentTime = Math.min(1.0, video.duration * 0.1);
            };

            video.onseeked = () => {
                const canvas = document.createElement("canvas");
                canvas.width = 96;
                canvas.height = 96;
                const ctx = canvas.getContext("2d");
                ctx.drawImage(video, 0, 0, 96, 96);
                const dataUrl = canvas.toDataURL("image/jpeg", 0.7);
                resolve(dataUrl);
            };

            video.onerror = () => resolve(null);
        } catch (e) {
            resolve(null);
        }
    });
}

function setupTheaterModal() {
    const modal = document.getElementById("theater-modal");
    const video = document.getElementById("theater-video");
    const playBtn = document.getElementById("theater-play-btn");
    const muteBtn = document.getElementById("theater-mute-btn");
    const scrub = document.getElementById("theater-scrub");
    const timeEl = document.getElementById("theater-time");
    const fsBtn = document.getElementById("theater-fs-btn");
    const closeBtn = document.getElementById("close-theater-btn");

    closeBtn.onclick = () => {
        video.pause();
        modal.classList.remove("open");
    };

    playBtn.onclick = () => {
        if (video.paused) {
            video.play();
            playBtn.textContent = "⏸ Pause";
        } else {
            video.pause();
            playBtn.textContent = "▶ Play";
        }
    };

    muteBtn.onclick = () => {
        video.muted = !video.muted;
        muteBtn.textContent = video.muted ? "🔇 Muted" : "🔊 Unmuted";
    };

    video.ontimeupdate = () => {
        if (video.duration) {
            scrub.value = (video.currentTime / video.duration) * 100;
            timeEl.textContent = `${formatTime(video.currentTime)} / ${formatTime(video.duration)}`;
        }
    };

    scrub.oninput = (e) => {
        if (video.duration) {
            video.currentTime = (parseFloat(e.target.value) / 100) * video.duration;
        }
    };

    fsBtn.onclick = () => {
        if (video.requestFullscreen) {
            video.requestFullscreen();
        }
    };

    window.addEventListener("keydown", (e) => {
        if (e.key === "Escape" && modal.classList.contains("open")) {
            video.pause();
            modal.classList.remove("open");
        }
    });
}

function openTheaterPreview(slot) {
    const modal = document.getElementById("theater-modal");
    const video = document.getElementById("theater-video");
    const title = document.getElementById("theater-title");
    const badge = document.getElementById("theater-badge");

    title.textContent = `Preview: ${slot.title || slot.clip_id}`;
    badge.textContent = `DECK ${slot.deck === 1 ? 'B' : 'A'} (HEVC 1080p60)`;

    if (slot.videoUrl) {
        video.src = slot.videoUrl;
    } else {
        video.src = "";
    }

    video.currentTime = 0;
    video.play();
    modal.classList.add("open");
}

function formatTime(seconds) {
    const m = Math.floor(seconds / 60);
    const s = Math.floor(seconds % 60);
    return `${m}:${s < 10 ? '0' : ''}${s}`;
}

function setupTransferModal() {
    const transferBtn = document.getElementById("transfer-pi-btn");
    const modal = document.getElementById("transfer-modal");
    const closeBtn = document.getElementById("close-transfer-btn");
    const doneBtn = document.getElementById("done-transfer-btn");
    const downloadZipBtn = document.getElementById("download-zip-btn");

    if (transferBtn && modal) {
        transferBtn.onclick = () => modal.classList.add("open");
        closeBtn.onclick = () => modal.classList.remove("open");
        doneBtn.onclick = () => modal.classList.remove("open");
        downloadZipBtn.onclick = exportShowToml;
    }
}

function renderMixerStrips() {
    const mixerEl = document.getElementById("channel-strips");
    mixerEl.innerHTML = "";

    showData.layers.forEach((layer, idx) => {
        const strip = document.createElement("div");
        strip.className = "channel-strip";
        strip.innerHTML = `
            <span class="strip-label">CH ${idx + 1}</span>
            <div class="rotary-knob" title="Encoder ${idx + 1}: Effect Param">
                <div class="knob-inner">${Math.round(layer.opacity * 100)}</div>
            </div>
            <div class="fader-box">
                <input type="range" min="0" max="1" step="0.01" value="${layer.opacity}" orient="vertical">
                <span class="fader-val">${Math.round(layer.opacity * 100)}%</span>
            </div>
            <span style="font-size: 9px; color: var(--text-muted); text-align: center;">${layer.name.substring(0, 10)}</span>
        `;

        const fader = strip.querySelector('input[type="range"]');
        const faderVal = strip.querySelector('.fader-val');
        const knobVal = strip.querySelector('.knob-inner');

        fader.oninput = (e) => {
            const val = parseFloat(e.target.value);
            layer.opacity = val;
            faderVal.textContent = `${Math.round(val * 100)}%`;
            knobVal.textContent = `${Math.round(val * 100)}`;
        };

        mixerEl.appendChild(strip);
    });
}

function openSlotModal(row, col) {
    activeModalSlot = { row, col };
    document.getElementById("modal-title").textContent = `Configure Stream Deck Key [Row ${row+1}, Col ${col+1}]`;
    const slot = showData.banks[currentBank].grid[row][col];
    const typeSelect = document.getElementById("slot-type");

    if (slot) {
        typeSelect.value = slot.type;
        if (slot.type === "clip") {
            document.getElementById("slot-video-path").value = slot.path || `media/hevc/${slot.clip_id}_pi.mp4`;
            document.getElementById("slot-deck").value = slot.deck || 0;
            document.getElementById("slot-loop").checked = slot.looping !== false;
        } else if (slot.type === "shader") {
            document.getElementById("slot-shader-id").value = slot.shader_id || "neon_tunnel";
            document.getElementById("slot-layer").value = slot.layer || 2;
        } else if (slot.type === "action") {
            document.getElementById("slot-action-name").value = slot.action || "blackout";
        }
    } else {
        typeSelect.value = "clip";
        document.getElementById("slot-video-path").value = "";
    }

    updateModalFields();
    document.getElementById("slot-modal").classList.add("open");
}

function updateModalFields() {
    const type = document.getElementById("slot-type").value;
    document.getElementById("clip-fields").style.display = type === "clip" ? "block" : "none";
    document.getElementById("shader-fields").style.display = type === "shader" ? "block" : "none";
    document.getElementById("action-fields").style.display = type === "action" ? "block" : "none";
}

function setupEventListeners() {
    document.getElementById("slot-type").onchange = updateModalFields;
    document.getElementById("close-modal-btn").onclick = () => {
        document.getElementById("slot-modal").classList.remove("open");
    };

    const fileInput = document.getElementById("batch-file-input");
    const browseBtn = document.getElementById("browse-files-btn");

    if (browseBtn && fileInput) {
        browseBtn.onclick = () => fileInput.click();
        fileInput.onchange = (e) => {
            if (e.target.files && e.target.files.length > 0) {
                handleFilesDropped(e.target.files, 0, 0);
            }
        };
    }

    document.getElementById("save-slot-btn").onclick = () => {
        const { row, col } = activeModalSlot;
        const type = document.getElementById("slot-type").value;
        let slot = null;

        if (type === "clip") {
            const path = document.getElementById("slot-video-path").value || "clip.mp4";
            const clipId = path.split("/").pop().split(".")[0];
            slot = {
                type: "clip",
                clip_id: clipId,
                path: path,
                deck: parseInt(document.getElementById("slot-deck").value, 10),
                looping: document.getElementById("slot-loop").checked,
                title: clipId,
                thumb: "▶"
            };
        } else if (type === "shader") {
            const shaderId = document.getElementById("slot-shader-id").value;
            const selectEl = document.getElementById("slot-shader-id");
            const title = selectEl.options[selectEl.selectedIndex].text;
            slot = {
                type: "shader",
                shader_id: shaderId,
                layer: parseInt(document.getElementById("slot-layer").value, 10),
                title: title.split(" (")[0],
                thumb: "🌀"
            };
        } else if (type === "action") {
            const action = document.getElementById("slot-action-name").value;
            const selectEl = document.getElementById("slot-action-name");
            const title = selectEl.options[selectEl.selectedIndex].text;
            slot = {
                type: "action",
                action: action,
                title: title,
                color: document.getElementById("slot-action-color").value
            };
        }

        showData.banks[currentBank].grid[row][col] = slot;
        document.getElementById("slot-modal").classList.remove("open");
        renderDeckGrid();
        showToast(`Saved [R${row+1}:C${col+1}]`);
    };

    document.getElementById("clear-slot-btn").onclick = () => {
        const { row, col } = activeModalSlot;
        showData.banks[currentBank].grid[row][col] = null;
        document.getElementById("slot-modal").classList.remove("open");
        renderDeckGrid();
        showToast(`Cleared [R${row+1}:C${col+1}]`);
    };

    document.getElementById("export-show-btn").onclick = exportShowToml;
}

function exportShowToml() {
    showData.name = document.getElementById("show-name").value;
    showData.bpm_default = parseFloat(document.getElementById("show-bpm").value);

    const tomlContent = generateToml(showData);
    const blob = new Blob([tomlContent], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = "show.toml";
    a.click();
    URL.revokeObjectURL(url);
    showToast("💾 show.toml exported successfully!");
}

function generateToml(data) {
    let toml = `# RPi-VJ Show Manifest Generated by Studio\nschema_version = ${data.schema_version}\nname = "${data.name}"\nbpm_default = ${data.bpm_default}\nbeats_per_bar = ${data.beats_per_bar}\n\n`;
    toml += `[requirements]\nmin_tier = "${data.requirements.min_tier}"\nvideo_decks = ${data.requirements.video_decks}\nshader_layers = ${data.requirements.shader_layers}\nrender_scale = ${data.requirements.render_scale}\n\n`;

    data.banks.forEach((bank) => {
        toml += `[[banks]]\nid = ${bank.id}\nname = "${bank.name}"\n\n`;
    });

    return toml;
}

window.onload = init;
