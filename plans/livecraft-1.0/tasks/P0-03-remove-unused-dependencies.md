# Task P0-03: Remove unused dependencies

Depends on: P0-02. Size: 5 files, about 10 changed lines. Milestone: P0.

## Goal

No crate declares a dependency it does not use, and the workspace no longer declares `ringbuf`.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-02 is `[x]` or `[~]` and P0-03 is `[ ]`.
3. Run `cargo test --workspace`. Expected: 44 tests pass.

## Context

Verified on 2026-10-27 by searching each crate's `src` and `tests` for uses:

| Crate            | Unused dependency                |
| ---------------- | -------------------------------- |
| `livecraft-core` | `serde_json`                     |
| `livecraft-dsp`  | `ringbuf`, `serde`, `serde_json` |
| `livecraft-cli`  | `serde`                          |

`ringbuf` is used nowhere, so its line in the root `Cargo.toml` `[workspace.dependencies]` goes too. Removing it also removes a cargo-deny advisory seen on 2026-10-27 ("Double free / use-after-free in `Consumer::skip` and `Consumer::clear` when an element's `Drop` panics"). `log` stays everywhere (Task P0-02 uses it).

## Steps

1. Run, one at a time:

   ```bash
   cargo remove serde_json -p livecraft-core
   cargo remove ringbuf serde serde_json -p livecraft-dsp
   cargo remove serde -p livecraft-cli
   ```

   Expected: each prints `Removing ... from dependencies`.

2. Run `grep -n "ringbuf" Cargo.toml`. If it prints a line `ringbuf = "0.4"`, delete that line from `Cargo.toml`.

3. In `README.md`, find:

   ```text
   `ringbuf` 0.4 is declared by `livecraft-dsp` but not used yet. The plan's [Task 03](plans/livecraft-1.0/tasks/P0-03-remove-unused-dependencies.md) removes it.

   ```

   Replace it with nothing (delete those two lines).

## Do Not

- Change files other than: `Cargo.toml`, `Cargo.lock`, `crates/livecraft-core/Cargo.toml`, `crates/livecraft-dsp/Cargo.toml`, `crates/livecraft-cli/Cargo.toml`, `README.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not remove any dependency not in the table, even if you believe it is unused.

## Verify

| Command                                                 | Expected result |
| ------------------------------------------------------- | --------------- |
| `grep -rn "ringbuf" Cargo.toml crates`                  | no output       |
| `cargo fmt --all -- --check`                            | exit 0          |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0          |
| `cargo test --workspace`                                | 44 tests pass   |
| `cargo build --release --workspace`                     | exit 0          |

## Commit And Push

1. Tick P0-03 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add Cargo.toml Cargo.lock crates/livecraft-core/Cargo.toml crates/livecraft-dsp/Cargo.toml crates/livecraft-cli/Cargo.toml README.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Remove unused dependencies

   - Drop serde_json from livecraft-core
   - Drop ringbuf, serde, serde_json from livecraft-dsp and ringbuf from the workspace
   - Drop serde from livecraft-cli
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- A build error says a removed crate is needed: re-add only that crate with `cargo add <crate> -p <package>` using `--workspace`-style inheritance (`cargo add <crate>@<version from Cargo.toml> -p <package>`), and note it in the commit message. Rerun Verify. At most 3 attempts.
- Still failing after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: do not commit code. Append under Blockers in `CHECKLIST.md`: `P0-03, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
