# Runbook: benchmark a new batch of shaders on the Pi 5

For an agent with no prior knowledge of this project. Follow it top to bottom.
It covers the batch added to the library on 2026-10-07 and any later batch.

## 1. What you are doing and why

RPi-VJ is a headless VJ appliance: a Raspberry Pi 5 (2 GB) runs a Rust engine
that draws audio-reactive GLSL shaders and video clips to HDMI at 1080p60. A
Stream Deck is the only user interface. The shader library is large and mostly
converted from Shadertoy. Most of it is too heavy for the Pi's GPU, so every
shader is measured on the Pi itself before it may go into the show.

Your task: measure the shaders that have not been measured yet, regenerate the
results report, and hand back a list of what qualifies. **You do not deploy
anything to the live show.** The owner or the project lead does that.

Design background, if you need it: `docs/native/SHADER_QUALIFICATION_PLAN.md`
(section 3 defines the manifest and result formats) and the "Shader benchmark"
section of `engine/README.md`.

## 2. Rules

1. **The benchmark stops the live show.** The Pi's HDMI output goes dark for
   the whole run (about an hour for 500 entries). Ask the owner before you
   start the run in step 6.3, and do not start it if they are using the Pi.
2. Never write the Pi's password into any file in this repository, including
   scripts, logs and this document. Get it from the owner. Keep helper scripts
   that contain it outside the repository.
3. Do not change, move or delete anything in the shader library
   (`target/native-13-shaders`, `target/shadertoy-fractals/shaders`,
   `engine/examples/shaders`). The tools write converted copies elsewhere.
4. Do not touch `/opt/rpi-vj-native` or `/srv/rpi-vj-native` on the Pi. Those
   are the live show. The benchmark has its own directories (section 3).
5. Do not rebuild or replace the engine binary. Use the one installed at
   `/opt/rpi-vj-bench/rpi-vj-native`.
6. Do not commit to git. Leave your output files in place and report.
7. If `/home/rpi/DEPLOY_IN_PROGRESS` exists on the Pi, someone is deploying.
   Stop and ask.
8. If something fails in a way this document does not cover, stop and report
   what you saw. Do not improvise on the Pi.

## 3. Where things are

Workstation (Windows, repository at `E:\__Projects\RPi-VJ`, use Git Bash):

| Path | Contents |
|---|---|
| `engine/tools/shader_bank/` | The tools: `catalog.py`, `adapt.py`, `fetch_assets.py`, `make_manifest.py`, `report.py` |
| `target/shader-bank/catalog.json` | One record per library shader, with its class |
| `target/shader-bank/variants/<variant>/<id>.fs` | Converted copies that are actually measured |
| `target/shader-bank/assets/` | Input textures for texture shaders, plus `manifest.json` |
| `target/shader-bank/staging/<name>/` | What gets uploaded for one run |
| `target/shader-bank/results/<name>/` | Results and thumbnails fetched back from the Pi |
| `docs/native/SHADER_BENCH_RESULTS.md` | Generated report (do not edit by hand) |

Raspberry Pi (`rpi@rpi-vj3.local`, SSH):

| Path | Contents |
|---|---|
| `/opt/rpi-vj-bench/rpi-vj-native`, `pi_bench.py` | Benchmark engine and its supervisor |
| `/srv/rpi-vj-bench/shaders/<variant>/<id>.fs` | Uploaded shaders |
| `/srv/rpi-vj-bench/assets/` | Uploaded textures |
| `/var/lib/rpi-vj-bench/` | Manifests, results (`*.jsonl`), thumbnails, `supervisor.log` |

Terms:

