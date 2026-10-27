# Task M01-10: Verify M01 workspace restructure and update docs

Depends on: M01-09. Size: 5 files, about 80 changed lines. Milestone: M01.

## Goal

Verify every M01 acceptance criterion with recorded evidence, update `ARCHITECTURE.md`, `README.md`, and `AGENTS.md` to reflect the new crate layout, and write `plans/livecraft-1.0/milestones/M01-review.md`.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-09 is `[x]` and M01-10 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- `plans/livecraft-1.0/milestones/M01-workspace-restructure.md` Acceptance Criteria:
  1. `cargo xtask ci` passes; characterisation tests pass unchanged.
  2. `cargo metadata` lists exactly target packages: `livecraft-model`, `livecraft-formats`, `livecraft-dsp`, `livecraft-devices`, `livecraft-engine`, `livecraft-ui`, `livecraft-testkit`, `livecraft`, `xtask`.
  3. No references to `livecraft_core`, `livecraft_gui`, `livecraft_cli` in any Rust source file.
  4. `ARCHITECTURE.md` module map matches the target layout.

## Steps

1. Run verification checks:
   - `cargo xtask ci` -> exits 0, 55 tests.
   - Characterisation tests: `cargo test -p livecraft --test characterisation` -> 2 passed.
   - Check packages: `cargo metadata --format-version 1 --no-deps | jq -r '.packages[].name' | sort`
   - Check legacy names: `git grep -E "livecraft_core|livecraft_gui|livecraft_cli" -- "*.rs"` -> empty.

2. Create `plans/livecraft-1.0/milestones/M01-review.md` from the outputs of step 1. Fill every cell from what the commands printed; never write `Yes` for a check that did not run or did not pass:

   ```markdown
   # Milestone M01 Review: Workspace Restructure

   Date: <date>. Commit: <git rev-parse --short HEAD>.

   | Acceptance criterion                       | Met      | Evidence                 |
   | ------------------------------------------ | -------- | ------------------------ |
   | cargo xtask ci passes                      | <Yes/No> | <final line, test count> |
   | Characterisation tests pass unchanged      | <Yes/No> | <test output>            |
   | Target packages match target architecture  | <Yes/No> | <sorted package list>    |
   | No legacy crate names in Rust source files | <Yes/No> | <git grep output>        |
   | ARCHITECTURE.md module map matches layout  | <Yes/No> | <sections updated>       |

   Test counts: <per-package counts from cargo test>.
   ```

3. Update `ARCHITECTURE.md`:
   - Set `Last verified: 2026-10-27`.
   - Update `## Module Map` table with the new crate paths (`crates/livecraft-model`, `crates/livecraft-formats`, `crates/livecraft-dsp`, `crates/livecraft-devices`, `crates/livecraft-engine`, `crates/livecraft-ui`, `crates/livecraft-testkit`, `apps/livecraft`, `xtask`).
   - Update `## Dependency Rules` to reflect target layer constraints.

4. Update `README.md`:
   - Quick Start: update test count (55 tests pass).
   - Repository Structure: update tree to show `apps/livecraft` and `crates/{livecraft-model, livecraft-formats, livecraft-dsp, livecraft-devices, livecraft-engine, livecraft-ui, livecraft-testkit}`.
   - Testing and Code Quality: update baseline description.

5. Update `AGENTS.md`:
   - Update command examples to reference new crates (`cargo test -p livecraft-model`, `livecraft-devices`, `livecraft-engine`, `livecraft-ui`, `apps/livecraft`).

6. Run `cargo xtask ci` to verify docs and all checks.

## Do Not

- Change any production code in this task.
- Leave any legacy crate name in documentation examples.

## Verify

| Command                                                                | Expected result                      |
| ---------------------------------------------------------------------- | ------------------------------------ |
| `git grep -E "livecraft_core\|livecraft_gui\|livecraft_cli" -- "*.rs"` | no output                            |
| `cargo test -p livecraft --test characterisation`                      | 2 passed                             |
| `cargo xtask ci`                                                       | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-10 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add plans/livecraft-1.0/milestones/M01-review.md ARCHITECTURE.md README.md AGENTS.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Verify M01 workspace restructure and update docs

   - Record M01 review with characterisation test evidence
   - Update ARCHITECTURE.md module map with target crates
   - Update README.md repository tree and AGENTS.md test commands
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Read the whole error and find the root cause. If this file is wrong (an anchor, API, or expected value changed), correct it and add `Revision <date>: <what and why>` at its end. If the line still cannot be finished, log it under Blockers in `CHECKLIST.md`.
