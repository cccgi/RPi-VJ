# A3-06 — Native Stream Deck XL Transport

## Goal

Operate the Stream Deck without Chromium, WebHID, pairing, or a browser profile.

## Scope

Complete or replace `vj/crates/vj-surface-hid` as needed and package it in
`rpi-vj-controller.service`. Support the production device's measured VID/PID,
input report layout, key ordering, JPEG framing, brightness, and reconnect.

Requirements:

- hidraw/hidapi access through a narrow udev rule and `vj` group;
- one press and one release edge per physical action;
- bounded input and output queues;
- icon writes coalesced by key and timed out;
- input continues while icons upload;
- reconnect replays current cached page;
- raw-report diagnostic mode is bounded and off by default;
- controller state is published to the broker and boot ledger.

## Observable

Run without Chromium. All 32 keys produce the correct logical index exactly
once, icons match their keys, ten unplug/replug cycles restore the page within
five seconds, and an intentionally stalled icon write does not block presses.

