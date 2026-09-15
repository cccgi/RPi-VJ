# Appliance V3 Board

| ID | Task | Depends | Suggested model | Status |
|---|---|---|---|---|
| A3-00 | Freeze baseline and recovery image | — | cheap | DONE |
| A3-01 | Lite OS and deterministic provisioning | A3-00 | mid | DONE |
| A3-02 | Single-output KMS and Cage launcher | A3-01 | mid | DONE |
| A3-04 | Persistent logs and boot ledger | A3-01 | cheap | DONE |
| A3-03 | Offline boot hardware gate | A3-02,A3-04 | reviewer/human | TODO |
| A3-05 | Broker protocol and monotonic supervisor | A3-04 | mid | TODO |
| A3-06 | Native Stream Deck transport | A3-01 | mid | DONE |
| A3-07 | Stream Deck state UI and command integration | A3-05,A3-06 | mid | TODO |
| A3-08 | Immutable show storage and job model | A3-05 | mid | TODO |
| A3-09 | USB discovery and transactional copy | A3-08 | cheap | TODO |
| A3-10 | Media analysis and appliance transcode | A3-08 | mid | TODO |
| A3-11 | Stream Deck-only ingest workflow | A3-07,A3-09,A3-10 | mid | TODO |
| A3-12 | LAN ingest portal and stable discovery | A3-08 | mid | TODO |
| A3-13 | Storage, power-loss, and security hardening | A3-04,A3-08 | mid | TODO |
| A3-14 | Release qualification and cutover | all | reviewer/human | TODO |

Only the ground-truth runner may set `DONE` for a hardware-gated task.
