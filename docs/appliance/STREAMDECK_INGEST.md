# Appliance V3 Stream Deck Standalone USB Ingest Workflow

This document details the native standalone USB ingest, media analysis decision, transcode management, and show generation activation workflow for the **Elgato Stream Deck XL** control surface on **RPi-VJ Appliance V3** platform, implementing task **A3-11** and adhering to [`tasks/APPLIANCE-V3/ARCHITECTURE.md`](file:///c:/Users/Admin/Projects/RPi-VJ/tasks/APPLIANCE-V3/ARCHITECTURE.md) and [`docs/appliance/USB_INGEST.md`](file:///c:/Users/Admin/Projects/RPi-VJ/docs/appliance/USB_INGEST.md).

---

## 1. Architectural Principles

1. **Standalone Appliance Operation:** Media import, media analysis inspection, policy overrides, transcode execution, show placement, and final activation can be completed entirely using only the Stream Deck XL and HDMI TV output without connecting an external PC or display.
2. **Deterministic Non-Blocking UI:** Probing, hashing, transcoding, and `fsync` writes execute asynchronously over decoupled background workers. Button events provide instantaneous feedback (< 80 ms latency) and bounded icon updates.
3. **Playback Priority & Zero-Stutter Ingest Yielding:** Heavy ingest and transcode jobs automatically yield CPU and pause when the operator switches to or arms `PERFORMANCE` mode. Background transcode never interferes with active playback.
4. **Guarded Multi-Step Protections:** Destructive or costly choices (e.g. Cancel Ingest, Delete Show Staging, Activate Show Generation, Safe Eject USB Drive) require a 3-second sustained hold timer with continuous circular animation and edge release triggers.
5. **Durable Ledger & Power Loss Resilience:** All discovered clips and state transitions are backed by `/var/lib/rpi-vj/ingest-jobs.jsonl` with `fsync`. Power interruption cleanly quarantines incomplete staging directories without corrupting active shows.

---

## 2. Stream Deck XL 32-Key Ingest Page Layout

The `INGEST` page maps 32 hardware keys (4 rows × 8 columns) into a dedicated operator console:

```text
┌──────────────┬──────────────┬──────────────┬──────────────┬──────────────┬──────────────┬──────────────┬──────────────┐
│  Key 0: USB  │  Key 1: STG  │  Key 2: PRB  │  Key 3: ANL  │  Key 4: TRN  │  Key 5: VAL  │  Key 6: ADV  │  Key 7: TOT  │
│  DRIVE STAT  │   1.STAGE    │   2.PROBE    │  3.ANALYZE   │ 4.TRANSCODE  │  5.VALIDATE  │  WARN/CLEAN  │   PROGRESS   │
├──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┤
│  Key 8..9:   │  Key 10..11: │  Key 12..13: │  Key 14..15: │              │              │              │              │
│  FILE QUEUE  │ CURRENT ITEM │ MEDIA ANL REC│ HEADROOM/RSV │              │              │              │              │
│  (e.g. 1/5)  │  (TRUNCATED) │(QUAD/SEAM/OK)│  (15% CHECK) │              │              │              │              │
├──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┤
│ Key 16: ACC  │ Key 17: FULL │ Key 18: CROP │ Key 19: SEAM │ Key 20: SKIP │ Key 21..23:  │              │              │
│ ACCEPT REC   │ KEEP FULL FRM│ CROP QUADRANT│ REPAIR SEAM  │  SKIP FILE   │ TARGET SPECS │              │              │
│  (RECOMMEND) │(OVERRIDE QD) │ (FORCE QUAD) │ (BLEND LOOP) │ (NEXT ITEM)  │(96x96 + FHD) │              │              │
├──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┼──────────────┤
│ Key 24: START│ Key 25: CNCL │ Key 26: DEL  │ Key 27: ACT  │ Key 28: EJCT │ Key 29..30:  │ Key 31: EXIT │              │
│ START / NEXT │ CANCEL JOB   │ DELETE SHOW  │ ACTIVATE GEN │ SAFE EJECT   │ FSYNC STATUS │ RETURN READY │              │
│ (IMMEDIATE)  │ (HOLD 3 SEC) │ (HOLD 3 SEC) │ (HOLD 3 SEC) │ (HOLD 3 SEC) │  (VERIFIED)  │  (STANDBY)   │              │
└──────────────┴──────────────┴──────────────┴──────────────┴──────────────┴──────────────┴──────────────┴──────────────┘
```

---

## 3. Interactive Decision Engine

When candidate media is analyzed, heuristic detection provides advisory classification:

| Media Recommendation | Detection Pattern | Default Proposed Action | Manual Override Keys |
|---|---|---|---|
| **`QUAD_MIRROR`** | 2x2 mirrored / identical quadrant tiles detected | Crop top-left quadrant (`crop = true`) | **Key 17 (`KEEP FULL FRAME`)** / **Key 18 (`CROP QUADRANT`)** |
| **`SEAM_GAP`** | Loop end discontinuity / abrupt seam jump | Apply loop crossfade blend (`seam = true`) | **Key 19 (`REPAIR SEAM`)** |
| **`QUAD+SEAM`** | Both quad mirror and seam gap detected | Crop quadrant and repair loop seam | **Key 17**, **Key 18**, **Key 19** |
| **`REC: OK`** | Clean single-stream video with smooth loop | Direct transcode without crop/blend | **Key 16 (`ACCEPT RECOMMEND`)** |
| **`BAD FORMAT`** | Unreadable / unsupported codec | Skip file recommendation | **Key 20 (`SKIP FILE`)** |

- **Key 16 (`ACCEPT RECOMMEND`):** Automatically commits the heuristic policy and proceeds with proposed transcode options.
- **Key 17 (`KEEP FULL FRAME`):** Explicitly overrides quadrant splitting, preserving full uncropped frame.
- **Key 18 (`CROP QUADRANT`):** Forces 2x2 quadrant isolation regardless of heuristic score.
- **Key 19 (`REPAIR SEAM`):** Forces a smooth 15-frame crossfade blend loop.
- **Key 20 (`SKIP FILE`):** Bypasses current item in queue without processing.

---

## 4. Guarded Operations & Safety Guarantees

1. **Guarded Cancellation (Key 25, 3s hold):**
   - Aborts active ingest job.
   - Cleans up partial staging artifacts in `/srv/rpi-vj/staging/job-<uuid>/`.
   - Transitions job in durable ledger to `Cancelled`.
2. **Guarded Show Activation (Key 27, 3s hold):**
   - Promotes completed staging directory into `/srv/rpi-vj/shows/gen-<timestamp>-<hash>`.
   - Flushes directory contents with `fsync`.
   - Atomically updates `/srv/rpi-vj/current` symlink.
3. **Guarded Safe Eject (Key 28, 3s hold):**
   - Safely invokes `umount` for `/media/usb-*` mount points.
   - Flushes OS buffer caches.
   - Key icon updates to green `SAFE NOW / EJECTED`.

---

## 5. Performance Mode Integration

When the operator switches from `READY` or `INGEST` to `PERFORMANCE` mode:
1. `UIEngine` detects page transition and immediately sends `ingest.pause_transcode`.
2. Active FFmpeg transcode processes yield CPU immediately to eliminate frame drops or renderer stutter.
3. Ingest step 4 icon displays `PAUSED`.
4. When leaving `PERFORMANCE` mode, `ingest.resume_transcode` automatically unpauses the background worker.

---

## 6. Verification & Test Suite

The workflow is validated in [`vj/crates/vj-surface-hid/tests/state_ui_integration.rs`](file:///c:/Users/Admin/Projects/RPi-VJ/vj/crates/vj-surface-hid/tests/state_ui_integration.rs):
- `test_observable_a3_11_streamdeck_usb_ingest_complete_workflow`:
  * 5 mixed clips discovered from `/RPI-VJ-INBOX/`.
  * Decision toggles (`ACCEPT`, `KEEP FULL FRAME`, `CROP`, `REPAIR SEAM`, `SKIP`).
  * Performance mode pause/resume verification.
  * Guarded safe eject (3-second hold).
  * Guarded show activation (3-second hold).
  * Persistence of activated show generation across simulated reboots.
