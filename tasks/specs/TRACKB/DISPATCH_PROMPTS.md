# Track B Dispatch Prompts

Copy one block per worker conversation, in order (TB-02 needs TB-01 done, TB-03 needs TB-02 done, etc.).
Paste into a cheap/mid-tier model's chat with repo access (Cursor, Copilot Chat, a Claude/GPT session with
file tools — whatever you're routing to). Each prompt is self-contained per `AGENT_PROTOCOL_V2.md`: the worker
should open ONLY the spec file named, plus the file(s) the spec explicitly names. Don't paste extra project
context "to help" — that's the thing this protocol exists to prevent.

---

## TB-01 — dispatch to: cheap model

```
You're working in the RPi-VJ repo, a Rust Cargo workspace at the path I'll give you. Open and follow exactly
one file: tasks/specs/TRACKB/TB-01-wgpu-deps.md — do not read any other file in tasks/ or docs/, only the files
that spec itself names as targets.

Do exactly what the spec says, no more. When done, append your ≤10-line report to tasks/RESULTS-TRACKB.md in
the format the spec's §5 describes (don't overwrite the file — append a new entry below whatever's already
there). Then tell me in one line whether you'd mark this TODO/NEEDS-REVIEW/DONE per the spec's own acceptance
criteria, and why.

This machine: [Windows PC / RTX 3090 — or say which box you're actually running on].
```

---

## TB-02 — dispatch to: cheap model, MUST run on the 3090 PC

```
You're working in the RPi-VJ repo, a Rust Cargo workspace. TB-01 (adding wgpu/winit/pollster deps) is already
done. Open and follow exactly one file: tasks/specs/TRACKB/TB-02-wgpu-window-windows.md — do not read any other
file in tasks/ or docs/, only the files that spec names as targets (and its one named reference, the TB-01
Cargo.toml, to confirm deps are present).

This task is only DONE if you actually run `cargo run -p gpu-spike --release` on the physical RTX 3090 PC and
see the window. If you can only compile it (no GPU / no display in your environment), say so explicitly and
mark it NEEDS-REVIEW in your report rather than claiming DONE.

Append your ≤10-line report to tasks/RESULTS-TRACKB.md (append, don't overwrite). Tell me the exact adapter
line it printed.
```

---

## TB-03 — dispatch to: cheap model, MUST run on the M2 Pro MacBook

```
You're working in the RPi-VJ repo, a Rust Cargo workspace, on the M2 Pro MacBook. TB-02 already built and ran
this same gpu-spike binary on a separate Windows/3090 machine — you're verifying/adapting it here, not writing
it from scratch. Open and follow exactly one file: tasks/specs/TRACKB/TB-03-wgpu-window-macos.md. Per that
spec, read the CURRENT tools/gpu-spike/src/main.rs as your source of truth for what the code already does —
don't re-derive it from the TB-02 spec text, the repo file is authoritative.

Make only the changes needed to get it building and running on macOS; the spec explains what's likely and
what's out of bounds (don't hardcode the Metal backend).

This task is only DONE if you actually run it on the physical M2 Pro Mac and see the window. If you can't run
it on real hardware in your environment, mark NEEDS-REVIEW and say so.

Append your ≤10-line report to tasks/RESULTS-TRACKB.md (append, don't overwrite), including a diff if you
needed to change anything — that's the actual finding this task is for.
```

---

## TB-04 — dispatch to: cheap model, MUST run on the 3090 PC

```
You're working in the RPi-VJ repo. TB-01–TB-03 are done — gpu-spike opens a window and clears it to a color on
both the 3090 PC and the Mac. Open and follow exactly one file: tasks/specs/TRACKB/TB-04-triangle.md — no other
file in tasks/ or docs/.

Run it on the 3090 PC when done; confirm the triangle's three corners are red/green/blue and smoothly
interpolated, not solid-colored per-vertex-only or missing.

Append your ≤10-line report to tasks/RESULTS-TRACKB.md (append, don't overwrite).
```

