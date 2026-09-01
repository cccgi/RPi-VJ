# Agent Protocol v2 — Token Discipline
**Supersedes `ORCHESTRATION_WORKFLOW.md`.** Effective 2026-08-25.

The v1 protocol produced accurate-sounding status from agents that had loaded
too much context to check anything. v2 optimises for **small, cheap, verifiable
units**. A task that a Haiku/Sonnet-class model cannot complete without reading
the whole repo is a badly written task — split it.

## The five rules

1. **One task = one conversation = one spec file.**
   The worker opens the spec and nothing else from `tasks/`. Never paste
   `CLAUDE_COLLABORATION_BRIEFING.md`, `SYSTEM_STATE_SNAPSHOT.md`, or this file
   into a worker conversation.

2. **Specs are self-contained.** A spec must inline every constant, signature
   and band edge the worker needs. If a spec requires the worker to read more
   than **two** existing repo files, it is too big. Split it.

3. **Every task ends in an observable.** Not "implement X" — "after this,
   running `Y` prints `Z`" or "opening `page.html` shows N". A task with no
   observable cannot be marked DONE, ever, by anyone.

4. **Report in ≤10 lines, appended to `RESULTS.md`.** Files changed, the
   observable's actual output, and anything that surprised you. No essays. No
   code in chat. No re-describing the spec back.

5. **`BOARD.md` is the only status file.** One line edited per task. Nobody
   writes a new summary document. Nobody regenerates a snapshot.

## Status values
`TODO` → `WIP` → `BLOCKED(reason)` → `NEEDS-REVIEW` → `DONE`

**`DONE` means the observable was observed on the target hardware.** Compiling
is not DONE. Passing self-written unit tests is not DONE. If it has only ever
run on a dev PC, the status is `NEEDS-REVIEW` and the note says so.

## Model routing
| Work | Model |
|---|---|
| Single-file implementation from a written spec | cheapest available |
| Porting a known algorithm between languages | cheap |
| Shader authoring / perf tuning | mid |
| Writing the specs, reviewing risk, deciding architecture | expensive, rarely |

The expensive model's job is to write specs and adjudicate — not to write code.
If you find yourself asking the expensive model to implement, you have a spec
that isn't finished.

## Anti-patterns that cost real money here
- Re-reading briefing docs at the start of every session. Read `BOARD.md`.
- Generating a fresh "snapshot" or "summary" doc. There is one board.
- Marking things DONE from code inspection. See `ENGINE_REALITY_CHECK.md`.
- Adding a new layer on top of an untested layer.