- **Class**: `selfcontained` (needs no input), `needs_texture` (reads an image
  input), `multipass` (needs buffer passes the engine does not have; never
  measured), `original18` (the owner's hand-built shaders).
- **Variant**: a converted copy. `base` is the shader as converted. `tex` is
  `base` with a texture fix, where one applies. `lite` lowers quality settings.
  `react` and `react_lite` are the same shaders with the new audio-reactive
  wrapper. The owner compared both on 2026-10-07 and kept the original
  wrapper, so measure `base` (and `tex`) only.
- **Tier**: the height the shader is drawn at: 1080 or 720. The owner does not
  use shaders below 720, so lower tiers are not measured.
- **Verdict** per tier, from the 95th-percentile frame time: SOLID up to 10 ms,
  OK up to 13 ms, MARGINAL up to 16 ms, otherwise FAIL. A shader qualifies at
  the highest tier that is SOLID or OK under **both** audio patterns (a 128 BPM
  beat, and every band pinned at maximum). The engine measures both.

## 4. Workstation quirks

Run every command from the repository root in Git Bash.

- Git needs `git -c safe.directory=E:/__Projects/RPi-VJ ...`.
- Python must read UTF-8: run `export PYTHONUTF8=1` first.
- Git Bash rewrites arguments that look like Unix paths. Prefix any command
  that passes a Pi path such as `/var/lib/...` with `MSYS_NO_PATHCONV=1`.
  Check the manifest afterwards: if you see `C:/Program Files/Git/var/...` in
  it, the prefix was missing.
- The tools need Python 3.10 or later and Pillow (`pip install pillow`).
- SSH: there is no `sshpass`. Either install an SSH key on the Pi once
  (`ssh-copy-id rpi@rpi-vj3.local`) or use an `SSH_ASKPASS` helper kept outside
  the repository. `sudo` on the Pi asks for the same password.

Check the tools work before you start:

```sh
export PYTHONUTF8=1
cd engine/tools/shader_bank && python -m unittest discover -s tests && cd -
```

All tests must pass. If they do not, stop and report.

## 5. Prepare the batch (workstation)

Pick a short run name with the date, for example `new-2026-10-07`. It is
written as `$RUN` below: `export RUN=new-2026-10-07`.

**For the 2026-10-07 batch, steps 5.1, 5.2 and 5.4 are already done.** The
catalogue was refreshed, the variants were written, and the manifest was built
at `target/shader-bank/staging/new-2026-10-07/` with 532 entries (532 shaders,
`base`). Do step 5.3, then run step 5.4 again only if 5.3 fetched
new files, then go to section 6. For a later batch, do all four.

### 5.1 Refresh the catalogue

```sh
cp target/shader-bank/catalog.json target/shader-bank/catalog.before-$RUN.json
python engine/tools/shader_bank/catalog.py --changes-out target/shader-bank/todo-$RUN.txt
```

Expected: a `CATALOG <n> shaders` summary and a line
`since previous catalog: added=..., removed=..., content_changed=...`.
`todo-$RUN.txt` then holds one shader id per line: everything added or changed.
If `added` and `content_changed` are both 0 there is nothing to measure.

### 5.2 Write the variants

```sh
python engine/tools/shader_bank/adapt.py
```

Expected: one `VARIANT <name>: <n> of <total> shaders` line per variant.

### 5.3 Fetch missing textures

Texture shaders need their input images. This contacts shadertoy.com.

```sh
python engine/tools/shader_bank/fetch_assets.py --help     # read the options first
python engine/tools/shader_bank/fetch_assets.py
```

It reports how many inputs it resolved and how many are still missing. If the
site blocks the download, do not fight it: record the count of unresolved
inputs and continue. Shaders whose inputs are missing are measured anyway and
reported as "needs texture"; they do not enter the show. Details:
`docs/native/SHADER_ASSETS.md`.

### 5.4 Build the manifest and staging directory

```sh
MSYS_NO_PATHCONV=1 python engine/tools/shader_bank/make_manifest.py --name $RUN \
  --class selfcontained --class needs_texture \
  --ids-file target/shader-bank/todo-$RUN.txt \
  --variant base --variant tex \
  --pi-out /var/lib/rpi-vj-bench/results-$RUN.jsonl \
  --pi-thumb-dir /var/lib/rpi-vj-bench/thumbs-$RUN
```

Expected: `MANIFEST <n> entries, tiers [720, 1080]`. It writes
`target/shader-bank/staging/$RUN/manifest.json`, a `shaders/` tree, and
`assets.txt` (the texture files this run needs). A note about shaders that are
"only partly bound" is normal.

Check: `grep -c "Program Files" target/shader-bank/staging/$RUN/manifest.json`
must print 0.

## 6. Run it on the Pi

### 6.1 Check the Pi is free

```sh
ssh rpi@rpi-vj3.local 'systemctl is-active rpi-vj-bench; ls /home/rpi/DEPLOY_IN_PROGRESS; vcgencmd measure_temp; df -h /var/lib | tail -1'
```

`rpi-vj-bench` must be `inactive`, the lock file must not exist, and there must
be at least 1 GB free. If a benchmark is already running, stop and ask.

### 6.2 Upload

```sh
tar czf /tmp/$RUN-shaders.tgz -C target/shader-bank/staging/$RUN shaders
tar czf /tmp/$RUN-assets.tgz -C target/shader-bank/assets -T target/shader-bank/staging/$RUN/assets.txt
scp /tmp/$RUN-shaders.tgz /tmp/$RUN-assets.tgz target/shader-bank/staging/$RUN/manifest.json rpi@rpi-vj3.local:/tmp/
ssh rpi@rpi-vj3.local "tar xzf /tmp/$RUN-shaders.tgz -C /srv/rpi-vj-bench \
  && tar xzf /tmp/$RUN-assets.tgz -C /srv/rpi-vj-bench/assets \
  && mv /tmp/manifest.json /var/lib/rpi-vj-bench/$RUN.json \
  && ls /srv/rpi-vj-bench/shaders && ls /srv/rpi-vj-bench/assets | wc -l"
```

If `assets.txt` is empty, skip the assets archive. Uploading does not disturb
the live show, but do it before the run, never during one.

### 6.3 Start the run (stops the live show; owner's go-ahead required)

```sh
ssh -t rpi@rpi-vj3.local "sudo systemd-run --unit=rpi-vj-bench --collect \
  -p StandardOutput=append:/var/lib/rpi-vj-bench/supervisor.log \
  -p StandardError=append:/var/lib/rpi-vj-bench/supervisor.log \
  /usr/bin/python3 /opt/rpi-vj-bench/pi_bench.py /var/lib/rpi-vj-bench/$RUN.json \
  --stop-services --restart-services --max-strikes 6"
```

The run is detached and survives your SSH session. The supervisor stops the two
live services, measures every entry, records a shader that hangs or crashes the
GPU and carries on, pauses above 80 C, and starts the live services again when
it ends, however it ends.

### 6.4 Watch it

```sh
ssh rpi@rpi-vj3.local "systemctl is-active rpi-vj-bench; tail -3 /var/lib/rpi-vj-bench/supervisor.log; grep -c '\"result\"' /var/lib/rpi-vj-bench/results-$RUN.jsonl"
```

Check every 10 to 15 minutes. Expect 3 to 9 seconds per entry, so 532 entries
take about an hour. Do not upload, build or run
anything else on the Pi while it runs: that distorts the measurements.

Normal: lines with status `too_slow`, `gpu_reset`, `hang`, `crash`,
`compile_error` or `prepare_error` for individual shaders. Those are findings.

Not normal, stop and report:

- the unit is `failed`, or `inactive` with fewer results than entries;
- the log says it aborted after consecutive strikes, or reports a kernel oops;
- the result count has not moved for 10 minutes while the unit is `active`.

Running the same `systemd-run` command again resumes where it stopped; results
already written are kept. Do that at most once before reporting. To stop a run
early: `sudo systemctl stop rpi-vj-bench` (the live services are restored).

### 6.5 Confirm the end state

```sh
ssh rpi@rpi-vj3.local "systemctl is-active rpi-vj-bench rpi-vj-native rpi-vj-controller; grep -c '\"result\"' /var/lib/rpi-vj-bench/results-$RUN.jsonl; journalctl -u rpi-vj-native --since -1min --no-pager | tail -3"
```

Required: `inactive`, `active`, `active`; the result count equals the manifest's
entry count; the engine log shows `renderer frames=...` lines advancing by about
60 per second. If the live services did not come back, run
`sudo systemctl start rpi-vj-native rpi-vj-controller` and say so in your report.

## 7. Fetch the results and build the report (workstation)

```sh
mkdir -p target/shader-bank/results/$RUN
ssh rpi@rpi-vj3.local "tar czf /tmp/$RUN-out.tgz -C /var/lib/rpi-vj-bench results-$RUN.jsonl thumbs-$RUN"
scp rpi@rpi-vj3.local:/tmp/$RUN-out.tgz /tmp/
tar xzf /tmp/$RUN-out.tgz -C target/shader-bank/results/$RUN
```

Then regenerate the report from every results file, oldest first, new one last:

```sh
R=target/shader-bank/results
python engine/tools/shader_bank/report.py \
  $R/wp-a-proof.jsonl $R/wave1/results-wave1.jsonl $R/wave2/results-wave2.jsonl \
  $R/textures/results-textures.jsonl $R/wp-j-proof.jsonl $R/$RUN/results-$RUN.jsonl \
  --variants base,lite,tex --min-tier 720 \
  --thumbs-dir $R/wave1/thumbs --thumbs-dir $R/textures/thumbs-textures \
  --thumbs-dir $R/wp-j-thumbs --thumbs-dir $R/$RUN/thumbs-$RUN \
  --show-dir target/shader-bank/show-720-$RUN
```

If earlier batches exist under `target/shader-bank/results/`, add their results
files (in date order, before the new one) and their thumbnail directories.

This rewrites `docs/native/SHADER_BENCH_RESULTS.md` and
`target/shader-bank/qualified.json`, and writes a candidate show (shader files,
key pictures, textures, `show.json`) to `target/shader-bank/show-720-$RUN/`.
It prints the counts per outcome and per tier.

## 8. Check the output before you report

1. In `qualified.json`, `summary.not_measured` counts shaders with no result.
   For the classes you measured it should be 0 (multipass shaders are listed
   separately as excluded).
2. Look at 20 thumbnails of qualified shaders (`.ppm` files open in most image
   viewers, or convert with Pillow). A qualified shader with a black or
   single-colour thumbnail is a problem: list it.
3. The report marks shaders whose output was black, flat or static as
   "renders but suspect". The owner wants those listed for checking on a
   desktop PC. Do not try to fix them.
4. Shaders with status `hang`, `gpu_reset`, `crash` or a first frame over
   1.5 s are dangerous to the live show. List them separately: the owner
   removes those from the library.

## 9. What to hand back

A short written report with:

- run name, date, start and end time, number of entries, and whether the run
  completed in one go or had to be resumed;
- counts: qualified at 1080p, qualified at 720p, too heavy, suspect, failed,
  needs texture;
- the list of dangerous shaders (section 8, item 4) with their status;
- the list of suspect shaders (section 8, item 3);
- texture inputs that could not be fetched;
- the final state of the Pi from step 6.5;
- anything that did not match this document.

Files you leave behind: `target/shader-bank/results/$RUN/`,
`target/shader-bank/show-720-$RUN/`, the
regenerated `docs/native/SHADER_BENCH_RESULTS.md` and `qualified.json`.
Do not deploy the candidate show and do not commit.
