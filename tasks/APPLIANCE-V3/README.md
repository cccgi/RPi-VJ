# RPi-VJ Appliance V3 — Start Here

This program replaces the Raspberry Pi OS Desktop + LightDM + Labwc + Chromium
WebHID deployment with a deterministic, offline-first VJ appliance.

The operator has exactly one control surface (Stream Deck XL) and one program
output (the TV on the labelled primary micro-HDMI port). No keyboard, mouse, or
operator monitor is assumed at any point during a show.

## Definition of appliance-ready

The release is ready only after the production Pi and peripherals pass all of
these gates:

- 25 consecutive offline cold boots, each presenting a known visual in <=45 s.
- Cold boot succeeds with HDMI absent; attaching the TV later shows the program.
- Exactly one HDMI connector is forced; Chromium cannot land on a phantom output.
- No desktop, display manager, panel, file manager, or mouse cursor is installed.
- Stream Deck input works without Chromium, WebHID, pairing, or a browser profile.
- A Stream Deck press is accepted while the renderer is restarting and is logged.
- USB removal/reinsert restores the current Stream Deck page without intervention.
- USB media ingest is transactional and cannot damage the active show.
- Network, DHCP, DNS, NTP, SMB, and Internet absence cannot delay program output.
- Eight-hour mixed-media soak passes with injected HDMI, USB, and renderer faults.
- Logs from the preceding boot remain available after a forced power interruption.
- A known-good recovery image and rollback procedure have been exercised.

## Implementation strategy

Phase 1 keeps the current web renderer but replaces its unreliable environment:

1. Raspberry Pi OS Lite, one forced HDMI connector, minimal Cage compositor.
2. System services, persistent logs, monotonic supervisor.
3. Native Stream Deck service and command broker; WebHID is removed from control.
4. Transactional USB and LAN ingest.

Phase 2 may replace Chromium with the native Rust renderer only after Phase 1 is
measured. A renderer rewrite is not allowed to block appliance reliability.

## How agents work

1. Read this file, `BOARD.md`, and exactly one task spec.
2. Read no other task documents unless the spec explicitly permits it.
3. Change only the files listed in the spec.
4. Run the specified observable. Compilation alone is never completion.
5. Append at most ten lines to `RESULTS.md` and update one row in `BOARD.md`.

Status: `TODO -> WIP -> BLOCKED(reason) -> NEEDS-REVIEW -> DONE`.

`DONE` means Frank observed the hardware gate on the production Pi.

## Order

Execute tasks in board order unless their dependency column permits parallel work.
The hardware gates in Tasks 03, 06, 09, 12, and 14 are mandatory stop points.
Task IDs are stable; numeric order does not override the board's dependency order.
