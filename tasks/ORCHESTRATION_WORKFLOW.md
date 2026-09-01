# Multi-Agent File-Based Orchestration Protocol
## (Codex Orchestrator ↔ Sonnet Worker)

This protocol enables **Codex (Project Manager / Reviewer)** and **Sonnet 4.6 / 5 low (Coding Worker)** to collaborate efficiently without token waste or context bloat.

---

## 1. Core Rule: Zero-Context-Bloat Principle

*   **Do NOT** paste entire repository architecture documents or multi-megabyte chat histories into worker sessions.
*   **Do NOT** have agents talk in long chat threads when handing off tasks.
*   **DO** use strict, self-contained markdown task files located in `tasks/specs/`.
*   A Worker session requires **only two inputs**:
    1. The Worker Prompt Template with the path to the task spec.
    2. The 1 or 2 source files specified in that task spec.

---

## 2. File-Based State Machine

```
   [ Codex (Manager) ]
           │
           │ 1. Writes Task Spec
           ▼
   tasks/specs/TASK-XX.md  ──► (Status: TODO in tasks/TRACKER.md)
           │
           │ 2. Sonnet (Worker) executes & writes code + unit test
           ▼
   [ Source Code + Tests ]
           │
           │ 3. Worker updates Status to IN_REVIEW
           ▼
   tasks/TRACKER.md
           │
           │ 4. Codex reviews code diff & runs tests
           ▼
   tasks/reviews/REVIEW-XX.md ──► (Status: DONE or REVISE)
```

---

## 3. Step-by-Step Roles & Workflows

### Role A: Codex (Project Manager & Orchestrator)
1. **Picks the next task** from `tasks/TRACKER.md`.
2. **Generates the task spec** in `tasks/specs/TASK-XX-<name>.md` with:
   - Target files (max 2).
   - Exact input/output data types.
   - Prohibited dependencies/actions.
   - Acceptance criteria and required unit test cases.
3. **Marks task** as `TODO` in `tasks/TRACKER.md`.
4. **Reviews completion**:
   - Inspects the git diff.
   - Runs `cargo test -p <crate_name>`.
   - Writes `tasks/reviews/REVIEW-XX-<name>.md` and updates `tasks/TRACKER.md` to `DONE` (or `REVISE` with bulleted feedback).

---

### Role B: Sonnet 4.6 / Sonnet 5 low (Worker / Coder)
1. **Reads ONLY** the specified `tasks/specs/TASK-XX.md` and the 1–2 target source files.
2. **Writes the implementation** strictly adhering to the types and boundaries.
3. **Writes the self-contained unit tests** `#[cfg(test)]`.
4. **Runs tests** to verify they pass.
5. **Updates `tasks/TRACKER.md`** status to `IN_REVIEW`.

---

## 4. Standard Prompts

### Prompt for Sonnet Worker:
```markdown
You are the implementation worker for RPi-VJ.
Please read and execute the task defined in:
`tasks/specs/TASK-XX-<name>.md`

Rules:
- Read only the target files listed in the spec.
- Modify only the allowed files.
- Write complete, compiling Rust code with comprehensive unit tests.
- Update `tasks/TRACKER.md` to IN_REVIEW when done.
```

### Prompt for Codex Orchestrator (Review Phase):
```markdown
You are the Project Manager / Reviewer for RPi-VJ.
Task `TASK-XX` has been submitted for review.

1. Inspect the code changes made in `[target files]`.
2. Verify against the acceptance criteria in `tasks/specs/TASK-XX-<name>.md`.
3. Run `cargo test -p [crate]`.
4. Write the review summary to `tasks/reviews/REVIEW-XX-<name>.md` and update `tasks/TRACKER.md`.
```