---

## TB-05 — dispatch to: mid-tier model, MUST run on the 3090 PC

```
You're working in the RPi-VJ repo. gpu-spike currently draws a hardcoded triangle. Open and follow exactly one
file: tasks/specs/TRACKB/TB-05-isf-quad.md. It names exactly one other file to read for reference — an existing
GLSL shader you're hand-porting to WGSL, not running as-is. Don't open any other shader or any docs/ file.

This one requires judgment porting GLSL idioms (mod, wrapping, coordinate convention) to WGSL — the spec flags
the known gotchas, but if you hit one it doesn't mention, make the visually-faithful call and describe exactly
what you changed and why in your report; don't silently simplify the pattern away.

Run it on the 3090 PC; the whole window should fill with the shader's pattern, correctly oriented, not
stretched or confined to a corner.

Append your ≤10-line report to tasks/RESULTS-TRACKB.md (append, don't overwrite).
```

---

## TB-06 — dispatch to: mid-tier model, MUST run on the 3090 PC

```
You're working in the RPi-VJ repo. gpu-spike currently renders a fullscreen shader with hardcoded/const
uniform values. Open and follow exactly one file: tasks/specs/TRACKB/TB-06-live-uniforms.md — no other file.

Run it on the 3090 PC after: confirm the pattern now animates over time, confirm resizing the window doesn't
crash it, and report the actual fps number printed — don't adjust anything to make the number look "expected,"
just report what you measured.

Append your ≤10-line report to tasks/RESULTS-TRACKB.md (append, don't overwrite).
```

---

## TB-07 — dispatch to: mid-tier model, run on BOTH the 3090 PC and the M2 Pro Mac

```
You're working in the RPi-VJ repo. gpu-spike currently renders live and prints fps to stdout. Open and follow
exactly one file: tasks/specs/TRACKB/TB-07-metrics-log.md — no other file.

This task needs two runs on two different physical machines (the 3090 PC and the M2 Pro Mac) — if you only
have access to one of them in this conversation, do that machine's run, say clearly which machine you covered
and which one is still outstanding, and don't mark it DONE until both CSVs exist.

The spec explicitly defers GPU temperature/memory logging — don't add nvml-wrapper or IOKit calls to "complete"
that gap, just flag it in your report as the spec says to.

Append your ≤10-line report (plus the first/last 3 CSV rows, which don't count against the line limit) to
tasks/RESULTS-TRACKB.md (append, don't overwrite).
```

---

## After TB-07: not a dispatch task
B1's exit criterion is ADR-100 — a written backend-choice justification from the measured numbers. That's an
architect call (bring the two CSVs back to an expensive-model conversation, or to me), not something to hand to
a cheap/mid worker. Don't write a TB-08 "write ADR-100" dispatch prompt.

---
---

# Round 2 — TB-08 … TB-12 (closing Gate B for ADR-100)

Added 2026-08-27 alongside `docs/v2/adr/ADR-100-render-backend.md`. TB-01–TB-07 answered "does wgpu port"; these
answer "does it perform". Same rules: paste one block, one worker, one conversation. **Do not paste the ADR
into a worker conversation** — the specs already inline what each worker needs.

