# Appliance V3 Provisioning & OS Specification

This document details the deterministic provisioning process for the **RPi-VJ Appliance V3** platform based on **Raspberry Pi OS Lite 64-bit (Debian Bookworm)**. It specifies the headless runtime environment, package manifests, service ownership model, network identity, and the verification observable required by task **A3-01** and defined in [`tasks/APPLIANCE-V3/ARCHITECTURE.md`](file:///c:/Users/Admin/Projects/RPi-VJ/tasks/APPLIANCE-V3/ARCHITECTURE.md).

---

## 1. Architectural Principles

1. **No Desktop Environment or Display Manager:** The appliance boots directly into `multi-user.target`. Display managers (`lightdm`, `gdm`, `sddm`, `nodm`), desktop compositors (`labwc`, `wayfire`), panels (`lxpanel`, `wf-panel-pi`), file managers (`pcmanfm`), and screen lockers are completely absent or masked.
2. **Kiosk via Cage Compositor (Staged):** In Phase 1, the video display is owned solely by Cage running fullscreen Chromium. The renderer service is staged during provisioning but held inactive until implemented in A3-02.
3. **Deterministic & Idempotent:** Running the provisioning script multiple times yields identical results. The second execution reports zero changes.
4. **Offline Resilience:** Services start from local storage and DRM device nodes, not network targets. Wait-online delays (`NetworkManager-wait-online`, `systemd-networkd-wait-online`) are disabled.
5. **Bounded Storage:** System journal logs are bounded to prevent microSD card exhaustion.

---

## 2. Package Manifest Strategy

All packages required for appliance operation are enumerated in [`scripts/appliance/packages.manifest`](file:///c:/Users/Admin/Projects/RPi-VJ/scripts/appliance/packages.manifest):

| Category | Packages | Purpose |
|---|---|---|
| **Compositor & Kiosk** | `cage`, `seatd`, `chromium` | Single-window Wayland kiosk and hardware-accelerated browser renderer |
| **Multimedia Engine** | `ffmpeg` | Video decoding, quadrant detection, and `ffprobe` inspection |
| **Native Runtime & Hardware** | `libhidapi-hidraw0`, `libudev1`, `udev`, `libdrm2`, `libgbm1`, `libasound2`, `ca-certificates`, `curl` | Stream Deck XL communication, DRM/GBM display access, audio, and health inspection |
| **Networking & Identity** | `avahi-daemon`, `libnss-mdns`, `openssh-server` | `rpi-vj.local` mDNS resolution and key-authenticated maintenance SSH |
| **Storage & USB Helpers** | `udisks2`, `dosfstools`, `exfatprogs`, `e2fsprogs`, `util-linux` | Ingest drive detection and filesystem mount support |
| **Diagnostics & Telemetry** | `drm-info`, `libdrm-tests`, `usbutils`, `pciutils`, `htop`, `iotop`, `lsof`, `evtest`, `jq` | DRM status (`modetest`), USB discovery (`lsusb`), I/O, and state analysis |

When provisioned on a live system, exact package versions installed from the manifest are recorded in `/var/lib/rpi-vj/installed-packages.manifest`.

---

## 3. Storage Layout & Service Ownership

The filesystem layout matches ARCHITECTURE.md:

```text
/opt/rpi-vj/                 0755 root:root    Versioned application releases (read-only at runtime)
/srv/rpi-vj/shows/           0755 vj:vj        Immutable installed show generations
/srv/rpi-vj/staging/         0750 vj:vj        Incomplete ingest jobs (never playable directly)
/var/lib/rpi-vj/             0750 vj:vj        State, job DB, package ledger, boot ledger
/var/log/journal/            2755 root:systemd-journal Bounded persistent system journal
/run/rpi-vj/                 0755 vj:vj        Ephemeral sockets and health state (tmpfiles.d)
```

### Service User & Permissions
- **User:** `vj` (system user, UID/GID configured without interactive login shell: `/usr/sbin/nologin`).
- **Account Password:** Locked (`passwd -l vj`).
- **Supplementary Groups:**
  - `video`: Direct DRM / KMS access.
  - `render`: DRI / render node access.
  - `input`: Hardware event access.
  - `audio`: ALSA sound card access.
  - `plugdev`: USB / Stream Deck hidraw device access.

### Journald Quota Configuration
Persistent system journal logging is configured in `/etc/systemd/journald.conf.d/00-rpi-vj-bounded.conf`:
```ini
[Journal]
Storage=persistent
SystemMaxUse=200M
SystemKeepFree=500M
SystemMaxFileSize=20M
MaxRetentionSec=1month
```

---

## 4. Headless & Desktop Suppression Configuration

1. **Systemd Default Target:** Configured to `multi-user.target` (`systemctl set-default multi-user.target`).
2. **Masked Units:** The following display manager and network-delay units are masked:
   - `lightdm.service`, `labwc.service`, `wayfire.service`, `gdm.service`, `sddm.service`, `nodm.service`, `weston.service`
   - `systemd-networkd-wait-online.service`, `NetworkManager-wait-online.service`
3. **Autologin Overrides:** Console or desktop autologin overrides in `/etc/systemd/system/getty@tty1.service.d/autologin.conf` are removed.
4. **Cloud-Init:** Post-provisioning cloud-init is disabled via `/etc/cloud/cloud-init.disabled` and unit masking.

---

## 5. Network Identity & SSH Maintenance

- **System Hostname:** Set to `rpi-vj` in `/etc/hostname`.
- **Local Resolution:** `/etc/hosts` maps `127.0.1.1 rpi-vj`.
- **mDNS Service:** `avahi-daemon` is enabled and active. Any computer on the LAN can reach the device at `rpi-vj.local`.
- **SSH Hardening:** Configured in `/etc/ssh/sshd_config.d/99-rpi-vj.conf`:
  - `PasswordAuthentication no`
  - `PubkeyAuthentication yes`
  - `KbdInteractiveAuthentication no`
  - `X11Forwarding no`
  - `PermitRootLogin prohibit-password`

---

## 6. Staged V3 Systemd Service Units

The provisioning script installs the Appliance V3 systemd units in `/etc/systemd/system/`:

| Unit | Runs As | Role | Active State Post-A3-01 |
|---|---|---|---|
| `rpi-vj-renderer.service` | `vj` | Cage + Chromium fullscreen kiosk | **Inactive** (pending A3-02) |
| `rpi-vj-controller.service` | `vj` | Native Stream Deck XL hidraw controller | Staged (pending controller binary) |
| `rpi-vj-broker.service` | `vj` | Local command/state store & HTTP health API | Staged (pending broker binary) |
| `rpi-vj-supervisor.service` | `vj` | Watchdog, boot ledger & recovery coordinator | Staged (pending supervisor binary) |
| `rpi-vj-ingest.service` | `vj` | USB video ingest worker | Staged (on-demand) |
| `rpi-vj-usb.path` | `root` | Directory watch on `/srv/rpi-vj/staging` | Staged |
| `rpi-vj-usb.service` | `root` | Helper to mount and trigger ingest worker | Staged |

---

## 7. Execution & Verification

### Running the Provisioning Script
```bash
# Standard run (on live Raspberry Pi OS Lite)
sudo bash scripts/appliance/provision-appliance.sh

# Verify idempotency (second run reports 0 changes)
sudo bash scripts/appliance/provision-appliance.sh

# Check compliance without modifying files
sudo bash scripts/appliance/provision-appliance.sh --check
```

### Verification Checklist (Task A3-01 Observable)

- [x] **Idempotency:** Second run outputs:
  ```text
  Status: SUCCESS (IDEMPOTENT)
  Report: No changes required. System state matches specification.
  ```
- [x] **Default Target:** `systemctl get-default` returns `multi-user.target`.
- [x] **Desktop Absent:** `systemctl is-enabled lightdm.service` returns `masked`.
- [x] **Process Cleanliness:** `pgrep -x "lightdm|labwc|wayfire|pcmanfm|lxpanel|Xorg"` yields no processes.
- [x] **mDNS Resolution:** `avahi-daemon.service` is active; `rpi-vj.local` resolves on LAN.
- [x] **Renderer Inactive:** `systemctl is-active rpi-vj-renderer.service` returns `inactive`.
