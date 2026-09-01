# Model Routing & Context Economy
**Compiled 2026-08-25.** Figures from vendor/aggregator pages that month — tiers
move fast, re-check quarterly. Benchmarks are directional; your own task-level
results beat any leaderboard.

## Part 1 — The lineups, grounded

### OpenAI / Codex (GPT-5.6, three durable tiers)
| | Sol | Terra | Luna |
|---|---|---|---|
| API $/M in / out | 5 / 30 | 2 / 12 | 0.20 / 1.20 |
| Terminal-Bench 2.1 | 88.8% | 87.4% | 84.7% |
| Agents' Last Exam | 53.6 | 50.4 | 50.3 |
| SWE-Bench Pro | 64.6% | — | — |
| **MRCR (long ctx)** | 91.5% | 89.6% | **41.3%** |

Two things matter here. Terra is **1.4 points** behind Sol on Terminal-Bench at
40% of the price — the flagship premium buys very little for ordinary agentic
coding. And Luna has a **long-context cliff**: 41.3% recall. Luna is 25× cheaper
and nearly as good *only* on short, self-contained work.

### Anthropic / Claude
| | Fable 5 | Opus 5 | Opus 4.8 | Sonnet 5 | Sonnet 4.6 |
|---|---|---|---|---|---|
| API $/M in / out | 10 / 50 | ~5 / 25 | 5 / 25 | 2–3 / 10–15 | 3 / 15 |
| SWE-Bench Pro | 80.3% | 79.2% | 69.2% | 63.2% | 58.1% |
| SWE-Bench Verified | — | 96% | — | 85.2% | — |
| Context | — | 1M | 200K | 1M | 200K |

Fable 5 and Opus 5 are effectively tied on repo-level coding and both lead the
field. Sonnet 5 gives ~80% of Opus 5's coding score at ~40% of the cost and
matches Opus 4.8 on knowledge work. Note Fable 5 routes cyber/bio queries to
Opus 4.8 via a classifier — irrelevant here, but it surprises people.

### Google / Gemini
| | 3.5 Pro | 3.5 Flash |
|---|---|---|
| API $/M in / out | ~12–15 / 36–45 | 1.50 / 9 |
| Context | **2M** | large |
| Notes | Deep Think, 4 intensity levels | beats Gemini 3.1 Pro, ~4× faster |

Antigravity 2.0's agent harness is **co-optimised with 3.5 Flash** and is built
for orchestrating parallel agents. That's a real structural advantage for the
legwork tier, independent of raw model quality. AI Ultra ($100) = 5× the Pro
plan's Antigravity limit.

## Part 2 — Role → model

The organising principle: **spend where errors are silent and compounding; save
where errors are loud and immediate.** A bad architecture decision surfaces in
three weeks. A bad function surfaces when you run it. Price the role by how long
a mistake stays invisible, not by how hard the task feels.

| Role | Best | Economical & sufficient | Effort | Why |
|---|---|---|---|---|
| Deep research | **Gemini 3.5 Pro**, Deep Think medium | GPT-5.6 Terra | med | 2M context is the whole point — read 40 sources without chunking. Infrequent, so cost is fine. |
| Plan of development | GPT-5.6 Terra | Sonnet 5 | high | Judgment over knowledge. Flagships add little. |
| **Project architect** | **Claude Opus 5** (or Fable 5) | Sol / 3.5 Pro Deep Think high | high/max | Errors invisible for weeks. Highest-leverage spend in the whole table, and rare enough to be cheap in absolute terms. |
| Scaffolding | Sonnet 5 | Luna / 3.5 Flash | low–med | Mechanical once architecture is fixed. **Danger zone** — see the warning below. |
| **Breaking work into micro-specs** | **Claude Opus 5** | Sonnet 5 high + a fixed template | high | The single most underrated role. Spec quality *is* the cheap tier's success rate. |
| Orchestration (adaptive) | **Antigravity 2.0 + 3.5 Flash** | Sonnet 5 | low | Purpose-built harness for parallel agents. |
| Orchestration (deterministic) | **a shell script** | — | — | If the DAG is known, a model is pure overhead and a source of drift. |
| Code review | **Opus 5 or Fable 5** (risky diffs) | Terra (routine diffs) | high / med | Must be a **different family than the author** — same family shares blind spots. |
| Adversarial verify | Sol or Opus 5, prompted to *refute* | Terra ×3, majority vote | high | Distinct from review: the job is to falsify, not to assess. |
| Bulk implementation | Sonnet 5 | **Luna** or 3.5 Flash | low–med | Luna only works if the spec is short and closed — see Part 3. |
| Context curation / summarising | Luna / 3.5 Flash | — | low | Never spend a flagship on compressing text. |
| Drift audit (periodic) | Opus 5 | Sol | high | "Does the board match reality?" Schedule it. Today's audit found six months of false DONEs. |
| **Rule enforcement** | **none — write code** | — | — | See below. This is the important row. |

