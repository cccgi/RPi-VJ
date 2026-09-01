#!/usr/bin/env bash
# Set the Sound Blaster Play! 3 capture chain for LINE level, not mic level.
# Its input is a mic jack: mic boost + high capture gain will clip a booth feed
# in the ADC, before Web Audio ever sees it. Software gain cannot undo that.
set -euo pipefail
CARD="$(arecord -l | sed -n 's/^card \([0-9]\+\).*Sound Blaster.*/\1/p' | head -1)"
[ -n "$CARD" ] || { echo "!! Sound Blaster not found in 'arecord -l'"; exit 1; }
echo "== Sound Blaster on card $CARD"
echo "== available capture controls:"
amixer -c "$CARD" scontrols || true
echo
for CTL in "Mic Boost" "Mic Boost (+20dB)" "Auto Gain Control"; do
  if amixer -c "$CARD" sget "$CTL" >/dev/null 2>&1; then
    amixer -c "$CARD" sset "$CTL" 0 >/dev/null 2>&1 || amixer -c "$CARD" sset "$CTL" off >/dev/null 2>&1 || true
    echo "   $CTL -> OFF/0"
  fi
done
# The Play! 3 capture control is a MIC preamp: 0-60 in 0.5dB steps, so 40%
# was +12dB of gain on a line-level booth feed. Target 0dB (or below) and let
# the X-Touch fader do the fine trim in software.
TARGET="${1:-0dB}"
for CTL in "Mic" "Capture" "Digital"; do
  if amixer -c "$CARD" sget "$CTL" >/dev/null 2>&1; then
    amixer -c "$CARD" sset "$CTL" "$TARGET" capture >/dev/null 2>&1 \
      || amixer -c "$CARD" sset "$CTL" "$TARGET" >/dev/null 2>&1 || true
    echo "   $CTL capture -> $TARGET"
  fi
done
echo
echo "== resulting state:"
amixer -c "$CARD" | sed -n '/Simple mixer/,$p' | head -40
sudo alsactl store 2>/dev/null && echo "== saved (alsactl store) — survives reboot" || \
  echo "!! could not persist; run: sudo alsactl store"
