# Task P0-18: Verify Phase 0 and bring the docs up to date

Depends on: P0-17. Size: 5 files, about 80 changed lines. Milestone: P0.

## Goal

Every Phase 0 exit criterion in `plans/livecraft-1.0/PLAN.md` is proven with recorded evidence, and `README.md`, `ARCHITECTURE.md`, `AGENTS.md`, and `START-HERE.md` describe the repository as it now is.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-17 is `[x]` or `[~]` and P0-18 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 53 tests.

## Context

Phase 0 exit criteria (from `PLAN.md`): `cargo xtask ci` passes locally and in CI on Windows, Ubuntu, macOS; `cargo-deny` clean; edition 2024; egui 0.37 with wgpu; SECURITY.md findings LC-001 to LC-004 fixed with tests.

Expected test counts after Phase 0: livecraft-cli 4, livecraft-core 15, livecraft-dsp 26 unit plus 2 integration (`wav_limits`), livecraft-gui 2, xtask 4; total 53.

## Steps

1. Collect evidence (save outputs under `.agent-local/output/p0-18/`):
   - `cargo xtask ci > .agent-local/output/p0-18/xtask-ci.log 2>&1`; last line `xtask: all checks passed`.
   - `cargo test --workspace 2>&1 | grep "test result"`: note each count.
   - `gh run list --workflow CI --limit 1 --json conclusion,headSha`: `success` for the latest pushed commit (if `gh` is unavailable, write "CI not verified locally" in the review).
   - `grep -n "edition" Cargo.toml`, `cargo tree -i egui --depth 0`, `cargo tree -i quick-xml --depth 0`, `cargo tree -i cpal --depth 0`.
   - `grep -E "^\| LC-00[1-4]" SECURITY.md`: all four show `Fixed`.
   - Screenshots: `cargo dev -- gui --screenshot .agent-local/output/p0-18/session.png --view session` and with `--view arrangement --theme light` into `arrangement-light.png`.

2. Create `plans/livecraft-1.0/milestones/P0-review.md`:

   ```markdown
   # Phase 0 Review

   Date: <today>. Commit: <git rev-parse --short HEAD>.

   | Exit criterion                      | Met      | Evidence                               |
   | ----------------------------------- | -------- | -------------------------------------- |
   | cargo xtask ci passes locally       | <Yes/No> | <last line of xtask-ci.log>            |
   | CI passes on Windows, Ubuntu, macOS | <Yes/No> | <gh run list output>                   |
   | cargo-deny clean                    | <Yes/No> | <cargo-deny summary line>              |
   | Edition 2024                        | <Yes/No> | <grep output>                          |
   | egui 0.37 with wgpu                 | <Yes/No> | <cargo tree output>, screenshots saved |
   | LC-001 to LC-004 fixed with tests   | <Yes/No> | <grep output>                          |

   Test counts: <per crate>.
   ```

   Fill every `<...>` with the real values. Any `No` row: log a blocker instead of ticking this task.

3. `README.md`:
   - Quick Start table, first row: replace `44 tests pass (4 cli, 13 core, 25 dsp, 2 gui)` with `53 tests pass`.
   - Testing and Code Quality: replace the sentence starting `Baseline on 2026-10-27:` with `Baseline after Phase 0 (<today>): cargo xtask ci passes; 53 tests (cli 4, core 15, dsp 28, gui 2, xtask 4).`
   - Repository Structure tree: add the lines `|-- xtask/                     cargo xtask ci: checks, layering, docs links, cargo-deny`, `|-- deny.toml                  Dependency licence and advisory policy`, `|-- rust-toolchain.toml        Pinned Rust 1.99.1`, and `|-- .cargo/config.toml         cargo xtask and cargo dev aliases`, keeping alphabetical-ish order consistent with the existing tree.

4. `ARCHITECTURE.md`: set `Last verified:` to today; in Decisions add `- wgpu renderer with DX12 default on Windows: Vulkan crashed on the reference machine (DECISIONS.md D12).`; confirm every Known Violations row still describes current code (open each listed file and check); delete rows that no longer apply.

5. `plans/livecraft-1.0/START-HERE.md`: in Project Facts, after the line that starts `- Baseline on 2026-10-27`, add `- Baseline after Phase 0 (<today>): cargo xtask ci passes, 53 tests.`

6. `AGENTS.md`: set `Last verified:` to today.

7. Run `cargo xtask ci` again (docs link check covers the edits).

## Do Not

- Change files other than: `plans/livecraft-1.0/milestones/P0-review.md` (new), `README.md`, `ARCHITECTURE.md`, `plans/livecraft-1.0/START-HERE.md`, `AGENTS.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Fix code in this task. A failing criterion becomes a blocker.

## Verify

| Command          | Expected result                      |
| ---------------- | ------------------------------------ |
| `cargo xtask ci` | `xtask: all checks passed`, 53 tests |
| `grep -c "       | Yes                                  | " plans/livecraft-1.0/milestones/P0-review.md` | `6` |

## Commit And Push

1. Tick P0-18 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add plans/livecraft-1.0/milestones/P0-review.md README.md ARCHITECTURE.md plans/livecraft-1.0/START-HERE.md AGENTS.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Verify Phase 0 and update docs

   - Record evidence for every Phase 0 exit criterion
   - Update test counts, repository structure, and decisions in the docs
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- A criterion is not met: fix it with the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md` (one `Fix <criterion>` commit per fix). Still not met: append under Blockers in `CHECKLIST.md`: `P0-18, <date>: <criterion> not met`, with the evidence. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
