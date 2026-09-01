# G-17 — Dress Rehearsal & Failure Playbook
**Model:** mid · **Friday. This is the task that decides whether Saturday works.**

## Deliverable
`gig/RUNBOOK.md` — written to be read **on paper, at 1am, by a tired person
under time pressure**. Short sentences. No prose. Print it and tape it inside
the case.

## Part A — full dress rehearsal (do this before writing the runbook)
Real conditions, not a desk test: Pi, 100" TV, the actual HDMI cable and its
actual length, Sound Blaster, FLX10, Stream Deck, real music, lights down.
Run for **90 continuous minutes** — the length of the set, not five minutes.

Record every 10 minutes: fps, settled render scale, `vcgencmd measure_temp`,
`vcgencmd get_throttled`, and free memory. Thermal throttling and memory creep
only appear over time, which is exactly why a five-minute test proves nothing.

## Part B — deliberately break things and write down what happens
Do each of these *during* the rehearsal and time the recovery:
1. Unplug the Stream Deck, replug it
2. Unplug the Sound Blaster, replug it
3. Unplug HDMI, replug it
4. Trigger a shader that fails to compile
5. Trigger a missing clip file
6. Kill Chromium (`pkill chromium`) — does systemd bring it back, and how fast?
7. Pull the Pi's power mid-set and cold boot — **how long to visuals?**

Each one gets a runbook entry: symptom, cause, fix, and how long the fix takes.
Item 7's number is the one that matters most — it's your worst case, and knowing
it is 45 seconds versus 4 minutes changes what you do when it happens.

## Part C — the runbook itself
- **Pre-flight checklist** — plug order, what to verify before the first track
- **The one-page failure table** from Part B
- **Blackout is key 31.** State it at the top. It always works.
- **Known-good fallback:** if the engine is unrecoverable, what do you switch
  to? Decide this Friday, not Saturday. A single reliable clip on loop from
  a phone or laptop is a completely acceptable answer, and having decided it in
  advance is the difference between a ten-second gap and a visible panic.
- **Physical kit list:** spare USB cable, spare micro-HDMI adapter, spare SD
  card with a known-good image, the active cooler, a powered USB hub. The
  micro-HDMI adapter is the single most-forgotten item in Pi VJ rigs.

## Observable
`RUNBOOK.md` exists, all 7 failure modes have measured recovery times, the
90-minute run completed without a crash or sustained throttling, and the
printed copy is in the case. Summarise the 90-minute telemetry in
`../RESULTS.md`.

## If the rehearsal fails
Cut features per `BOARD.md`'s cut list until it passes. **Do not spend Friday
night fixing.** A simpler show that runs for 90 minutes beats a richer one that
dies at minute 40.
