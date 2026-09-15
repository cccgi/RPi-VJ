pub fn embedded_portal_html() -> String {
    concat!(
r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>RPi-VJ Appliance — Ingest Portal</title>
    <style>
        :root {
            --bg-color: #0d0f18;
            --card-bg: #151824;
            --border-color: #272c3d;
            --text-primary: #e6edf3;
            --text-secondary: #8b949e;
            --accent-cyan: #00ffcc;
            --accent-purple: #bc8cff;
            --accent-pink: #ff7b72;
            --accent-green: #3fb950;
            --accent-yellow: #d29922;
            --danger: #f85149;
            --font-mono: "JetBrains Mono", Consolas, Menlo, Monaco, monospace;
            --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
        }

        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            background-color: var(--bg-color);
            color: var(--text-primary);
            font-family: var(--font-sans);
            line-height: 1.5;
            padding: 24px;
            min-height: 100vh;
        }

        .container {
            max-width: 1100px;
            margin: 0 auto;
            display: flex;
            flex-direction: column;
            gap: 24px;
        }

        /* Header */
        header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            border-bottom: 1px solid var(--border-color);
            padding-bottom: 16px;
        }
        .brand {
            display: flex;
            align-items: center;
            gap: 12px;
        }
        .logo-badge {
            background: linear-gradient(135deg, var(--accent-cyan), var(--accent-purple));
            color: #000;
            font-weight: 900;
            font-family: var(--font-mono);
            padding: 6px 12px;
            border-radius: 6px;
            font-size: 16px;
            letter-spacing: 1px;
        }
        .brand-title {
            font-size: 20px;
            font-weight: 700;
            letter-spacing: 0.5px;
        }
        .system-pill {
            display: flex;
            align-items: center;
            gap: 8px;
            background: var(--card-bg);
            border: 1px solid var(--border-color);
            padding: 6px 14px;
            border-radius: 20px;
            font-size: 13px;
            font-family: var(--font-mono);
        }
        .status-dot {
            width: 10px;
            height: 10px;
            border-radius: 50%;
            background: var(--accent-green);
            box-shadow: 0 0 8px var(--accent-green);
        }
        .status-dot.degraded {
            background: var(--danger);
            box-shadow: 0 0 8px var(--danger);
        }

        /* Top Grid */
        .grid-2 {
            display: grid;
            grid-template-columns: 1fr 1fr;
            gap: 20px;
        }
        @media (max-width: 768px) {
            .grid-2 { grid-template-columns: 1fr; }
        }

        .card {
            background: var(--card-bg);
            border: 1px solid var(--border-color);
            border-radius: 10px;
            padding: 20px;
            display: flex;
            flex-direction: column;
            gap: 14px;
        }
        .card-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            font-weight: 600;
            font-size: 15px;
            color: var(--text-secondary);
            text-transform: uppercase;
            letter-spacing: 1px;
        }

        /* Storage Bar */
        .storage-bar-container {
            position: relative;
            background: #21262d;
            height: 24px;
            border-radius: 6px;
            overflow: hidden;
            margin-top: 8px;
        }
        .storage-bar-fill {
            background: linear-gradient(90deg, #238636, #3fb950);
            height: 100%;
            width: 0%;
            transition: width 0.4s ease;
        }
        .storage-bar-fill.warning {
            background: linear-gradient(90deg, #d29922, #f0883e);
        }
        .storage-reserve-line {
            position: absolute;
            right: 15%;
            top: 0;
            bottom: 0;
            width: 2px;
            background: var(--accent-pink);
            z-index: 2;
        }
        .storage-labels {
            display: flex;
            justify-content: space-between;
            font-size: 13px;
            font-family: var(--font-mono);
            color: var(--text-secondary);
        }

        /* Active Show */
        .show-info {
            display: flex;
            flex-direction: column;
            gap: 6px;
        }
        .gen-badge {
            font-family: var(--font-mono);
            color: var(--accent-cyan);
            font-size: 16px;
            font-weight: 700;
            word-break: break-all;
        }
        .btn-group {
            display: flex;
            gap: 10px;
            margin-top: 8px;
        }
        button {
            cursor: pointer;
            border: none;
            border-radius: 6px;
            padding: 8px 16px;
            font-weight: 600;
            font-size: 13px;
            transition: all 0.2s ease;
            font-family: var(--font-sans);
        }
        .btn-primary {
            background: var(--accent-cyan);
            color: #000;
        }
        .btn-primary:hover {
            opacity: 0.9;
            box-shadow: 0 0 10px rgba(0, 255, 204, 0.4);
        }
        .btn-secondary {
            background: #21262d;
            color: var(--text-primary);
            border: 1px solid var(--border-color);
        }
        .btn-secondary:hover {
            background: #30363d;
        }
        .btn-danger {
            background: #b62324;
            color: #fff;
        }
        .btn-danger:hover {
            background: #e5534b;
        }
        button:disabled {
            opacity: 0.4;
            cursor: not-allowed;
        }

        /* Upload Zone */
        .dropzone {
            border: 2px dashed #30363d;
            border-radius: 10px;
            padding: 36px 20px;
            text-align: center;
            background: #0e111a;
            cursor: pointer;
            transition: border-color 0.2s, background-color 0.2s;
        }
        .dropzone.dragover {
            border-color: var(--accent-cyan);
            background: rgba(0, 255, 204, 0.05);
        }
        .dropzone h3 {
            font-size: 16px;
            margin-bottom: 6px;
            color: var(--text-primary);
        }
        .dropzone p {
            font-size: 13px;
            color: var(--text-secondary);
        }

        /* Uploading progress card */
        .upload-progress-box {
            display: none;
            flex-direction: column;
            gap: 10px;
            background: #0e111a;
            border: 1px solid var(--border-color);
            border-radius: 8px;
            padding: 16px;
            margin-top: 12px;
        }
        .progress-bar-container {
            background: #21262d;
            height: 12px;
            border-radius: 6px;
            overflow: hidden;
        }
        .progress-bar-fill {
            background: linear-gradient(90deg, var(--accent-cyan), var(--accent-purple));
            height: 100%;
            width: 0%;
            transition: width 0.2s ease;
        }
        .upload-details {
            display: flex;
            justify-content: space-between;
            font-size: 12px;
            font-family: var(--font-mono);
            color: var(--text-secondary);
        }

        /* Jobs Table */
        .table-wrap {
            overflow-x: auto;
        }
        table {
            width: 100%;
            border-collapse: collapse;
            font-size: 13px;
            text-align: left;
        }
        th, td {
            padding: 12px 14px;
            border-bottom: 1px solid var(--border-color);
        }
        th {
            color: var(--text-secondary);
            font-weight: 600;
            text-transform: uppercase;
            font-size: 11px;
            letter-spacing: 0.5px;
        }
        .pill {
            display: inline-block;
            padding: 3px 8px;
            border-radius: 12px;
            font-size: 11px;
            font-family: var(--font-mono);
            font-weight: 600;
            text-transform: uppercase;
        }
        .pill-discovered { background: #30363d; color: #8b949e; }
        .pill-copying { background: #1f3a5f; color: #58a6ff; }
        .pill-validating { background: #3c2a63; color: var(--accent-purple); }
        .pill-ready { background: #1b4b2c; color: var(--accent-green); }
        .pill-activating { background: #5c4416; color: var(--accent-yellow); }
        .pill-complete { background: #10422a; color: var(--accent-cyan); }
        .pill-failed { background: #4e1919; color: var(--accent-pink); }
        .pill-cancelled { background: #30363d; color: #6e7681; }

        /* Auth Modal */
        .modal-backdrop {
            display: none;
            position: fixed;
            top: 0; left: 0; right: 0; bottom: 0;
            background: rgba(0, 0, 0, 0.8);
            backdrop-filter: blur(4px);
            z-index: 100;
            align-items: center;
            justify-content: center;
        }
        .modal {
            background: var(--card-bg);
            border: 1px solid var(--border-color);
            border-radius: 12px;
            width: 90%;
            max-width: 420px;
            padding: 28px;
            display: flex;
            flex-direction: column;
            gap: 16px;
            box-shadow: 0 10px 30px rgba(0,0,0,0.5);
        }
        .modal h2 {
            font-size: 18px;
        }
        .modal input {
            background: #0d0f18;
            border: 1px solid var(--border-color);
            border-radius: 6px;
            padding: 12px;
            color: #fff;
            font-family: var(--font-mono);
            font-size: 16px;
            text-align: center;
            letter-spacing: 2px;
        }
        .modal input:focus {
            outline: none;
            border-color: var(--accent-cyan);
            box-shadow: 0 0 8px rgba(0,255,204,0.3);
        }
        .auth-error {
            color: var(--danger);
            font-size: 12px;
            text-align: center;
            display: none;
        }
    </style>
</head>
<body>
    <div class="container">
        <!-- Header -->
        <header>
            <div class="brand">
                <span class="logo-badge">RPI-VJ</span>
                <span class="brand-title">Ingest Portal</span>
            </div>
            <div class="system-pill">
                <span class="status-dot" id="system-dot"></span>
                <span id="system-host">rpi-vj.local</span>
                <span style="color: var(--text-secondary);">&bull;</span>
                <span id="system-ip">127.0.0.1</span>
            </div>
        </header>

        <!-- Grid 1: Storage & Active Generation -->
        <div class="grid-2">
            <!-- Storage Card -->
            <div class="card">
                <div class="card-header">
                    <span>Appliance Storage</span>
                    <span id="storage-status-text" style="color: var(--accent-green); font-size: 12px;">15% Reserve Intact</span>
                </div>
                <div>
                    <div class="storage-bar-container">
                        <div class="storage-bar-fill" id="storage-bar-fill"></div>
                        <div class="storage-reserve-line" title="15% Free Space Reserve Boundary"></div>
                    </div>
                    <div class="storage-labels" style="margin-top: 6px;">
                        <span id="storage-used">Used: 0 GB</span>
                        <span id="storage-free">Free: 0 GB (0%)</span>
                    </div>
                </div>
            </div>

            <!-- Active Show Card -->
            <div class="card">
                <div class="card-header">
                    <span>Active Show Generation</span>
                    <span id="show-status-pill" class="pill pill-ready">LIVE</span>
                </div>
                <div class="show-info">
                    <span class="gen-badge" id="active-gen-name">gen-default</span>
                    <span style="font-size: 12px; color: var(--text-secondary);" id="active-gen-path">/srv/rpi-vj/current</span>
                </div>
                <div class="btn-group">
                    <button class="btn-secondary" id="btn-rollback" onclick="rollbackGeneration()">Rollback to Previous</button>
                    <button class="btn-primary" id="btn-refresh" onclick="fetchStatus()">Refresh Status</button>
                </div>
            </div>
        </div>

        <!-- Ingest Drag-and-Drop & Resumable Upload Area -->
        <div class="card">
            <div class="card-header">
                <span>LAN Media Ingest (Resumable Chunked Upload)</span>
                <span style="font-size: 12px; color: var(--text-secondary);">Chunk Size: 2 MB</span>
            </div>

            <div class="dropzone" id="dropzone" onclick="document.getElementById('file-input').click()">
                <h3>Drag & Drop Video / Loop File</h3>
                <p>Supports .mp4, .mov, .mkv, .webm &bull; Chunked, verified, & resumable into /srv/rpi-vj/staging/</p>
                <input type="file" id="file-input" style="display: none;" onchange="handleFileSelected(this.files[0])">
            </div>

            <div class="upload-progress-box" id="upload-box">
                <div style="display: flex; justify-content: space-between; align-items: center;">
                    <strong id="upload-filename" style="font-family: var(--font-mono); font-size: 14px;">filename.mp4</strong>
                    <span id="upload-percent" style="font-family: var(--font-mono); color: var(--accent-cyan); font-weight: bold;">0%</span>
                </div>
                <div class="progress-bar-container">
                    <div class="progress-bar-fill" id="upload-bar-fill"></div>
                </div>
                <div class="upload-details">
                    <span id="upload-chunks">Chunk 0/0</span>
                    <span id="upload-bytes">0 / 0 MB</span>
                    <span id="upload-status">Uploading...</span>
                </div>
            </div>
        </div>

        <!-- Queued Jobs Table -->
        <div class="card">
            <div class="card-header">
                <span>Job Ledger & Validation Outcomes</span>
                <span id="job-count" style="font-size: 12px; color: var(--text-secondary);">0 Jobs</span>
            </div>
            <div class="table-wrap">
                <table>
                    <thead>
                        <tr>
                            <th>Job ID</th>
                            <th>Filename</th>
                            <th>State</th>
                            <th>Progress</th>
                            <th>Validation / Hash</th>
                            <th>Action</th>
                        </tr>
                    </thead>
                    <tbody id="jobs-tbody">
                        <tr><td colspan="6" style="text-align: center; color: var(--text-secondary); padding: 24px;">No ingest jobs in ledger</td></tr>
                    </tbody>
                </table>
            </div>
        </div>
    </div>

    <!-- Auth Setup Code Modal -->
    <div class="modal-backdrop" id="auth-modal">
        <div class="modal">
            <h2>Portal Authentication</h2>
            <p style="font-size: 13px; color: var(--text-secondary);">Enter the one-time Setup / Recovery Code generated at <code>/var/lib/rpi-vj/portal-auth.json</code> or shown on Stream Deck.</p>
            <input type="text" id="auth-code-input" placeholder="RPI-VJ-XXXX-YYYY" autofocus>
            <div class="auth-error" id="auth-error">Invalid setup code or rate limit active</div>
            <button class="btn-primary" onclick="submitAuthCode()">Unlock Ingest Portal</button>
        </div>
    </div>

    <script>
        const CHUNK_SIZE = 2 * 1024 * 1024; // 2MB chunks
        let authToken = localStorage.getItem("vj_portal_token") || "";
        let currentUpload = null;

        // Init
        document.addEventListener("DOMContentLoaded", () => {
            setupDropzone();
            fetchStatus();
            setInterval(fetchStatus, 3000);
        });

        // Dropzone handlers
        function setupDropzone() {
            const dz = document.getElementById("dropzone");
            dz.addEventListener("dragover", (e) => { e.preventDefault(); dz.classList.add("dragover"); });
            dz.addEventListener("dragleave", () => { dz.classList.remove("dragover"); });
            dz.addEventListener("drop", (e) => {
                e.preventDefault();
                dz.classList.remove("dragover");
                if (e.dataTransfer.files && e.dataTransfer.files[0]) {
                    handleFileSelected(e.dataTransfer.files[0]);
                }
            });
        }

        async function fetchStatus() {
            try {
                const res = await fetch("/api/v1/portal/status", {
                    headers: authToken ? { "Authorization": `Bearer ${authToken}` } : {}
                });

                if (res.status === 401) {
                    showAuthModal();
                    return;
                }
                hideAuthModal();

                if (!res.ok) return;
                const data = await res.json();
                renderStatus(data);
            } catch (err) {
                console.warn("Failed to fetch status", err);
            }
        }

        function renderStatus(data) {
            // Identity
            if (data.hostname) document.getElementById("system-host").textContent = data.hostname;
            if (data.ip) document.getElementById("system-ip").textContent = data.ip;

            // Supervisor health
            const dot = document.getElementById("system-dot");
            if (data.supervisor && data.supervisor.durable_degraded) {
                dot.classList.add("degraded");
            } else {
                dot.classList.remove("degraded");
            }

            // Storage
            if (data.storage) {
                const total = data.storage.total_bytes || 1;
                const free = data.storage.free_bytes || 0;
                const used = total - free;
                const usedPct = Math.min(100, (used / total) * 100).toFixed(1);
                const freePct = (100 - usedPct).toFixed(1);

                const bar = document.getElementById("storage-bar-fill");
                bar.style.width = `${usedPct}%`;
                if (freePct < 15.0) {
                    bar.classList.add("warning");
                    document.getElementById("storage-status-text").textContent = "LOW SPACE (<15% Reserve)";
                    document.getElementById("storage-status-text").style.color = "var(--danger)";
                } else {
                    bar.classList.remove("warning");
                    document.getElementById("storage-status-text").textContent = "15% Reserve Intact";
                    document.getElementById("storage-status-text").style.color = "var(--accent-green)";
                }

                document.getElementById("storage-used").textContent = `Used: ${(used / 1e9).toFixed(2)} GB (${usedPct}%)`;
                document.getElementById("storage-free").textContent = `Free: ${(free / 1e9).toFixed(2)} GB (${freePct}%)`;
            }

            // Active generation
            if (data.active_generation) {
                document.getElementById("active-gen-name").textContent = data.active_generation.name || "None";
                document.getElementById("active-gen-path").textContent = data.active_generation.path || "/srv/rpi-vj/current";
            }

            // Jobs
            renderJobs(data.jobs || []);
        }

        function renderJobs(jobs) {
            const tbody = document.getElementById("jobs-tbody");
            document.getElementById("job-count").textContent = `${jobs.length} Job${jobs.length === 1 ? '' : 's'}`;

            if (jobs.length === 0) {
                tbody.innerHTML = `<tr><td colspan="6" style="text-align: center; color: var(--text-secondary); padding: 24px;">No ingest jobs in ledger</td></tr>`;
                return;
            }

            tbody.innerHTML = jobs.map(j => {
                const stateKey = typeof j.state === 'string' ? j.state : Object.keys(j.state)[0];
                const stateClass = `pill-${stateKey.toLowerCase()}`;
                const filename = (j.details && j.details.filename) ? j.details.filename : (j.source_path ? j.source_path.split(/[\\/]/).pop() : "media");
                const hash = (j.details && j.details.sha256) ? j.details.sha256.substring(0, 10) + '...' : '-';
                const validation = (j.details && j.details.validation) ? j.details.validation : '-';

                let actionBtn = '';
                if (stateKey.toLowerCase() === 'ready') {
                    actionBtn = `<button class="btn-primary" style="padding: 4px 10px; font-size: 11px;" onclick="activateJob('${j.id}')">Activate Show</button>`;
                } else if (stateKey.toLowerCase() === 'copying' || stateKey.toLowerCase() === 'discovered') {
                    actionBtn = `<button class="btn-danger" style="padding: 4px 10px; font-size: 11px;" onclick="cancelJob('${j.id}')">Cancel</button>`;
                }

                return `
                    <tr>
                        <td style="font-family: var(--font-mono); font-size: 11px;">${j.id.substring(0, 8)}...</td>
                        <td style="font-weight: 600;">${escapeHtml(filename)}</td>
                        <td><span class="pill ${stateClass}">${stateKey}</span></td>
                        <td style="font-family: var(--font-mono); font-size: 12px;">${(j.progress_percent || 0).toFixed(0)}%</td>
                        <td style="font-family: var(--font-mono); font-size: 11px; color: var(--text-secondary);">${validation} / ${hash}</td>
                        <td>${actionBtn}</td>
                    </tr>
                `;
            }).join("");
        }

        async function handleFileSelected(file) {
            if (!file) return;

            const uploadBox = document.getElementById("upload-box");
            const barFill = document.getElementById("upload-bar-fill");
            const txtFilename = document.getElementById("upload-filename");
            const txtPercent = document.getElementById("upload-percent");
            const txtChunks = document.getElementById("upload-chunks");
            const txtBytes = document.getElementById("upload-bytes");
            const txtStatus = document.getElementById("upload-status");

            uploadBox.style.display = "flex";
            txtFilename.textContent = file.name;
            txtStatus.textContent = "Initializing upload...";

            const totalSize = file.size;
            const totalChunks = Math.ceil(totalSize / CHUNK_SIZE);

            try {
                // 1. Init Upload Session
                const initRes = await fetch("/api/v1/portal/upload/init", {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "Authorization": authToken ? `Bearer ${authToken}` : ""
                    },
                    body: JSON.stringify({
                        filename: file.name,
                        filesize: totalSize,
                        chunk_size: CHUNK_SIZE,
                        total_chunks: totalChunks
                    })
                });

                if (!initRes.ok) {
                    const err = await initRes.json().catch(() => ({ error: "Init failed" }));
                    alert(`Upload initialization failed: ${err.error || initRes.statusText}`);
                    uploadBox.style.display = "none";
                    return;
                }

                const initData = await initRes.json();
                const uploadId = initData.upload_id;

                // 2. Upload chunks (resumable)
                let receivedChunks = new Set(initData.received_chunks || []);

                for (let i = 0; i < totalChunks; i++) {
                    if (receivedChunks.has(i)) {
                        continue; // Skip already uploaded chunk
                    }

                    const start = i * CHUNK_SIZE;
                    const end = Math.min(start + CHUNK_SIZE, totalSize);
                    const chunkBlob = file.slice(start, end);
                    const chunkBuffer = await chunkBlob.arrayBuffer();

                    // Optional chunk sha256
                    const hashBuffer = await crypto.subtle.digest("SHA-256", chunkBuffer);
                    const hashArray = Array.from(new Uint8Array(hashBuffer));
                    const chunkSha256 = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');

                    txtChunks.textContent = `Chunk ${i + 1}/${totalChunks}`;
                    txtBytes.textContent = `${(end / 1e6).toFixed(1)} / ${(totalSize / 1e6).toFixed(1)} MB`;
                    txtStatus.textContent = "Uploading chunk...";

                    const chunkRes = await fetch(`/api/v1/portal/upload/chunk?upload_id=${uploadId}&index=${i}&offset=${start}&sha256=${chunkSha256}`, {
                        method: "POST",
                        headers: {
                            "Content-Type": "application/octet-stream",
                            "Authorization": authToken ? `Bearer ${authToken}` : ""
                        },
                        body: chunkBuffer
                    });

                    if (!chunkRes.ok) {
                        const err = await chunkRes.json().catch(() => ({ error: "Chunk upload failed" }));
                        throw new Error(`Chunk ${i} failed: ${err.error || chunkRes.statusText}`);
                    }

                    const chunkData = await chunkRes.json();
                    const pct = chunkData.progress_percent.toFixed(0);
                    barFill.style.width = `${pct}%`;
                    txtPercent.textContent = `${pct}%`;
                }

                // 3. Complete Upload
                txtStatus.textContent = "Validating and finalizing in storage pipeline...";
                const compRes = await fetch("/api/v1/portal/upload/complete", {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "Authorization": authToken ? `Bearer ${authToken}` : ""
                    },
                    body: JSON.stringify({ upload_id: uploadId })
                });

                if (!compRes.ok) {
                    const err = await compRes.json().catch(() => ({ error: "Completion failed" }));
                    throw new Error(`Validation failed: ${err.error || compRes.statusText}`);
                }

                txtStatus.textContent = "Upload complete! Ready for activation.";
                setTimeout(() => {
                    uploadBox.style.display = "none";
                    fetchStatus();
                }, 2000);

            } catch (e) {
                console.error("Upload error", e);
                txtStatus.textContent = `Upload error: ${e.message}`;
                txtStatus.style.color = "var(--danger)";
            }
        }

        async function activateJob(jobId) {
            if (!confirm("Promote staged show and atomically activate as live show?")) return;
            try {
                const res = await fetch("/api/v1/portal/activate", {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "Authorization": authToken ? `Bearer ${authToken}` : ""
                    },
                    body: JSON.stringify({ job_id: jobId })
                });
                const data = await res.json();
                if (res.ok) {
                    alert(`Show activated successfully! Generation: ${data.active_generation}`);
                    fetchStatus();
                } else {
                    alert(`Activation failed: ${data.error || res.statusText}`);
                }
            } catch (err) {
                alert(`Network error activating show: ${err.message}`);
            }
        }

        async function rollbackGeneration() {
            if (!confirm("Atomically rollback current show to previous generation?")) return;
            try {
                const res = await fetch("/api/v1/portal/rollback", {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "Authorization": authToken ? `Bearer ${authToken}` : ""
                    }
                });
                const data = await res.json();
                if (res.ok) {
                    alert(`Rolled back to generation: ${data.active_generation}`);
                    fetchStatus();
                } else {
                    alert(`Rollback failed: ${data.error || res.statusText}`);
                }
            } catch (err) {
                alert(`Error rolling back: ${err.message}`);
            }
        }

        async function cancelJob(jobId) {
            try {
                await fetch("/api/v1/portal/jobs/cancel", {
                    method: "POST",
                    headers: {
                        "Content-Type": "application/json",
                        "Authorization": authToken ? `Bearer ${authToken}` : ""
                    },
                    body: JSON.stringify({ job_id: jobId })
                });
                fetchStatus();
            } catch (e) {}
        }

        function showAuthModal() {
            document.getElementById("auth-modal").style.display = "flex";
        }
        function hideAuthModal() {
            document.getElementById("auth-modal").style.display = "none";
        }

        async function submitAuthCode() {
            const input = document.getElementById("auth-code-input");
            const errorDiv = document.getElementById("auth-error");
            const code = input.value.trim();

            errorDiv.style.display = "none";

            try {
                const res = await fetch("/api/v1/portal/auth/login", {
                    method: "POST",
                    headers: { "Content-Type": "application/json" },
                    body: JSON.stringify({ code })
                });

                if (res.ok) {
                    const data = await res.json();
                    authToken = data.token;
                    localStorage.setItem("vj_portal_token", authToken);
                    hideAuthModal();
                    fetchStatus();
                } else {
                    const err = await res.json().catch(() => ({}));
                    errorDiv.textContent = err.error || "Authentication failed";
                    errorDiv.style.display = "block";
                }
            } catch (e) {
                errorDiv.textContent = `Error: ${e.message}`;
                errorDiv.style.display = "block";
            }
        }

        function escapeHtml(str) {
            return (str || '').replace(/[&<>'"]/g, tag => ({
                '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;'
            }[tag] || tag));
        }
    </script>
</body>
</html>"#
    ).to_string()
}
