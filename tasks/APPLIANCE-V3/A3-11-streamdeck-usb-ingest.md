# A3-11 — Stream Deck-Only USB Ingest Workflow

## Goal

Complete media import, analysis decisions, transcoding, show placement, and
activation without another computer or display.

## Scope

Map the INGEST state to Stream Deck pages that show drive found, file count,
current filename abbreviation, progress, space estimate, analysis recommendation,
and error code. Provide actions: accept recommendation, keep full frame, crop
quadrant, repair seam, skip, cancel job, activate generation, and eject.

Long operations must return input control immediately and update progress at a
bounded rate. Destructive/costly choices require confirmation. PERFORMANCE mode
pauses ingest before running a new ffmpeg segment; it never interrupts playback.
Power loss resumes from the durable job model.

## Observable

Starting from a powered appliance with only TV and Stream Deck, insert a drive
with five mixed clips. Complete decisions, transcode, activate the new bank,
trigger every imported clip, safely eject, reboot, and verify the bank persists.

