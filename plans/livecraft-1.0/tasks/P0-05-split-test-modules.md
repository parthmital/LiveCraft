# Task P0-05: Move the large inline test modules out of `lib.rs`

Depends on: P0-04. Size: 4 files (2 created), about 1300 lines moved, 0 logic changes. Milestone: P0.

## Goal

`crates/livecraft-core/src/lib.rs` and `crates/livecraft-dsp/src/lib.rs` hold only module declarations and re-exports; their tests live in `src/tests.rs` as child modules, with identical results.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-04 is ticked and P0-05 is not.
3. Run `cargo test --workspace`. Expected: 44 tests pass (core 13, dsp 25).

## Context

- `crates/livecraft-core/src/lib.rs` is 523 lines; lines 15 onward are `#[cfg(test)] mod tests { ... }`. `crates/livecraft-dsp/src/lib.rs` is 857 lines; lines 53 onward are its test module. Both test modules are the last item in the file.
- `plans/livecraft-1.0/tools/split_test_module.py` moves such a block into `src/tests.rs` and leaves `#[cfg(test)] mod tests;`. A child module keeps `use super::*;` working, so no visibility changes are needed. The script was dry-run on 2026-10-27: both crates compiled and all tests passed afterwards.
- This task also creates the repo-local Python environment that later tasks use.

## Steps

1. If `.venv` does not exist, create it and install the plan tools:

   ```bash
   python -m venv .venv
   ./.venv/Scripts/python -m pip install -r plans/livecraft-1.0/tools/requirements.txt
   ```

   On macOS or Linux use `./.venv/bin/python`. Expected: pip ends with `Successfully installed` (or `Requirement already satisfied`).

2. Run:

   ```bash
   ./.venv/Scripts/python plans/livecraft-1.0/tools/split_test_module.py crates/livecraft-core/src/lib.rs
   ./.venv/Scripts/python plans/livecraft-1.0/tools/split_test_module.py crates/livecraft-dsp/src/lib.rs
   ```

   Expected: `moved 506 lines from crates/livecraft-core/src/lib.rs to ...tests.rs` and `moved 802 lines from crates/livecraft-dsp/src/lib.rs to ...tests.rs`.

3. Run `cargo fmt --all`.

4. Run `wc -l crates/livecraft-core/src/lib.rs crates/livecraft-dsp/src/lib.rs`. Expected: 16 and 54 lines.

5. In `ARCHITECTURE.md`, find:

   ```text
   | Large files mixing concerns: `model.rs` (1208 lines), `graph.rs` (1072), `devices.rs` (990), `dsp/src/lib.rs` (800 lines of tests) | as listed | 0 and 1 |
   ```

   Replace it with:

   ```text
   | Large files mixing concerns: `model.rs` (1208 lines), `graph.rs` (1072), `devices.rs` (990) | as listed | 1 |
   ```

## Do Not

- Change files other than: `crates/livecraft-core/src/lib.rs`, `crates/livecraft-core/src/tests.rs` (new), `crates/livecraft-dsp/src/lib.rs`, `crates/livecraft-dsp/src/tests.rs` (new), `ARCHITECTURE.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not edit the moved tests. Do not commit `.venv`.

## Verify

| Command                                                 | Expected result                 |
| ------------------------------------------------------- | ------------------------------- |
| `cargo fmt --all -- --check`                            | exit 0                          |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                          |
| `cargo test --workspace`                                | 44 tests pass (core 13, dsp 25) |
| `cargo build --release --workspace`                     | exit 0                          |
| `git status --short`                                    | no `.venv` entry                |

## Commit And Push

1. Tick P0-05 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-core/src/lib.rs crates/livecraft-core/src/tests.rs crates/livecraft-dsp/src/lib.rs crates/livecraft-dsp/src/tests.rs ARCHITECTURE.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Move inline test modules out of lib.rs

   - Move 506 lines of livecraft-core tests to src/tests.rs
   - Move 802 lines of livecraft-dsp tests to src/tests.rs
   - Update the known violations table
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead.

## If Something Fails

- The script refuses (`not the last item` or `already exists`): log a blocker with its message.
- Other Verify failures: fix within the listed files, at most 3 attempts.
- Still failing: do not commit code. Append under Blockers in `CHECKLIST.md`: `P0-05, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, stop.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, log a blocker, stop.
