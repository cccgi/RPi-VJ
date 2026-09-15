# Appliance V3 Renderer & Single-Output KMS Specification

This document details the single-output DRM/KMS and Cage kiosk renderer architecture for the **RPi-VJ Appliance V3** platform on **Raspberry Pi OS Lite 64-bit (Debian Bookworm)**. It specifies the hardware video topology, kernel display forcing, Wayland kiosk lifecycle, browser containment, and verification observable required by task **A3-02** and defined in [`tasks/APPLIANCE-V3/ARCHITECTURE.md`](file:///c:/Users/Admin/Projects/RPi-VJ/tasks/APPLIANCE-V3/ARCHITECTURE.md).

---

## 1. Architectural Principles & Operational Boundary

1. **No Desktop Environment or Display Manager:** The appliance boots directly into `multi-user.target`. No display manager (`lightdm`, `gdm`, `sddm`), desktop shell (`labwc`, `wayfire`), panel (`lxpanel`, `wf-panel-pi`), or session manager runs.
2. **Kiosk via Cage Compositor:** In Phase 1, the video pipeline is owned by **Cage**, a minimal Wayland kiosk compositor designed to run a single fullscreen client application.
3. **Hardware-Accelerated Browser Show:** The web show is presented by Chromium executing directly against Wayland via Ozone (`--ozone-platform=wayland`), bypassing X11/Xwayland overhead.
4. **Single Forced Output:** Only the primary micro-HDMI connector (port 0, adjacent to the USB-C power socket) is forced to **1920x1080@60Hz**. The secondary connector (port 1) has no forced declaration and remains blank.
5. **No Cursor or Pointer UI:** No pointer theme, cursor process (e.g. `unclutter`), or desktop pointer UI is instantiated. With mouse and keyboard disconnected, the display presents visuals without a mouse cursor.
6. **Offline & Non-Network Resilience:** The service starts strictly from local storage (`local-fs.target`) and DRM hardware devices (`systemd-udevd.service`). It has no dependency on network connectivity, IP address assignment, or wall-clock synchronization.
7. **Complete Cgroup Containment:** Cage and all Chromium child processes (GPU, zygote, renderer) remain in one systemd cgroup. Stopping or restarting the service terminates the entire cgroup.

---

## 2. KMS & Single-Output Display Contract

### Physical Connector Topology
Raspberry Pi 4 and Raspberry Pi 5 provide dual micro-HDMI ports:
- **Port 0 (Primary):** Physical port closest to the USB-C power connector. In Linux kernel DRM/KMS (`vc4-kms-v3d`), this connector is identified as `HDMI-A-1`.
- **Port 1 (Secondary):** Physical port furthest from the USB-C power connector. Identified as `HDMI-A-2`.

### Kernel Command-Line Forcing (`cmdline.txt`)
To satisfy the cold-boot observable where the appliance boots with HDMI disconnected and the TV is attached post-boot, the primary connector must be forced active at boot time:

```text
video=HDMI-A-1:1920x1080@60D
```

- **`HDMI-A-1:1920x1080@60`:** Enforces exact 1080p resolution at 60 Hz, preventing 4K TV EDID modes from exhausting GPU and RAM buffers on 2 GB appliances.
- **`D` (Digital Force):** Forces the digital output to active/connected state even when no cable or sink EDID is detected at cold boot.
- **`HDMI-A-2` Policy:** All forced declarations for the second connector (`video=HDMI-A-2:...`) are explicitly removed. Port 1 is not declared and remains blank.

The launcher script provides automated idempotency helpers to enforce this in `/boot/firmware/cmdline.txt`:
```bash
# Verify compliance
bash scripts/appliance/launch-renderer.sh --check-kms

# Enforce KMS cmdline configuration (requires root)
sudo bash scripts/appliance/launch-renderer.sh --setup-kms
```

---

## 3. Renderer Launcher (`launch-renderer.sh`)

The launcher (`scripts/appliance/launch-renderer.sh`) manages the two-stage initialization lifecycle:

```text
Systemd (rpi-vj-renderer.service)
  │
  ▼
[Phase 1: Host Environment (Outside Wayland)]
  ├── Verify & own XDG_RUNTIME_DIR (/run/rpi-vj/renderer, 0700)
  ├── Suppress cursor themes (XCURSOR_THEME="", XCURSOR_SIZE=0)
  ├── Bounded DRM readiness check (/dev/dri/card*)
  ├── Bounded local-server readiness check (monotonic bash $SECONDS)
  ├── Sanitize Chromium profile (clean exit, clear singleton locks)
  └── exec cage -d -s -- /opt/rpi-vj/scripts/appliance/launch-renderer.sh --exec-browser
        │
        ▼
      [Phase 2: Inside Wayland Session (Cage Child)]
        ├── Apply wlr-randr output configuration (HDMI-A-1 1080p60, HDMI-A-2 off)
        └── exec /usr/lib/chromium/chromium [flags] http://localhost:8080/gig/
```

### XDG_RUNTIME_DIR Ownership
Wayland compositors require a valid, secure runtime directory (`mode 0700`, owned by current UID). System services running without PAM logind do not receive `/run/user/<uid>` automatically. The launcher and systemd unit explicitly establish `/run/rpi-vj/renderer` with mode `0700` owned by `vj:vj`.

### Dedicated Profile & Crash Bubble Sanitization
Power cuts mid-performance must not leave unclickable "Restore pages?" recovery bubbles on the TV display:
1. A dedicated profile path is assigned: `/var/lib/rpi-vj/renderer-profile`.
2. Prior to launching Chromium, `Preferences` is parsed and updated so `exit_type` is `"Normal"` and `exited_cleanly` is `true`.
3. Stale singleton lock files (`SingletonLock`, `SingletonSocket`, `SingletonCookie`) left by abrupt shutdowns are removed to prevent launch deadlock.

### Bounded Monotonic Readiness Checks
- **DRM Node:** Waits up to 5 seconds (`RENDERER_DRM_WAIT_SECS`) for `/dev/dri/card0` or `/dev/dri/card1` to be available.
- **Local Server:** Polls the local show URL (default `http://localhost:8080/gig/`) up to 10 seconds (`RENDERER_SERVER_WAIT_SECS`). If the server takes longer to respond, the check terminates and Chromium is launched anyway, avoiding any indefinite delay that would violate the 45-second service activation window.
- **Monotonic Clock:** All timeout loops use bash's internal `$SECONDS` counter, ensuring complete independence from NTP, RTC, and wall-clock shifts.

### Chromium Flag Profile
Chromium is invoked with minimal overhead tuned for embedded 2 GB Pi hardware:
- `--ozone-platform=wayland --enable-features=UseOzonePlatform`: Native Wayland rendering.
- `--kiosk --app=http://localhost:8080/gig/`: Fullscreen single application window without browser chrome or tabs.
- `--user-data-dir=/var/lib/rpi-vj/renderer-profile`: Isolated profile directory.
- `--password-store=basic --use-mock-keychain`: Prevents secret service prompts.
- `--autoplay-policy=no-user-gesture-required`: Autoplays visual streams without user input.
- `--enable-gpu-rasterization --ignore-gpu-blocklist`: V3D hardware acceleration.
- `--renderer-process-limit=1 --process-per-site --js-flags=--max-old-space-size=256`: Strict memory containment.
- `--disable-session-crashed-bubble --hide-crash-restore-bubble`: Prevents crash UI overlays.

---

## 4. Systemd Service Unit (`rpi-vj-renderer.service`)

Installed at `/etc/systemd/system/rpi-vj-renderer.service`:

```ini
[Unit]
Description=RPi-VJ KMS and Cage Renderer Service
Documentation=file:///opt/rpi-vj/docs/appliance/RENDERER.md file:///tasks/APPLIANCE-V3/ARCHITECTURE.md

# Start strictly from local files and DRM devices, never network targets
DefaultDependencies=no
After=systemd-udevd.service local-fs.target systemd-tmpfiles-setup.service
Requires=local-fs.target
Wants=systemd-udevd.service

# Prohibit concurrent display managers or desktop sessions
Conflicts=display-manager.service lightdm.service gdm.service sddm.service nodm.service

[Service]
Type=simple
User=vj
Group=vj
SupplementaryGroups=video render input audio

# Storage layout & runtime directory ownership
WorkingDirectory=/var/lib/rpi-vj
RuntimeDirectory=rpi-vj/renderer
RuntimeDirectoryMode=0700
Environment=XDG_RUNTIME_DIR=/run/rpi-vj/renderer
Environment=HOME=/var/lib/rpi-vj
Environment=WLR_LIBINPUT_NO_DEVICES=1

# Clean environment: suppress pointer themes and desktop sessions
UnsetEnvironment=XCURSOR_THEME XCURSOR_SIZE DESKTOP_SESSION GDMSESSION XDG_CURRENT_DESKTOP

# Primary launcher: owns XDG_RUNTIME_DIR, starts Cage, and execs Chromium
ExecStart=/opt/rpi-vj/scripts/appliance/launch-renderer.sh

# Restart policy and cgroup containment:
# Cage and all Chromium children remain in one systemd cgroup.
# Restart must kill the complete cgroup.
Restart=always
RestartSec=2s
KillMode=control-group
SendSIGKILL=yes
TimeoutStopSec=5s
TimeoutStartSec=30s

# All failures go to journald
StandardOutput=journal
StandardError=journal

# Resource limits & containment
LimitNOFILE=65536
LimitMEMLOCK=infinity
MemoryMax=1600M

[Install]
WantedBy=multi-user.target
```

---

## 5. Verification & Observable Validation (A3-02)

### Required Hardware Observable Procedure
1. **Prepare Hardware:** Disconnect Ethernet cable, Wi-Fi networks (or disable in rfkill), keyboard, mouse, and HDMI cables from both ports.
2. **Cold Boot:** Connect USB-C power supply to cold boot the appliance.
3. **Timing Gate:** Within **45 seconds** from power application, `rpi-vj-renderer.service` enters `active (running)`.
4. **Attach Primary Display:** Connect HDMI cable from TV to **port 0** (labelled micro-HDMI port closest to USB-C power).
   - **Expected:** TV syncs immediately to 1080p60 and presents the default visual web show.
   - **Expected:** No mouse cursor is visible anywhere on screen.
   - **Expected:** No session restore dialogs, warning bars, or desktop panels appear.
5. **Inspect Secondary Port:** Disconnect cable from port 0 and connect TV to **port 1** (micro-HDMI port furthest from power).
   - **Expected:** Port 1 remains completely blank (no signal / unforced).

### Safe Local & Automated Verification
The implementation includes self-test and simulation modes that execute locally without requiring physical display hardware:

```bash
# 1. Run unit test suite (validates XDG dir, cursor unset, cmdline parsing, profile sanitization, monotonic timeout)
bash scripts/appliance/launch-renderer.sh --test

# 2. Run non-destructive dry run
RENDERER_DRM_WAIT_SECS=1 RENDERER_SERVER_WAIT_SECS=1 bash scripts/appliance/launch-renderer.sh --dry-run

# 3. Check system prerequisites and KMS status
bash scripts/appliance/launch-renderer.sh --check
```
