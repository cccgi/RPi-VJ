# Brief: adapt the too-heavy shaders so they run on the Pi 5

For a capable model working alone, with no prior knowledge of this project.
Read all of it before you start. It gives you the goal, the constraints and what
is already known; how you get there is yours to decide.

## 1. The situation

RPi-VJ is a live-visuals appliance. A Raspberry Pi 5 (2 GB) runs a Rust engine
that draws audio-reactive GLSL shaders and video clips to HDMI at 1080p60. The
owner performs with it: a Stream Deck selects shaders, a MIDI controller bends
and moves them, and the music drives their motion. There is no browser and no
desktop on the Pi.

The shader library is mostly converted from Shadertoy. Shadertoy shaders are
written for desktop GPUs; the Pi's GPU (VideoCore VII, Mesa V3D 7.1) is roughly
a hundred times weaker than a desktop card. Every shader is therefore measured
on the Pi before it may enter the show.

On 2026-10-07 the owner added 612 shaders. They were benchmarked exactly as
converted, with no changes. Of the 532 that the engine can run at all:

| Outcome | Count |
|---|---|
| Qualify at 1080p | 46 |
| Qualify at 720p | 53 |
| Too heavy at 720p | 367 |
| Render black, flat or static | 14 |
| Hang or stall the GPU | 31 |
| Need an input the Pi cannot supply | 21 |

**Your job is the 367 too-heavy ones: find out how many can be made to run
well enough to be worth having, make them run, and prove it on the Pi.** The
owner's words: "see if we can make any good use of them".

Success is not "all 367 pass". Many will not, and saying so with evidence is a
valid result. Success is a set of adapted shaders that qualify on the Pi and
still look like something a VJ would want, plus an honest account of the rest.

## 2. What "qualifies" means

A shader is drawn into an offscreen target at a **render tier** (1080 or 720
lines; the compositor scales it to the output). The benchmark measures the GPU
time of that pass per frame.

- Verdict from the 95th-percentile frame time: **SOLID** up to 10 ms, **OK** up
  to 13 ms, **MARGINAL** up to 16 ms, otherwise **FAIL**. The frame is 16.7 ms
  and compositing and video need the rest, which is why the limit is 13.
- A shader qualifies at the highest tier that is SOLID or OK under **both**
  audio patterns the benchmark uses: a 128 BPM beat, and every band pinned at
  maximum. Music changes the cost of some shaders; the second pattern is the
  worst case.
- **720p is the floor.** The owner layers shaders over video clips and does
  not want shaders below 720p. Do not target 540p or 360p.
- The benchmark also records pixel statistics and a thumbnail. Output that is
  black, a single colour, or does not change between frames is flagged and
  does not qualify, however fast it is.

## 3. What the owner cares about

These come from the owner's own decisions during the project. Treat them as
requirements.

1. **The shaders are audio-reactive instruments.** Motion driven by the music
   is the point. An adaptation that freezes or removes the reaction to save
   time defeats the purpose. Lower iteration counts, not responsiveness.
2. **It has to still look good.** A shader that passes but has turned into
   noise, banding or a blur of its former self is not a success. If the only
   way to pass destroys the look, record that and move on.
3. **720p minimum**, as above.
4. **Bright and normal, not dimmed.** The owner compared a darker variant of
   the bank and rejected it.
5. **Seamless.** Shaders are moved, spun and scrolled live. One that only
   works with the camera fixed at the screen centre is less useful, though not
   disqualified.
6. **Nothing that endangers the show.** A shader that hangs the GPU, or takes
   more than about 1.5 s to draw its first frame, freezes the output in front
   of an audience. Those are removed from the library, never deployed.

## 4. What you may and may not do

You may:

- Read anything in the repository.
- Write adapted copies of shaders and extend the adaptation tool
  (section 6). Add tests for what you add.
- Run benchmarks on the Pi, following `docs/native/SHADER_BENCH_RUNBOOK.md`
  for the mechanics (upload, run, watch, fetch, report).
- Write your findings into the documents named in section 9.

You may not:

- **Change, move or delete the library files** in `target/native-13-shaders`,
  `target/shadertoy-fractals/shaders` or `engine/examples/shaders`. They are
  the owner's originals. Adapted versions are separate copies.
- **Deploy to the live show.** Do not touch `/opt/rpi-vj-native` or
  `/srv/rpi-vj-native` on the Pi. You produce a candidate; the owner or the
  project lead deploys.
