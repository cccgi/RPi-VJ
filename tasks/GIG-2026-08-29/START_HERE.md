# Start here — worker onboarding (read this, ~1 min, then your spec only)

You are implementing **one** task for a live gig on Sat 29 Aug 2026.

1. Open `BOARD.md`, find your task ID, set it to `WIP`.
2. Open `specs/G-XX-*.md`. **That spec is self-contained — it is the only
   document you need.** Do not read the briefings, the tracker, or the Rust
   workspace unless your spec names a specific file.
3. Write the code. One task, the files the spec names, nothing else.
4. Run the spec's **Observable** and see it with your own eyes. On the Pi if
   the spec says Pi.
5. Append ≤10 lines to `RESULTS.md`. Set the board line to `NEEDS-REVIEW`
   (or `DONE` if you ran the observable on the Pi yourself).

## Rules that matter
- **`DONE` means observed, not compiled.** This project already has 62 passing
  tests for code that has never touched hardware. Don't add to that pile.
- If the spec is wrong or the hardware disagrees with it, **say so in
  `RESULTS.md` under `surprise:` and stop.** A wrong spec found on Tuesday is
  cheap. Found on Saturday it's the gig.
- Don't refactor anything outside your task. Don't improve neighbouring code.
- Don't write summary documents. `BOARD.md` and `RESULTS.md` are the only
  status surfaces that exist.

## Context
Raspberry Pi 5, headless, HDMI to a 100" TV. Booth audio in from a Pioneer
DDJ-FLX10 through a Sound Blaster Play! 3. Stream Deck XL for control.
The whole engine is one Chromium window: WebGL2 + Web Audio + WebHID + `<video>`.
No Rust, no drivers, no root.
