#!/usr/bin/env python3
"""Fast structural regression checks for the kiosk reliability fixes."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
index = (ROOT / "index.html").read_text(encoding="utf-8")
show_js = (ROOT / "src" / "show.js").read_text(encoding="utf-8")
deck_js = (ROOT / "src" / "deck.js").read_text(encoding="utf-8")
mixer_js = (ROOT / "src" / "mixer.js").read_text(encoding="utf-8")
unit = (ROOT / "deploy" / "vj-kiosk.service").read_text(encoding="utf-8")
sync_ps1 = (ROOT / "tools" / "sync.ps1").read_text(encoding="utf-8")
show = json.loads((ROOT / "show.json").read_text(encoding="utf-8"))

assert show["streamDeckThumbnails"] is False
assert "parsedData.streamDeckThumbnails !== true" in show_js
assert "body:not(.show-cursor) *" in index
assert index.index("await loadInitialShow();") < index.index("await initStreamDeck();")
assert "onReconnect?." in index and "scheduleDeckRepaint" in index
assert "texSubImage2D" in deck_js and "_loadGeneration" in deck_js
assert "deckUV.x = 1.0 - abs(deckUV.x * 2.0 - 1.0)" in mixer_js
assert "deckUV.y = abs(deckUV.y * 2.0 - 1.0)" in mixer_js

# Page 6 is a full 28-pad quarter-frame clip bank.  Keys 24/25 remain page
# navigation and 30/31 remain RESET/BLACK; the former FX amount keys 26-29
# must stay available to the bank.
quad_bank = show["banks"][5]
quad_pads = quad_bank["pads"]
quad_keys = set(range(24)) | {26, 27, 28, 29}
assert quad_bank["name"] == "QUAD MIRROR"
assert len(quad_pads) == 28
assert {pad["key"] for pad in quad_pads} == quad_keys
assert all(pad.get("layout") == "mirror_hv_tl" for pad in quad_pads)
assert all(pad["src"].startswith("media/quad/") for pad in quad_pads)
assert "{}, // page 6 — 28 quad-mirror clips" in index
assert [bank["name"] for bank in show["banks"][5:9]] == [
    "QUAD MIRROR", "50TH", "720p", "NEON"
]
assert "KillMode=control-group" in unit and "TimeoutStopSec=5" in unit
assert '"-4"' in sync_ps1 and '"BatchMode=yes"' in sync_ps1
assert '"ConnectTimeout=10"' in sync_ps1 and "scp -q" not in sync_ps1
assert "ssh @SshOptions" in sync_ps1 and "scp @SshOptions" in sync_ps1
assert "$Target -replace '\\\\@', '@'" in sync_ps1
assert "restart --no-block vj-server vj-kiosk" in sync_ps1
assert "restart readiness timed out" in sync_ps1
print("PASS: kiosk reliability policy and bounded, visible IPv4 deployment transport")
