# Task Specification: TB-03 — Verify wgpu clear-color window on macOS/M2 Pro

## 1. Goal
Confirm the exact code from TB-02 also runs correctly on the M2 Pro MacBook Pro via wgpu's Metal backend. This
is a verification task, not a new-feature task — if `gpu-spike` was written correctly in TB-02 it should need
zero or near-zero changes here. If it needs real changes, that itself is the finding.

## 2. Target Files
* **Modify** (only if something fails): `vj/tools/gpu-spike/src/main.rs`
* Read `vj/tools/gpu-spike/Cargo.toml` — do not add macOS-only deps unless a build error forces it; if it does,
  gate them with `[target.'cfg(target_os = "macos")'.dependencies]`, never unconditionally.

## 3. Requirements & Constraints
1. `git pull` / sync the repo so this Mac has the TB-02 code (do not re-derive it from this spec — read the
   actual current `vj/tools/gpu-spike/src/main.rs` file, it is the single source of truth, not the TB-02 spec
   text).
2. Run `cargo build -p gpu-spike` on the Mac. Fix only what's needed to get it building — the most likely
   failure is a Windows-only crate feature or a `PowerShell`/path assumption in a build script, not the wgpu
   code itself, since `Backends::PRIMARY` already auto-selects Metal on macOS.
3. Do not change `Backends::PRIMARY` to `Backends::METAL` explicitly — if the wrong backend gets selected, that
   is a bug to report, not something to paper over by hardcoding it (TB-02's whole point was writing
   platform-portable init code once).
4. If wgpu's Metal backend requires a minimum macOS/Xcode command-line-tools version to build, note the exact
   versions installed on this Mac in your report.

## 4. Acceptance Criteria & Observable
Run `cargo run -p gpu-spike --release` **on the physical M2 Pro MacBook**. A window must appear showing the same
solid blue-teal color as TB-02, no crash. Stdout must print the adapter line; it should name an Apple GPU
(`"Apple M2 Pro"` or similar) and backend `Metal`.

## 5. Report
Append to `tasks/RESULTS-TRACKB.md`: the exact adapter line printed on the Mac, whether any code changes were
needed (paste a diff if so — this is important, it tells the architect whether "write once" actually held),
macOS/Xcode CLT versions, anything that surprised you. ≤10 lines. `DONE` only if observed running on the
physical Mac.