- **Rebuild or replace the engine binaries** on the Pi. If you believe the
  engine itself is the limit, write down why and stop there.
- **Write the Pi password** into any file in the repository. Ask the owner
  for it and keep helpers that hold it outside the repository.
- **Commit or push.** Leave your work uncommitted and report.
- **Start a Pi benchmark without the owner's go-ahead.** A run blacks out the
  HDMI output and disables the controllers for its whole duration. Agree a
  window first, and batch your runs so there are few of them.

If you hit something this brief does not cover and the action would be hard to
undo or would affect the live show, stop and ask.

## 5. What is already known

Read these before designing anything; they will save you most of the dead ends.

- `docs/native/SHADER_ADAPTATION_RULES.md` is the evidence base. It records
  what the owner's own hand-built shaders do to fit the Pi, with measurements:
  loop bounds, ray-march step counts, noise taps per pixel, trigonometry per
  pixel, and which changes rescued shaders that were too slow. Section 4.1
  lists the rules that evidence supports; 4.2 lists general tile-GPU advice
  that is **unverified on this hardware**; section 6 lists what was never
  measured. Treat 4.2 and 6 as hypotheses to test, not facts.
- `docs/native/SHADER_QUALIFICATION_PLAN.md` section 3 defines the benchmark
  manifest and result formats and the measurement rules.
- `docs/native/SHADER_BENCH_RESULTS.md` is the generated report of every
  measurement so far, including the batch you are working on.
- `target/shader-bank/catalog.json` has one record per shader with static
  features extracted from its source: loop counts and bounds, nesting, an
  estimated iteration product, whether it ray-marches, noise and fbm hints,
  anti-aliasing and quality defines, texture fetches inside loops. Of the 367,
  109 are flagged as ray-marchers and 365 contain loops.
- `target/shader-bank/results/new-2026-10-07/results-new-2026-10-07.jsonl`
  holds the as-is measurement of each of them: per-tier frame times under both
  audio patterns, compile time, pixel statistics, thumbnail path.
- `target/shader-bank/qualified.json` lists the 367 under `too_heavy` (filter
  by the ids in `target/shader-bank/todo-2026-10-07.txt`, which is the batch).

Facts worth knowing up front:

- About 60 of the 367 are near misses: 13 to 22 ms at 720p. Roughly 300 are
  far over, some by ten times or more. The effort is best spent where the gap
  is smallest, but a far-over shader with one dominant loop can sometimes be
  rescued by a single change, so do not rule them out by frame time alone.
- 14 of the 367 already have a `lite` variant (anti-aliasing and quality
  defines turned down) that has **never been measured**. Measuring those costs
  almost nothing and is the obvious first run.
- Cost scales with pixel count, so 720p costs about 44% of 1080p. A shader
  that needs 20 ms at 1080p may already pass at 720p as it is; check the
  720p figure before adapting.
- The bank shaders end in a generated wrapper `main()` that zooms, turns and
  shakes the coordinates with the audio and then calls the shader's
  `mainImage`. Its cost is included in every measurement. Do not remove it:
  it is where the audio reaction comes from for shaders that have none of
  their own.
- The engine reads the pixel position through a helper so it can move and
  spin a shader live. A shader that derives everything from `gl_FragCoord`
  and `u_resolution` works with that automatically. Avoid adaptations that
  depend on the pixel position being the true screen pixel (for example
  dithering patterns or `texelFetch` on screen coordinates).
- First-frame time matters as well as steady frame time. A shader whose
  first frame takes over 1.5 s is rejected as dangerous, and long shader
  compiles stall the show when a key is pressed.

## 6. The tools

All under `engine/tools/shader_bank/`; each has a docstring describing its
options. Run them from the repository root in Git Bash with `PYTHONUTF8=1`.

- `adapt.py` writes **variants**: adapted copies under
  `target/shader-bank/variants/<variant>/<id>.fs`, with a `transforms.json`
  recording exactly which rule changed what in each file. `base` is the
  untouched copy; `lite` applies only look-preserving changes. Its docstring
  explains how to add a rule and a variant. Rules are functions from source
  text to new text plus a list of edits; they run in a fixed order.
- `glsl_features.py` is the static analyser behind the catalogue. Use and
  extend it instead of writing a second parser.
- `make_manifest.py` builds a benchmark manifest and the files to upload for a
  chosen set of shader ids and variants.
- `report.py` turns results files into the report, `qualified.json` and a
  candidate show. Given several variants of one shader it picks the best that
  qualifies.
