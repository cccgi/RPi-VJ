# A3-03 — Offline Boot Hardware Gate

## Goal

Falsify the claim that A3-02 is a reliable headless boot path.

## Scope

Create `scripts/appliance/test-cold-boots.sh` to append boot IDs, monotonic
first-service/first-frame times, DRM connector state, renderer restarts, and
result to a persistent CSV. The script must not declare success itself; a human
records whether the TV showed the expected visual and whether a cursor appeared.

Run 25 true power-removal cold boots across:

- TV connected before boot;
- TV attached 2 minutes after boot;
- no LAN/Wi-Fi throughout;
- Stream Deck present and absent;
- one HDMI disconnect/reconnect after successful presentation.

## Observable

25/25 boots show the default visual within 45 seconds after the TV becomes
available, zero cursors appear, no boot selects the second connector, and all
25 preceding-boot records remain readable. Any miss returns A3-02 to WIP.

