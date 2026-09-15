# Appliance USB Discovery & Transactional Ingest Engine

This document details the USB discovery, filesystem mount security, inbox scanning, and transactional staged copy architecture for **RPi-VJ Appliance V3** as specified in [`tasks/APPLIANCE-V3/A3-09-usb-discovery-copy.md`](file:///c:/Users/Admin/Projects/RPi-VJ/tasks/APPLIANCE-V3/A3-09-usb-discovery-copy.md) and [`tasks/APPLIANCE-V3/ARCHITECTURE.md`](file:///c:/Users/Admin/Projects/RPi-VJ/tasks/APPLIANCE-V3/ARCHITECTURE.md).

---

## 1. Architectural Overview

```text
Removable USB Drive (/dev/sdX)
       │
       ▼ (Udev rule: 99-rpi-vj-usb.rules / systemd path trigger)
 Mount Helper (nosuid,nodev,noexec,ro)
       │
       ▼
 Inbox Scanner (Exclusively scans /RPI-VJ-INBOX/)
       │ ── Filters symlinks, OS metadata, dotfiles, unsupported formats
       ▼
 15% Storage Reserve Check (Ensures free_bytes - file_bytes >= 0.15 * total_bytes)
       │
       ▼
 Transactional Streaming Copy & SHA-256 Hasher
       │ ── Guards against: mid-copy removal, short reads, duplicates, collisions
       ▼
 Staging Area (/srv/rpi-vj/staging/job-<uuid>/)
       │ ── fsync durable sync -> JobState: Probing
       ▼
 Safe Unmount (umount /media/usb-sdX) ── Never transcode directly from USB!
```

---

## 2. Removable Media Mounting & Security Boundaries

Removable USB storage devices are untrusted external media. To protect the appliance against code execution, device escalation, and privilege abuse:

1. **Strict Mount Flags**: All removable media partitions are mounted with:
   ```text
   nosuid,nodev,noexec,ro
   ```
2. **Conservative Ownership & Permissions**: For FAT32 and exFAT filesystems lacking POSIX permissions, masks are enforced:
   ```text
   uid=vj,gid=vj,fmask=0137,dmask=0027
   ```
3. **Supported Filesystems**:
   - `vfat` (FAT32)
   - `exfat` (exFAT)
   - `ext4` (Linux ext4)
4. **Read-Only Ingest Read**: Removable media is mounted read-only during media discovery and candidate copying. Receipts (if enabled) are written in a guarded post-activation phase.

---

## 3. Directory Layout & Inbox Scanning Policy

The scanner strictly restricts discovery to the `/RPI-VJ-INBOX/` directory at the root of the mounted filesystem (case-insensitive search):

```text
USB Root/
├── RPI-VJ-INBOX/                     <-- Sole directory scanned for ingest
│   ├── clip1.mp4                     (Candidate file)
│   ├── subfolder/
│   │   └── clip2.mov                 (Candidate file in nested directory)
│   ├── malicious_link.mp4 -> /root   (Ignored & rejected: symlink)
│   ├── .DS_Store                     (Ignored: hidden file / OS metadata)
│   ├── ._clip1.mp4                   (Ignored: AppleDouble metadata)
│   ├── System Volume Information/    (Ignored: OS metadata folder)
│   └── script.sh                     (Ignored: unsupported extension)
└── ignored_outside.mp4               (Ignored: outside /RPI-VJ-INBOX/)
```

### Filtering Invariants
- **Symlinks**: Checked via `fs::symlink_metadata()` without following targets. Symlinks are never traversed or copied, preventing directory traversal and arbitrary file reading attacks.
- **Hidden Files & Dotfiles**: Any file or directory beginning with `.` (e.g., `._*`, `.DS_Store`, `.Trashes`) is filtered out.
- **OS Metadata**: `System Volume Information`, `$RECYCLE.BIN`, `.Spotlight-V100`, `.fseventsd`, `lost+found`, `Thumbs.db`, and `desktop.ini` are explicitly ignored.
- **Special Device Nodes**: Fifos, sockets, block/character devices are skipped.
- **Supported Video Formats**: Whitelist includes `.mp4`, `.mov`, `.mkv`, `.webm`, `.avi`, `.m4v`. Unsupported formats are logged and skipped.

---

## 4. Transactional Staged Copy Engine

### Invariants Enforced During Ingest:

1. **15% Free Space Reserve**:
   - Before accepting any file, the storage manager verifies that:
     $$\text{free\_bytes} - \text{required\_bytes} \ge 0.15 \times \text{total\_bytes}$$
   - If capacity is insufficient, the job is rejected upfront with `StorageError::InsufficientSpace`.

2. **Durable Ledger Transitions**:
   - Each discovered file creates an `IngestJob` in `/var/lib/rpi-vj/ingest-jobs.jsonl` in state `Discovered`.
   - Transitions to `Copying` before reading starts.

3. **Isolated Staging & Collision Prevention**:
   - Output destination is isolated: `/srv/rpi-vj/staging/job-<uuid>/<sanitized_filename>`.
   - Files with identical names in different folders/drives do not collide.

4. **Streaming Copy & Incremental SHA-256**:
   - Data is streamed in 64 KB chunks while updating an in-flight `Sha256` hasher.
   - Progress percentage is recorded periodically in the durable ledger.

5. **Mid-Copy Removal & Short Read Guards**:
   - If the USB drive is unplugged mid-stream or returns an I/O error / EOF before the full declared size is copied:
     - Partial staging file is immediately deleted.
     - Job is transitioned to `Failed { reason: "Mid-copy removal or I/O failure..." }`.
     - Active show playback remains completely untouched.

6. **Content Deduplication & Reinsertion**:
   - Computed SHA-256 is checked against existing records in `JobStore`.
   - If identical content already exists, the staging copy is cleaned up and recorded as `Duplicate` without wasting disk space.
   - Reinserting a drive with already-completed jobs skips redundant copying (`SkippedAlreadyProcessed`).

7. **Durable Fsync & State Promotion**:
   - The completed staging file is synced via `fsync` (`sync_all()`).
   - The SHA-256 digest is stored in `job.details["sha256"]`.
   - The job is promoted to `Probing` for subsequent `ffprobe` and transcode analysis.

8. **Safe Unmounting**:
   - Once all queue items for the device are copied, `umount` is invoked cleanly.
   - **Video transcoding and playback NEVER execute directly from removable USB media.**

---

## 5. Systemd & Udev Trigger Specifications

### Udev Rule (`/etc/udev/rules.d/99-rpi-vj-usb.rules`)
```udev
ACTION=="add", SUBSYSTEM=="block", ENV{DEVTYPE}=="partition", ENV{ID_BUS}=="usb", TAG+="systemd", ENV{SYSTEMD_WANTS}+="rpi-vj-usb@%k.service"
```

### Systemd Ingest Service Template (`/etc/systemd/system/rpi-vj-usb@.service`)
```ini
[Unit]
Description=RPi-VJ Removable USB Ingest Trigger (%I)
Documentation=file:///docs/appliance/USB_INGEST.md
After=local-fs.target
BindsTo=dev-%i.device

[Service]
Type=oneshot
User=root
ExecStart=/opt/rpi-vj/bin/rpi-vj-usb-helper /dev/%I
TimeoutSec=300
```

---

## 6. Test Suite & Verification

The test suite in [`crates/vj-storage/tests/usb_ingest_tests.rs`](file:///c:/Users/Admin/Projects/RPi-VJ/vj/crates/vj-storage/tests/usb_ingest_tests.rs) provides automated verification for:

- `test_normal_usb_media_copy`: Normal discovery, staged copy, SHA-256 verification, and unmount.
- `test_read_only_media_copy`: Ingesting from write-protected media.
- `test_duplicate_media_detection_and_reinsertion`: Duplicate content hash match and drive reinsertion without duplicate jobs.
- `test_full_disk_reserve_rejection`: Rejecting ingests that violate the 15% free reserve.
- `test_mid_copy_removal_simulation`: Drive unplug simulation during stream copy, cleanup of partial staging files, and transition to `Failed`.
- `test_malicious_symlink_and_os_metadata_rejection`: Excluding symlinks pointing outside USB, hidden files, and OS metadata folders.
- `test_filename_collision_isolation`: Staging files with duplicate names in separate job directories.
