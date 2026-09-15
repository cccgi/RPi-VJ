# Appliance Storage Architecture & Ingest Job Model

## 1. Directory Structure

The appliance manages show data using an immutable generation model designed to resist sudden power loss and partial copying.

```text
/srv/rpi-vj/
  ├── current -> shows/gen-20260915120000-abcd1234/  (atomic symlink)
  ├── shows/
  │   ├── gen-20260915120000-abcd1234/               (active generation)
  │   └── gen-20260915100000-efgh5678/               (previous generation for rollback)
  └── staging/
      ├── job-<uuid>/                                (work in progress)
      └── quarantine-<timestamp>-<uuid>/             (quarantined partial outputs on boot)

/var/lib/rpi-vj/
  └── ingest-jobs.jsonl                              (durable, fsynced append-only job ledger)
```

## 2. Ingest Job Lifecycle

Ingest workflows execute through explicit, durable state transitions:

```text
[Discovered] -> [Copying] -> [Probing] -> [NeedsDecision] -> [Transcoding] -> [Validating] -> [Ready] -> [Activating] -> [Complete]
      │              │           │               │                 │               │            │             │
      └──────────────┴───────────┴───────────────┴─────────────────┴───────────────┴────────────┴─────────────┴──> [Failed / Cancelled]
```

### Invariants:
1. **15% Free Space Reserve Enforcement**: Before any ingest job starts, the storage engine verifies that `free_bytes - required_bytes >= 0.15 * total_bytes`. Jobs are rejected upfront if capacity is insufficient.
2. **Durable Ledger (`fsync`)**: Every state transition is written to `/var/lib/rpi-vj/ingest-jobs.jsonl` and flushed with `fsync` before processing continues.
3. **Atomic Activation**: Show promotion constructs a temporary symlink `/srv/rpi-vj/.current_tmp_<uuid>` and atomically renames it over `/srv/rpi-vj/current`.
4. **Boot Recovery & Quarantine**:
   - On boot, the engine replays the job ledger.
   - Any unfinalized job is quarantined into `/srv/rpi-vj/staging/quarantine-<timestamp>-<uuid>`.
   - The job is marked `Failed { reason: "Interrupted by power loss or reboot..." }`.
   - The active show generation remains untouched and immediately playable.
5. **Rollback Guarantee**: The storage manager preserves the previous generation alongside the active generation. If a newly activated show is defective, calling `rollback()` atomically reverts `/srv/rpi-vj/current` to the previous generation.