### Roles you didn't list but need
- **Adversarial verifier** — separate from reviewer. A reviewer asks "is this
  good?"; a verifier tries to prove the claim false. Different prompt, and it
  catches a different class of error.
- **Integrator** — merges parallel agent output. Nobody assigns it and it silently
  becomes your job.
- **Ground-truth runner** — executes the observable on real hardware. **This is
  you, Frank, and it cannot be delegated.** It is also the only role that has
  ever caught anything in this project.
- **Drift auditor** — scheduled, not accidental.

### The rule-enforcement row, expanded
You asked which model can "enforce everyone to adhere to strict rules." **None
of them can, reliably.** Every model will drift given enough turns, and the
better it writes, the more convincing the drift. What happened to Tasks 06–13
was not a weak model — it was plausible output with nothing to fail against.

Enforcement has to be **mechanical**:
- A CI check that fails if `BOARD.md` says `DONE` and `RESULTS.md` has no
  `observable:` line for that ID.
- A schema validator on the results format.
- `grep` for banned dependencies, or for `todo!`/`unimplemented!`.
- A pre-commit hook rejecting a spec whose file list is empty.

**Cheap model + hard gates beats expensive model + good intentions**, every time
and for a fraction of the price. Spend your engineering effort on the gates.

### Warning on scaffolding
Scaffolding is exactly where this project went wrong, and it's a structural
trap, not a model failure: scaffolding *by definition* produces structure
without substance, so it looks complete while being empty. Never let a
scaffolding task end without an observable attached. "It compiles" is not one.

## Part 3 — Context economy

The protocol in `AGENT_PROTOCOL_V2.md` isn't only about saving money. It is what
keeps tasks inside **Luna's and Flash's competence envelope**. Luna scores 84.7%
on Terminal-Bench but 41.3% on long-context recall — so a short, closed spec
makes a 25×-cheaper model nearly as good as the flagship, and a sprawling one
makes it dangerous. Spec discipline and cost discipline are the same lever.

### The twelve techniques

**1. Pass artifacts, not transcripts.** The unit of handoff is a file. Never
paste a conversation into another conversation.

**2. Every conversation ends by writing itself down.** ≤10 lines to a durable
file, then let it die. If a session's value lives only in its transcript, you
will pay to re-read that transcript forever.

**3. A spec is a closure.** Inline every constant, signature and band edge so the
worker's context is *the spec and nothing else*. Measurable target: **≤2 repo
files opened**. Over that, split the task.

**4. Decompose at interfaces.** Give each task the API signature its neighbours
expose. Then no task needs another's internals. This is the real context
reducer — same reason modularity works for humans.

**5. Never re-derive a finding.** `ENGINE_REALITY_CHECK.md` is marked
authoritative precisely so no future agent spends 30k tokens re-auditing.
Findings are durable artifacts, not conversation residue.

**6. One status surface.** Status docs breed, and every copy gets read by every
session. This repo had four overlapping ones; now it has `BOARD.md`.

**7. Structured output over prose.** A table or JSON blob costs a fraction of a
narrative and can be machine-checked. Ban "let me summarise what we did."

**8. Grep, don't read.** Instruct agents to search for what they need. Loading
a file "for context" is how a 2k-token task becomes 40k.

**9. Kill long conversations.** Every turn re-sends the entire history, so cost
grows roughly with turns². Four ten-turn conversations are dramatically cheaper
than one forty-turn conversation — and usually produce better work, because
context rot is real.

**10. Never let auto-compaction be your handoff.** It's lossy in unpredictable
places and it always drops the detail you needed. Write the handoff yourself,
before you're near the limit.

**11. Keep the stable prefix stable.** Prompt caching rewards an unchanging
head. Put the fixed protocol first, the variable task last. Don't reorder.

**12. Solve "what is this project?" exactly once.** A short `PROJECT.md` (<100
lines) that every session opens, and nothing else.

### The arithmetic that motivates this
The three briefing docs total ~19 KB ≈ **5,000 tokens**, and the old protocol
had every session read all three before doing anything. Over 50 sessions that's
**250,000 tokens of pure re-reading**, before a single line of work.
`START_HERE.md` + one spec is ~1,200 tokens. Same information, at the moment
it's needed, scoped to the task at hand.

### Applied to this project
| Stage | Who |
|---|---|
| Specs, architecture, review, drift audits | Claude Opus 5 — rare, high effort |
| Implementation from a closed spec | Codex Luna or Gemini 3.5 Flash |
| Anything long-context (research, whole-repo) | Gemini 3.5 Pro — never Luna |
| Parallel legwork | Antigravity 2.0 + 3.5 Flash |
| Second-opinion review of Claude-authored code | Terra or Sol — different family, on purpose |
| Running observables on the Pi | Frank |
