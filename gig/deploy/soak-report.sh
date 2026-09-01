#!/usr/bin/env bash
# One command that answers "did it survive the night, and if not, why?"
# Run ON THE PI after leaving the kiosk idle for several hours.
echo "===== SOAK REPORT  $(date) ====="
echo
echo "-- kiosk uptime (a recent start = it restarted; watchdog or crash)"
systemctl --user show vj-kiosk.service -p ActiveEnterTimestamp --value
systemctl --user show vj-server.service -p ActiveEnterTimestamp --value
echo
echo "-- watchdog health (must be healthy:true, or nothing is guarding the gig)"
curl -s http://127.0.0.1:8080/system/watchdog-status; echo
echo
echo "-- watchdog restarts (each line = the kiosk stopped heartbeating)"
if [ -f ~/vj-results/watchdog.log ]; then
  echo "   total: $(grep -c 'WATCHDOG RESTART' ~/vj-results/watchdog.log 2>/dev/null || echo 0)"
  tail -6 ~/vj-results/watchdog.log
else
  echo "   (no watchdog.log yet - no restart has ever fired)"
fi
echo
echo "-- Chromium crash dumps, newest last"
ls -la ~/.config/chromium/Crash*/pending/*.dmp 2>/dev/null | tail -8 || echo "   (none)"
echo "   count: $(ls ~/.config/chromium/Crash*/pending/*.dmp 2>/dev/null | wc -l)"
echo
echo "-- memory now (swap climbing over a long idle = a leak)"
free -m
echo
ps -eo rss,comm --sort=-rss | head -6
echo
echo "-- last engine lines (fps + audio; proves it is still drawing)"
grep -E 'engine fps|WEBGL CONTEXT' ~/vj-results/show.log 2>/dev/null | tail -4
echo
echo "-- AUDIO INPUT: must NOT say 'Internal Clock'. If it does, the show is"
echo "   running but deaf and every audio-reactive shader is dead."
grep -E 'audio OK|audio start|audio inputs visible|audio:' ~/vj-results/show.log 2>/dev/null | tail -6
if grep -q 'Internal Clock' ~/vj-results/show.log 2>/dev/null; then
  echo "   >> saw Internal Clock in the log at some point"
fi
echo
echo "-- is the Sound Blaster present to the OS at all?"
arecord -l 2>/dev/null | grep -i card || echo "   !! no capture devices listed"
echo
echo "===== INTERPRETING THIS ====="
echo "crash dumps grew, watchdog restarts matched  -> it crashed AND recovered. Good."
echo "crash dumps grew, NO watchdog restart        -> it crashed and sat there. Bad."
echo "swap climbing, RSS climbing, no crash yet    -> a leak; it will crash later."
echo "nothing changed at all                       -> the hardening held."