- `tests/test_shader_bank.py` covers all of the above. It must still pass
  when you are done: `python -m unittest discover -s tests` in that directory.

The intended home for your work is a new variant (the plan calls it `pi`)
built from rules, so that every change is reproducible, recorded and can be
re-applied when the library grows. Hand-edited one-off copies are acceptable
where a shader needs judgement no rule can express, but keep them in their own
variant directory, record each edit, and prefer a rule wherever the same
change helps more than a couple of shaders.

You can check that an adapted shader still compiles and what it looks like
without the Pi: the engine prepares shaders with `engine/src/shader.rs`, and
any desktop GL ES 3.0 context will do for a visual check. Only the Pi can tell
you the cost. Budget your Pi time accordingly.

## 7. How to approach it

This is a suggestion, not a script.

1. **Triage from the data you already have.** Join the as-is results with the
   catalogue features. Group the 367 by how far over they are and by what
   dominates their cost. You are looking for classes of shader that share a
   fix, because a rule that rescues thirty shaders is worth more than thirty
   hand edits.
2. **Take the free wins first.** Measure the 14 unmeasured `lite` variants.
   Check which shaders only fail at 1080p.
3. **Form hypotheses and test them on a small sample.** For each candidate
   change (fewer ray-march steps, a larger step epsilon, fewer fbm octaves,
   cheaper normals, removing a second light or a shadow march, lowering a
   quality define, hoisting work out of a loop), apply it to a handful of
   shaders, measure, and look at the pictures. Keep what works; write down
   what does not and why.
4. **Scale up what worked** into rules, apply across the group, and benchmark
   the group.
5. **Give the remainder a fair look and then stop.** Decide per class whether
   further work is worthwhile and say why.

Things that have helped on this hardware (see the rules document for the
evidence): ray-march step counts around 28 instead of 60 to 100; cheaper
normal estimation; at most two fbm octaves; no loop that calls another loop;
replacing per-step trigonometry with values computed once.

Things to be careful with:

- Changing two things at once tells you nothing about either.
- A fixed step count that works at rest may fail when loud audio zooms or
  warps the scene. The worst-case audio pattern exists to catch that; do not
  judge on the beat pattern alone.
- Fewer steps can turn a surface into holes or banding. Always look at the
  thumbnail, and for anything you intend to recommend, at more than one frame.
- Do not adapt by clamping, freezing or ignoring the audio uniforms.

## 8. Verifying your results

Before you report a shader as rescued, all of these must hold:

- It is SOLID or OK at 720p or above under both audio patterns, measured on
  the Pi, in a run you can point to.
- Its first frame and compile time are within the limits.
- Its pixel statistics carry no black, flat or static flag.
- You have looked at its thumbnail beside the original's and it is
  recognisably the same shader. Where it is not, say so; the owner may still
  want it, but that is their call.
- It still reacts to audio: the picture under the loud pattern differs from
  the picture under the beat pattern.

Run the benchmark results for the whole adapted set at least once more at the
end, in one run, so the final numbers come from a single consistent session
and not from a patchwork of experiments.

## 9. What to hand back

1. **The adapted shaders**, as variants produced by the tool, with their
   `transforms.json`.
2. **The code**: new rules, any analyser extensions, and tests, uncommitted.
3. **A findings section** added to `docs/native/SHADER_ADAPTATION_RULES.md`:
   each rule you tried, on how many shaders, what it saved, what it cost
   visually, and whether you recommend it. Move items from "unverified" to
   "supported" or "refuted" where you now have measurements. This is the
   lasting value of the work, so be specific and include the numbers.
4. **A results table** in a new file
   `docs/native/SHADER_ADAPTATION_RESULTS.md`, one row per shader of the 367:
   id, title, as-is frame time at 720p, what was applied, frame time after,
   verdict (rescued at 1080p, rescued at 720p, passes but look degraded, not
   rescued, dangerous), and a short note where the reason is not obvious.
5. **A candidate show** built by `report.py` that includes the rescued
   shaders, not deployed.
6. **A short summary for the owner** at the top of the results file, in plain
   language: how many were rescued and at what tier, how many pass but look
   worse, how many are out of reach and the main reasons, how many are
   dangerous and should leave the library, what you would do next with more
   time, and anything in this brief or in the tools that turned out to be
   wrong.

State plainly what you verified and what you are inferring. If a number comes
from a small sample, say how small. If something failed, include the output.
