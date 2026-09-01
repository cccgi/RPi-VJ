# Appliance V3 Recovery & Baseline Image Procedure

This document defines the standard operating procedure for creating a complete, bit-accurate backup of the Raspberry Pi appliance microSD card, verifying image integrity, restoring the image to a spare card, and proving that the restored media boots properly before performing destructive or appliance-level refactoring.

---

## 1. Safety & Golden Rules

- **Golden Rule:** Never perform destructive OS modifications or service overhauls without a verified, bootable physical backup.
- **Raw Image vs Filesystem Copy:** A filesystem copy (`cp`, `tar`, `rsync`) is **NOT** a recovery image. Filesystem copies fail to replicate partition table geometry, bootloader sectors, UUIDs/PARTUUIDs (required by `/etc/fstab` and `cmdline.txt`), and exact filesystem permissions/capabilities.
- **Production Media Protection:** Do not modify the production card during backup capture. Always unmount partitions or take the image offline from a separate workstation/host reader.
- **Secret Redaction:** Baseline config dumps created by `scripts/appliance/capture-baseline.sh` redact private keys, Wi-Fi PSKs, and Samba credentials. When taking disk images, the raw media contains full appliance state and must be stored securely.

---

## 2. Capturing the Baseline Snapshot (On Pi)

Before taking the Pi offline for raw imaging, capture the running software and hardware state:

```bash
# Run baseline capture script
bash scripts/appliance/capture-baseline.sh /var/tmp/baseline-$(date +%Y%m%d_%H%M%S)
```

This non-destructively records:
- OS release, kernel version, and installed Debian packages (`dpkg`).
- Bootloader configuration (`config.txt`, `cmdline.txt`, device tree).
- Active and enabled systemd units (`/etc/systemd/system/`).
- Block device layout, UUIDs, PARTUUIDs, and mount points.
- Udev rules (`/etc/udev/rules.d/`) and USB topology (`lsusb`).
- DRM display connectors and mode state (`/sys/class/drm/`).
- Sanitized network interface configurations.
- SHA-256 checksums of application binaries and config files.
- Recent system logs (`journalctl`, `dmesg`).

---

## 3. Creating the Raw Recovery Image (Offline on Workstation)

1. **Gracefully shut down the Pi:**
   ```bash
   sudo systemctl poweroff
   ```
2. **Remove the microSD card** from the Raspberry Pi and insert it into a USB card reader on a host workstation (Linux, macOS, or Windows).

3. **Identify the device node** (Linux host example):
   ```bash
   lsblk -o NAME,SIZE,TYPE,MOUNTPOINTS,MODEL
   # Identify the raw disk (e.g., /dev/sdb or /dev/mmcblk0), NOT a single partition like /dev/sdb1.
   ```
   > **WARNING:** Ensure all partitions on the card are unmounted before creating the image.
   ```bash
   sudo umount /dev/sdb* 2>/dev/null || true
   ```

4. **Capture the raw bit-accurate image using `dd`:**
   ```bash
   sudo dd if=/dev/sdb of=rpi-vj-baseline-$(date +%Y%m%d).img bs=4M status=progress conv=fsync
   ```
   *(Alternatively, use `ddrescue` or Raspberry Pi Imager's "Read Image" feature).*

5. **Generate the image SHA-256 checksum:**
   ```bash
   sha256sum rpi-vj-baseline-*.img > rpi-vj-baseline-$(date +%Y%m%d).img.sha256
   ```

6. **Verify the SHA-256 hash:**
   ```bash
   sha256sum -c rpi-vj-baseline-*.img.sha256
   ```

---

## 4. Restoring to a Spare MicroSD Card

To validate the rollback path, restore the captured image onto a **separate spare microSD card** of equal or greater capacity:

1. **Insert the spare microSD card** into the workstation card reader.
2. **Verify target device node** (e.g. `/dev/sdc`):
   ```bash
   lsblk
   sudo umount /dev/sdc* 2>/dev/null || true
   ```
3. **Write the image to the spare card:**
   ```bash
   sudo dd if=rpi-vj-baseline-*.img of=/dev/sdc bs=4M status=progress conv=fsync
   sync
   ```
4. **Eject the card safely:**
   ```bash
   sudo eject /dev/sdc
   ```

---

## 5. Proving Restored Media Boots (Verification Checklist)

Insert the restored spare microSD card into the Raspberry Pi hardware and execute the verification gate:

- [ ] **Power & Boot:** Connect HDMI display and power supply. Observe green ACT LED activity and HDMI video output (rainbow splash, console / DRM framebuffer).
- [ ] **System Startup:** Verify system boots cleanly into multi-user target without entering emergency mode or hanging on disk UUID/PARTUUID mount errors.
- [ ] **Service Health:** Check status of VJ services and pipewire:
  ```bash
  systemctl status rpi-vj --no-pager
  systemctl status pipewire wireplumber --no-pager
  ```
- [ ] **Hardware Detection:** Verify DRM display connector and USB controllers are present:
  ```bash
  lsusb
  cat /sys/class/drm/card*-*/status
  ```
- [ ] **Baseline Match:** Run `scripts/appliance/capture-baseline.sh /tmp/restored-baseline` and compare against the original pre-backup baseline.
- [ ] **Record Results:** Record the verified image SHA-256 hash and successful boot confirmation in `RESULTS.md`.

---

## 6. Emergency Rollback Procedure

If any subsequent task (A3-01 through A3-07) causes an unrecoverable state or boot failure:

1. Power off the Raspberry Pi.
2. Remove the modified card and set it aside.
3. Insert the verified spare microSD card (or re-flash a spare card using `rpi-vj-baseline-*.img`).
4. Power on the Pi and verify normal operation is restored.
