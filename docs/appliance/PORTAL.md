# RPi-VJ Appliance V3 — LAN Ingest Portal & Discovery Specification

## 1. Overview

The LAN Ingest Portal provides an authenticated web interface and resumable chunked upload API embedded directly within `vj-broker`. It enables zero-configuration media ingest and show activation from macOS, Windows, and Linux laptops over the local network via `http://rpi-vj.local/` (or `http://<ip>:8080/portal/`) without requiring SSH access, fixed IPs, or direct writes to active show media.

```text
  Client Browser / CLI
 (http://rpi-vj.local:8080)
            │
            ▼
┌────────────────────────────────────────────────────────────────────────┐
│  vj-broker Embedded HTTP Portal Server                                 │
│                                                                        │
│   ├── /portal/               (Embedded Responsive Web Dashboard)       │
│   ├── /api/v1/portal/auth/   (Setup Code Verification & Token Auth)    │
│   ├── /api/v1/portal/upload/ (Resumable Chunked Ingest API)            │
│   └── /api/v1/portal/status  (Appliance Health, Jobs, Storage Bar)     │
└───────────────────┬────────────────────────────────────────────────────┘
                    │
                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│  Storage Ingest Pipeline & Safety Gates                                │
│                                                                        │
│   1. Path Traversal & Name Sanitization (Strict allowlist)             │
│   2. 15% Storage Reserve Enforcement (Reject upfront if <15% free)     │
│   3. Staged Chunked Assembly (/srv/rpi-vj/staging/job-<uuid>/)         │
│   4. Fsync & Multi-level SHA-256 Checksum Validation                   │
│   5. Durable IngestJob Ledger Logging (/var/lib/rpi-vj/ingest-jobs.jsonl)│
│   6. Atomic Show Promotion & Symlink Swap (/srv/rpi-vj/current)        │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 2. Network Discovery & Hostname Resolution

- **mDNS Hostname**: `rpi-vj.local` broadcast by `avahi-daemon` on standard multicast UDP port 5353.
- **Port**: Default listen address `0.0.0.0:8080`.
- **Stream Deck Hardware Fallback**: The Stream Deck System status page displays the hostname and current primary IPv4 address (e.g. `192.168.1.42:8080`), ensuring the portal is accessible even if mDNS resolution is disabled or blocked on corporate networks.

---

## 3. Security & Authentication Architecture

### 3.1 One-Time Setup Code & Token Lifecycle
1. **Setup Code Generation**: On first boot, `vj-broker` auto-generates a secure setup/recovery code formatted as `RPI-VJ-XXXX-YYYY` and writes it to `/var/lib/rpi-vj/portal-auth.json` (mode `0600`).
2. **Setup Code Display**: Displayed in logs and visible on the Stream Deck system page for physical operator verification.
3. **Authentication Endpoint**: `POST /api/v1/portal/auth/login` verifies the setup code using constant-time comparison to prevent timing side-channel attacks.
4. **Session Token**: On successful login, an ephemeral bearer token (`vj_tok_<uuid>`) is issued with a 24-hour expiration window.
5. **Rate Limiting**: Failed authentication attempts are tracked per client IP. If a client exceeds **5 failed attempts within 60 seconds**, subsequent requests receive `HTTP 429 Too Many Requests` with a backoff timer.

### 3.2 Path Traversal & Payload Sanitization
- Filenames are strictly validated against directory traversal patterns (`..`, `/`, `\`, `\0`, control characters, Windows device names `CON`, `NUL`, etc.).
- File extensions are restricted to approved media formats (`.mp4`, `.mov`, `.mkv`, `.webm`, `.avi`, `.m4v`, `.ts`, `.jpg`, `.jpeg`, `.png`, `.gif`, `.json`, `.mp3`, `.wav`, `.flac`).
- Uploads cannot target arbitrary paths: files are staged strictly inside `/srv/rpi-vj/staging/job-<uuid>/`.

---

## 4. Resumable Chunked Upload Protocol

The upload protocol allows large media files (up to 50 GB) to be transferred in discrete chunks (default 2 MB) with pause/resume resilience and chunk-level integrity checking.

```text
 Client                                              Broker (vj-broker)
   │                                                         │
   ├─── 1. POST /api/v1/portal/upload/init ─────────────────►│ (Check 15% reserve, create job)
   │◄── 201 Created { upload_id, job_id } ───────────────────┤
   │                                                         │
   ├─── 2. POST /api/v1/portal/upload/chunk (chunk 0) ──────►│ (Write offset 0, update progress)
   │◄── 200 OK { received_chunks: [0], progress: 25% } ──────┤
   │                                                         │
   │  [Connection Drop / Interruption]                       │
   │                                                         │
   ├─── 3. GET /api/v1/portal/upload/status?upload_id=... ──►│ (Query missing chunks)
   │◄── 200 OK { received_chunks: [0], total: 4 } ───────────┤
   │                                                         │
   ├─── 4. POST /api/v1/portal/upload/chunk (chunks 1,2,3) ─►│ (Resume missing chunks)
   │◄── 200 OK { is_complete: true, progress: 100% } ────────┤
   │                                                         │
   ├─── 5. POST /api/v1/portal/upload/complete ─────────────►│ (Fsync, SHA-256 verify, probe)
   │◄── 200 OK { status: "complete", job: { state: "Ready" }┤
   │                                                         │
   ├─── 6. POST /api/v1/portal/activate ────────────────────►│ (Atomic symlink swap to active)
   │◄── 200 OK { active_generation: "gen-20260915..." } ─────┤
```

### 4.1 Endpoints Specification

#### `POST /api/v1/portal/upload/init`
Initializes a new staged ingest session.
- **Headers**: `Authorization: Bearer <token>`, `Content-Type: application/json`
- **Request Body**:
  ```json
  {
    "filename": "laser-loop-1080p.mp4",
    "filesize": 104857600,
    "chunk_size": 2097152,
    "total_chunks": 50,
    "sha256": "optional-expected-sha256-hash"
  }
  ```
- **Response `201 Created`**:
  ```json
  {
    "upload_id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
    "job_id": "7b1029c3-1129-47a2-92e1-456d9876a301",
    "filename": "laser-loop-1080p.mp4",
    "total_size": 104857600,
    "chunk_size": 2097152,
    "total_chunks": 50,
    "received_chunks": [],
    "progress_percent": 0.0,
    "is_complete": false
  }
  ```

#### `POST /api/v1/portal/upload/chunk`
Transfers an individual binary chunk into the staged file.
- **Headers**:
  - `Authorization: Bearer <token>`
  - `Content-Type: application/octet-stream`
  - `X-Upload-Id: <uuid>` (or query param `?upload_id=<uuid>`)
  - `X-Chunk-Index: <usize>` (or query param `?index=<usize>`)
  - `X-Chunk-Offset: <u64>` (or query param `?offset=<u64>`)
  - `X-Chunk-Sha256: <hex>` (optional chunk checksum)
- **Request Body**: Raw binary bytes of the chunk.
- **Response `200 OK`**:
  ```json
  {
    "upload_id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
    "job_id": "7b1029c3-1129-47a2-92e1-456d9876a301",
    "filename": "laser-loop-1080p.mp4",
    "total_size": 104857600,
    "chunk_size": 2097152,
    "total_chunks": 50,
    "received_chunks": [0, 1, 2],
    "progress_percent": 6.0,
    "is_complete": false
  }
  ```

#### `GET /api/v1/portal/upload/status?upload_id=<uuid>`
Returns current receipt bitmap for resuming interrupted transfers.
- **Response `200 OK`**:
  ```json
  {
    "upload_id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
    "job_id": "7b1029c3-1129-47a2-92e1-456d9876a301",
    "filename": "laser-loop-1080p.mp4",
    "total_size": 104857600,
    "chunk_size": 2097152,
    "total_chunks": 50,
    "received_chunks": [0, 1, 2, 5, 6],
    "progress_percent": 10.0,
    "is_complete": false
  }
  ```

#### `POST /api/v1/portal/upload/complete`
Finalizes transfer, executes `fsync`, verifies full-file SHA-256, validates container format, and promotes job state in `JobStore` to `Ready`.
- **Request Body**:
  ```json
  {
    "upload_id": "3fa85f64-5717-4562-b3fc-2c963f66afa6",
    "expected_sha256": "optional-sha256-hex"
  }
  ```
- **Response `200 OK`**:
  ```json
  {
    "status": "complete",
    "job": {
      "id": "7b1029c3-1129-47a2-92e1-456d9876a301",
      "state": "ready",
      "progress_percent": 100.0,
      "details": {
        "format": "video/mp4",
        "sha256": "a3f5...",
        "validation": "passed"
      }
    }
  }
  ```

#### `POST /api/v1/portal/activate`
Atomically installs and activates a staged show generation.
- **Request Body**:
  ```json
  {
    "job_id": "7b1029c3-1129-47a2-92e1-456d9876a301"
  }
  ```
- **Response `200 OK`**:
  ```json
  {
    "status": "activated",
    "active_generation": "gen-20260915120000-abcd1234"
  }
  ```

#### `POST /api/v1/portal/rollback`
Atomically reverts `/srv/rpi-vj/current` to the previous installed show generation.
- **Response `200 OK`**:
  ```json
  {
    "status": "rolled_back",
    "active_generation": "gen-20260915100000-efgh5678"
  }
  ```

---

## 5. Web UI Features

The portal Web UI is served at `/portal/` as a zero-dependency, self-contained single-page application:
- **Appliance Identity Banner**: Live display of hostname (`rpi-vj.local`), current IP, monotonic uptime, and supervisor health status.
- **Storage Reserve Gauge**: Real-time visual capacity bar displaying used and free gigabytes with a marked 15% protected reserve threshold.
- **Active Generation & Rollback**: Displays the active show generation with one-click rollback to the previous generation.
- **Resumable Chunked Ingest Zone**: Drag-and-drop zone with real-time multi-chunk progress indicator, retry/resume handling, and client-side SHA-256 calculation.
- **Job Ledger Table**: Live view of all jobs in the append-only `JobStore` ledger, state badges, validation outcomes, and show activation controls.
- **Setup Code Modal**: Clean modal for entering the one-time setup code stored at `/var/lib/rpi-vj/portal-auth.json`.