Order: TB-09 → TB-10 → TB-11 are a chain on the desktops and must run in that order (each builds on the
previous one's CSV columns). TB-08 is independent and can run in parallel **on or after 2026-08-29** — the Pi
is the gig machine until then. TB-12 runs last, on the final binary.

---

## TB-08 — dispatch to: cheap model, MUST run on a physical Pi 5, NOT before 2026-08-29

```
You're working in the RPi-VJ repo, a Rust Cargo workspace, on a Raspberry Pi 5. A gpu-spike tool already
exists and has been run successfully on a Windows/RTX 3090 box and an M2 Pro Mac — you are finding out whether
it comes up on this Pi's Vulkan driver. Open and follow exactly one file:
tasks/specs/TRACKB/TB-08-pi5-first-pixel.md — do not read any other file in tasks/ or docs/, only the files
that spec names.

Important: a clean failure is a valid, useful result here. If it doesn't work, do NOT go hunting for
workarounds or switch rendering approaches — capture the exact error and the diagnostic command output the
spec asks for, and report that. Making it work is somebody else's task.

Append your ≤10-line report to tasks/RESULTS-TRACKB.md (append, don't overwrite). Tell me the exact adapter
line or the exact error, and whether you'd mark it DONE or NEEDS-REVIEW per the spec's own criteria.
```

---

## TB-09 — dispatch to: mid model, MUST run on both the 3090 PC and the M2 Pro Mac

```
You're working in the RPi-VJ repo, a Rust Cargo workspace. The gpu-spike tool renders a fullscreen shader and
logs per-frame timings to CSV, but it's vsync-locked so the timings don't show real GPU cost. Open and follow
exactly one file: tasks/specs/TRACKB/TB-09-uncapped-frametime.md — do not read any other file in tasks/ or
docs/, only the two source files that spec names as targets.

The CSV column order and the percentile definitions in that spec are consumed by later tasks — match them
exactly, don't improve on them.

Run it on both physical machines as the spec's §4 describes. If you only have one of the two, do that one,
mark NEEDS-REVIEW and say which machine is missing. Append your ≤10-line report to tasks/RESULTS-TRACKB.md.
```

---

## TB-10 — dispatch to: mid model, MUST run on both the 3090 PC and the M2 Pro Mac

```
You're working in the RPi-VJ repo, a Rust Cargo workspace. TB-09 is done: gpu-spike now supports an uncapped
present mode and prints a percentile summary. Open and follow exactly one file:
tasks/specs/TRACKB/TB-10-1080p-gateb-workload.md — do not read any other file in tasks/ or docs/, only the
three files that spec names (main.rs, quad.wgsl, and the one GLSL shader it tells you to port).

You're raising the render workload to a realistic weight, not making it look good. Keep the blend pass as
trivial as the spec describes and don't add uniform fields.

Run all four configurations on both physical machines. Append your ≤10-line report to
tasks/RESULTS-TRACKB.md, and list every GLSL construct that didn't port cleanly to WGSL — that list is the
point of the task almost as much as the numbers are.
```

---

## TB-11 — dispatch to: mid model. PC and Mac now; Pi only on/after 2026-08-29

```
You're working in the RPi-VJ repo, a Rust Cargo workspace. TB-10 is done: gpu-spike renders a heavy shader to
a 1920x1080 offscreen target and blends it to the window, with per-frame CSV metrics and percentile summaries.
Open and follow exactly one file: tasks/specs/TRACKB/TB-11-texture-import-cost.md — do not read any other file
in tasks/ or docs/, only the source file that spec names as a target.

This task measures a cost; it does not fix it. If per-frame texture upload turns out to be slow, report the
number and stop — do NOT implement staging buffers, ring buffers, or any zero-copy import path. That decision
belongs to a later phase and depends on the number you're producing.

Run the matched import/noimport pair on each machine you have access to. Append your ≤10-line report to
tasks/RESULTS-TRACKB.md and state the mean/p99 upload_ms and the import-vs-noimport p99 delta explicitly.
```

---

## TB-12 — dispatch to: cheap model. One conversation per machine is fine

```
You're working in the RPi-VJ repo, a Rust Cargo workspace. The gpu-spike tool is finished — this is a
run-and-observe task with zero code changes. Open and follow exactly one file:
tasks/specs/TRACKB/TB-12-soak.md — do not read any other file in tasks/ or docs/.

If anything makes you want to edit source, stop and report BLOCKED instead of editing. The run takes 30+
minutes of wall clock and you must not use the machine for anything else during it.

Append your ≤10-line report to tasks/RESULTS-TRACKB.md, stating plainly per machine whether it passes the
memory and thermal conditions the spec lists.
```
