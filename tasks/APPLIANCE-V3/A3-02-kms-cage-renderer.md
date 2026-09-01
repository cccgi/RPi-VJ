# A3-02 — Single-Output KMS and Cage Renderer

## Goal

Present the existing web show fullscreen on exactly one HDMI connector without
a desktop, cursor, login session, network, or wall-clock dependency.

## Scope

Create the renderer launcher and `rpi-vj-renderer.service`. Force only the
labelled primary micro-HDMI connector (port 0, closest to USB-C power) to
1920x1080@60. Remove any forced declaration for the second connector.

The launcher owns `XDG_RUNTIME_DIR`, starts Cage, and execs Chromium using a
dedicated profile and localhost show URL. Cage and all Chromium children remain
in one systemd cgroup. Use bounded local-server readiness checks. Set no pointer
theme, cursor process, desktop shell, or display manager.

The service must start from local files and DRM devices, not network targets.
All failures go to journald. Restart must kill the complete cgroup.

## Observable

With Ethernet, Wi-Fi, keyboard, mouse, and HDMI disconnected, cold boot. Within
45 seconds the service is active. Attach the TV to the labelled port and observe
the default visual with no cursor. The other HDMI port must remain blank.

