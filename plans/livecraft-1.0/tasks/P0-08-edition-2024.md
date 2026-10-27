# Task P0-08: Migrate the workspace to Rust edition 2024

Depends on: P0-07. Size: about 14 files, about 200 changed lines, all produced by `cargo fix`, `cargo clippy --fix`, and `cargo fmt`. Milestone: P0.

## Goal

Every crate builds with edition 2024, with clippy clean and the same test results.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-07 is ticked and P0-08 is not.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 49 tests.

## Context

- Decision D10: edition 2024. The edition is set once in `[workspace.package]` in `Cargo.toml` (Task P0-01).
- Dry run on 2026-10-27 (on the code before Phase 0, with `set_var` already removed as in Task P0-02): `cargo fix --edition` rewrote 8 files (some `if let ... else` became `match`, which keeps 2021 drop order), added no `unsafe`; after switching the edition, clippy reported 27 `collapsible_if` errors because edition 2024 allows `if let` chains; `cargo clippy --fix` fixed all of them; then clippy was clean and all tests passed.
- rustfmt formats with the 2024 style edition afterwards, so many files get whitespace-only changes. That is expected.

## Steps

1. Run `cargo fix --edition --workspace --all-targets --allow-dirty`. Expected: ends without `error`; lines like `Migrating ... from 2021 edition to 2024` and `Fixed ...`.
2. In `Cargo.toml`, inside `[workspace.package]`, find `edition = "2021"` and replace it with `edition = "2024"`.
3. Run `cargo clippy --fix --workspace --all-targets --allow-dirty`. Expected: lines `Fixed <file> (<n> fixes)` and no errors.
4. Run `cargo fmt --all`.
5. Run `grep -rn "unsafe" crates xtask`. Expected: no output. If `unsafe` appears, log a blocker (the workspace forbids it).
6. In `README.md`, in the Tech Stack table, find `| Rust                     | 2021 edition            |` and replace `2021 edition` with `2024 edition`, keeping the column alignment.

## Do Not

- Hand-edit Rust code. Every Rust change in this task comes from the three commands above.
- Change `Cargo.toml` beyond the edition line, or any file other than: Rust files changed by the commands, `Cargo.toml`, `README.md`, `plans/livecraft-1.0/CHECKLIST.md`.

## Verify

| Command                                                                              | Expected result                        |
| ------------------------------------------------------------------------------------ | -------------------------------------- |
| `grep -n "edition" Cargo.toml`                                                       | `edition = "2024"`                     |
| `cargo xtask ci`                                                                     | `xtask: all checks passed`, 49 tests   |
| `cargo dev -- gui --screenshot .agent-local/output/p0-08/session.png --view session` | prints `Screenshot successfully saved` |

## Commit And Push

1. Tick P0-08 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add -u` is allowed in this task only (every change is to a tracked file), then `git add plans/livecraft-1.0/CHECKLIST.md`. Check `git status --short` shows no untracked files you meant to add.
3. Commit:

   ```text
   Migrate the workspace to Rust edition 2024

   - Apply cargo fix --edition, then set edition 2024 in workspace.package
   - Collapse nested if-let statements with cargo clippy --fix
   - Reformat with the 2024 style edition
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead.

## If Something Fails

- `cargo clippy` still reports lints that `--fix` cannot apply: fix them by hand following the lint's suggestion text exactly, only in the files it names. At most 3 attempts.
- Tests fail after the migration: run `git diff` on the failing crate, find the `cargo fix` change in the code path of the failing test, and log a blocker with that diff hunk; do not change test code.
- Still failing: `git checkout -- .` to discard, append under Blockers in `CHECKLIST.md`: `P0-08, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, stop.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, log a blocker, stop.
