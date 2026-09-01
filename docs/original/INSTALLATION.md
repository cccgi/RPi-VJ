# Original Chromium kiosk installation

This procedure documents the historical stack. It can reproduce the software, but it does not convert it into an appliance-grade system. Read the [known reliability findings](../../gig/APPLIANCE_RELIABILITY.md) before relying on it live.

## Requirements

### Target hardware

- Raspberry Pi 5 with active cooling and a stable 5 A USB-C supply.
- 64-bit Raspberry Pi OS **with Desktop**; the original kiosk expects a graphical user session and Chromium.
- High-quality micro-HDMI cable connected to HDMI port 0 and a 1080p display.
- Elgato Stream Deck XL (32 keys). Product support in the driver targets Elgato vendor `0x0fd9` and known XL IDs.
- Adequate microSD capacity for the OS, repository, transcoded clips, logs, and free working space.
- Optional audio reaction: Creative Sound Blaster Play! 3 or another browser-visible USB capture device, plus the booth line signal.
- Optional secondary control: Behringer X-Touch Mini.

### Development/ingest computer

- Git, SSH, Python 3, and FFmpeg.
- PowerShell 5+ for the supplied Windows sync and media commands.
- Network reachability to the Pi by hostname or IP.

## Install on the Pi

Clone the historical branch and enter it:

```sh
git clone --branch original https://github.com/cccgi/RPi-VJ.git
cd RPi-VJ
```

Inspect scripts before running them; they install packages and system configuration. The most complete historical desktop entry point is:

```sh
chmod +x gig/deploy/*.sh gig/tools/*.sh
./gig/deploy/install-desktop.sh
```

The deployment directory also contains several generations of installers and repair scripts. Do not run every script in sequence. Files under `gig/deploy/_superseded/` are preserved history and must not be used. `install-units.sh` installs the current service definitions when updating an existing configured Pi.

Useful optional setup:

```sh
./gig/deploy/set-audio-input.sh
./gig/deploy/install-power-sudoers.sh
./gig/deploy/install-samba.sh
```

The power sudoers rule is required if the Stream Deck shutdown gesture should power off the Pi. Samba is optional and increases network attack surface; enable it only on trusted networks.

## One-time interactive browser grants

The original WebHID/Web Audio design cannot be provisioned completely headlessly. Attach a temporary mouse and follow:

```sh
./gig/deploy/setup-pair.sh
```

In Chromium, select the Stream Deck in the HID chooser and grant audio capture. Verify that the 32 keys receive labels. These grants live in `~/.config/chromium`; deleting or replacing that profile can silently remove them.

## Add media

Media is not included in Git. Copy prepared clips into `gig/media/`, update `gig/show.json`, and follow [MEDIA.md](MEDIA.md). Verify every referenced path exists on the Pi.

## Start and verify

```sh
systemctl --user daemon-reload
systemctl --user enable --now vj-server.service vj-kiosk.service
systemctl --user status vj-server.service vj-kiosk.service
curl --fail http://127.0.0.1:8080/gig/
```

A successful process start is not sufficient. Confirm all of these physically:

1. HDMI shows animated program output at the expected resolution.
2. Stream Deck keys show the page labels.
3. Pressing a clip key starts the matching clip.
4. A shader key produces motion.
5. BLACK toggles black and restores output.
6. RESET clears the show.
7. With music playing, diagnostic audio values change.
8. Reboot with no keyboard/mouse and repeat the checks.

Run the probe and soak tooling before a show:

```sh
./gig/deploy/appliance-doctor.sh
./gig/deploy/run-probe.sh
./gig/deploy/soak-report.sh
```

See [`gig/RUNBOOK.md`](../../gig/RUNBOOK.md) for operation and failure recovery. Some labels in that historical runbook predate the final nine-page layout; [STREAMDECK.md](STREAMDECK.md) is generated from the final `show.json` and runtime system-row definitions.

## Security and maintenance

- Use SSH keys, disable password SSH after recovery access is proven, and never commit keys or passwords.
- Keep a known-good SD-card image; Git does not capture OS packages, browser profiles, permissions, udev state, or user services.
- Gracefully power off whenever possible. Repeated hard power removal can corrupt the writable microSD filesystem.
- Pin and test OS/browser updates on a spare card. Chromium changes can alter GPU, media, permissions, or kiosk behavior.
