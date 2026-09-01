# Appliance V3 Architecture Contract

## Runtime boundary

```text
                 optional LAN                         USB thumb drive
 PC/Mac browser ─────────────► ingest API       /dev/sdX ─► ingest worker
                                      │                         │
                                      └──────────┬──────────────┘
                                                 ▼
                                      staged show generation
                                      validate -> fsync -> activate
                                                 │
 Stream Deck XL ◄──hidraw── controller ──commands──► broker/state store
       ▲                       │                         │
       └──── icons/status ─────┘                         ▼
                                                    renderer
                                             Cage -> Chromium (Phase 1)
                                                 │
                                       one forced HDMI connector
                                                 ▼
                                                TV

 supervisor observes: DRM + compositor + renderer + frames + controller + disk
 persistent boot ledger and journald survive reboot and forced power loss
```

## Non-negotiable decisions

1. Base OS is Raspberry Pi OS Lite 64-bit with package versions pinned per image.
2. There is no desktop environment or display manager.
3. Cage owns the sole program output in Phase 1. No pointer UI is launched and
   Chromium receives no input cursor from a desktop shell.
4. Only the labelled HDMI port is force-connected. The other port is not declared.
5. Runtime starts from local storage and DRM availability, not network targets.
6. All timeout and heartbeat decisions use a monotonic clock.
7. Stream Deck is opened by a native hidraw/hidapi service. Chromium never owns it.
8. Controller commands go through a versioned local protocol with IDs and acks.
9. Active media is immutable. Ingest builds a new generation and atomically swaps
   a `current` symlink only after complete validation and durable writes.
10. Heavy ingest work is lower priority than playback and pauses automatically
    while performance mode is armed.
11. Persistent logs are bounded; media cannot fill the root filesystem.
12. Network management is optional. The stable identity is `rpi-vj.local` plus a
    locally displayed/Stream-Deck-visible IP, not a hard-coded address.

## Service ownership

| Unit | Runs as | Owns | Restart policy |
|---|---|---|---|
| `rpi-vj-renderer.service` | `vj` | Cage, Chromium, DRM session | always |
| `rpi-vj-controller.service` | `vj` | Stream Deck HID and icons | always |
| `rpi-vj-broker.service` | `vj` | commands, state, health API | always |
| `rpi-vj-supervisor.service` | `vj` | recovery policy, boot ledger | always |
| `rpi-vj-ingest.service` | `vj` | queued probe/transcode/install jobs | on demand |
| `rpi-vj-usb.path/service` | root helper + `vj` worker | mount discovery only | event driven |

Privileged helpers have fixed arguments. The browser and network API cannot run
arbitrary shell commands.

## Storage layout

```text
/opt/rpi-vj/                 versioned application releases (read-only at runtime)
/srv/rpi-vj/shows/<id>/      immutable installed show generations
/srv/rpi-vj/current          atomic symlink to active generation
/srv/rpi-vj/staging/         incomplete ingest jobs; never playable
/var/lib/rpi-vj/             state, job DB, boot ledger
/var/log/journal/            bounded persistent system journal
/run/rpi-vj/                 sockets and ephemeral health state
```

Reserve at least 15% of the filesystem and enough space for the active show plus
one incoming generation. Reject ingest before transcoding when that invariant
cannot be maintained.

## Stream Deck operator states

The controller always presents one of these hardware-visible states:

- `BOOTING`: progress keys identify storage, renderer, and media readiness.
- `READY`: show banks and system page available.
- `INGEST`: job progress, warnings, accept/cancel, and final activation.
- `PERFORMANCE`: ingest suspended; only show controls and guarded shutdown active.
- `DEGRADED`: renderer/controller/storage fault with recovery action and code.

System operations require hold or two-key confirmation. Clip presses never wait
for icon upload; icon updates are asynchronous and bounded.

## USB ingest contract

The drive contains `/RPI-VJ-INBOX/`. Each supported video becomes a job:

1. Copy to staging while hashing; never process directly from removable media.
2. Probe with `ffprobe`; reject unreadable or unsafe input.
3. Analyze frame samples for quadrant duplication and loop seam quality.
4. Present proposed actions on Stream Deck: keep, crop quadrant, seam/crossfade,
   transcode, skip. Defaults must be conservative and reversible.
5. Produce the appliance rendition and 96x96 Stream Deck image.
6. Validate decode, duration, pixel format, resolution, frame rate, and checksum.
7. Create a new show generation, fsync it, then atomically activate it.
8. Write a receipt back to the USB drive when writable.

Detection is advisory: never destructively crop or alter a loop solely from a
heuristic score without an operator-confirmed policy.

## Revisit later

- Replace Cage/Chromium with the native Rust DRM renderer only after equivalent
  clip/shader behavior and the same hardware gates pass.
- Move media to SSD if measured microSD endurance or throughput is insufficient.
- Add a Wi-Fi access point mode only if mDNS + Ethernet/known Wi-Fi is inadequate.

