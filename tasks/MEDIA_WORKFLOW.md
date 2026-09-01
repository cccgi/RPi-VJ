# Adding new clips — the whole workflow

One command does everything. Run from the repo root on Windows.

```powershell
.\gig\tools\add-clips.ps1 -Source "E:\__VJ\New Pack" -Name neon
```

That scans for seamless loop points, cuts and encodes to 1080p, adds Stream
Deck page(s), and copies only that folder to the Pi. `-Name` becomes both the
media folder (`gig/media/neon`) and the bank name (`NEON`).

## Do the scan on its own first

```powershell
.\gig\tools\add-clips.ps1 -Source "E:\__VJ\New Pack" -Name neon -Scan
```

Nothing is modified. You get a table:

```
clip                              dur     loop   winErr   seam  verdict
4K Abstract Techno Laser         60.0s   30.00s   0.02   0.01  SEAMLESS
4K Abstract Techno LED          459.6s  116.23s   3.83   2.73  uncertain - check
```

* **loop** — where the clip repeats. Round numbers (15.00, 30.00) are a good sign:
  rendered loops are exported at round durations.
* **seam** — the wrap discontinuity as a multiple of the clip's own frame-to-frame
  motion. **1.0 means the wrap looks like any other frame.** Under 1.5 is invisible.
* **winErr** — how well the next 45 frames match at the loop point. One frame
  matching can be luck; forty-five cannot.
* **uncertain** clips are skipped unless you pass `-IncludeUncertain`. They are
  usually long non-repeating animations, or their period is beyond `-MaxLoop`.

Happy with the table? Re-run without `-Scan`.

## Options worth knowing

| Flag | Default | Why |
|---|---|---|
| `-Height` | 1080 | `0` keeps source resolution. 4K plays but costs far more per frame. |
| `-MinLength` | 15 | A real 2s loop is seamless but reads as a stutter. This takes **whole multiples** of the loop, so it stays seamless and lasts longer. |
| `-MaxLoop` | 120 | Longest period to look for. Raise it if long clips come back `uncertain`. |
| `-Bank` | `-Name` uppercased | Page name on the Deck. |
| `-IncludeUncertain` | off | Cut the doubtful ones too. Preview them before a show. |

## Running one phase at a time

`-Scan`, `-Build`, `-BankOnly`, `-Push` each run just that step. Useful when an
encode succeeded but the transfer was interrupted — `-Push` on its own resumes,
skipping any file already on the Pi at the right size.

## Removing a set

```powershell
python gig\tools\build-clip-bank.py --bank "NEON" --remove
```

Pages go away; the media folder stays. Delete `gig\media\neon` yourself if you
want the disk space back.

## Rules the scripts enforce so you don't have to remember them

* **Sources are never modified.** Everything is written to `gig/media/<Name>`.
* **Frame-exact cuts.** A stream copy can only cut on a keyframe, and being a
  few frames out is exactly what makes a wrap visible — so it re-encodes.
* **Filenames are mapped** (`"A B.mp4"` → `"A_B.mp4"`), because `show.json` and
  the thumbnails are keyed by filename.
* **Only the named media folder is pushed.** `sync.ps1 -WithMedia` re-copies the
  whole library; `-MediaDir <name>` copies one folder, file by file, resumable,
  and counts the files on the Pi afterwards.

## If something looks wrong on the TV

```
ssh rpi@rpi-vj.local 'ls ~/RPi-VJ/gig/media/<name>/*.mp4 | wc -l'
ssh rpi@rpi-vj.local 'grep -E "clip PLAY|clip load FAILED" ~/vj-results/show.log | tail'
ssh rpi@rpi-vj.local '~/RPi-VJ/gig/deploy/install-units.sh --check'
```

A clip pad that does nothing is nearly always a filename mismatch — the log
line names the file it tried to fetch.
