# Task P0-04: Add workspace lints

Depends on: P0-03. Size: 6 files, about 25 changed lines. Milestone: P0.

## Goal

Every crate forbids `unsafe` code and denies `dbg!`, `todo!`, and `unimplemented!`, configured once in the workspace.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-03 is ticked and P0-04 is not.
3. Run `cargo test --workspace`. Expected: 44 tests pass.

## Context

- On 2026-10-27 the workspace has no `unsafe`, `dbg!`, `todo!`, or `unimplemented!` (searched with grep), so these lints pass without code changes.
- Future crates that must use `unsafe` (plug-in hosting in M37) will override with a crate-level `[lints]` table and a reason, recorded in `SECURITY.md`.

## Steps

1. In `Cargo.toml`, find:

   ```toml
   [workspace.dependencies]
   ```

   Replace it with:

   ```toml
   [workspace.lints.rust]
   unsafe_code = "forbid"

   [workspace.lints.clippy]
   dbg_macro = "deny"
   todo = "deny"
   unimplemented = "deny"

   [workspace.dependencies]
   ```

2. In each of `crates/livecraft-core/Cargo.toml`, `crates/livecraft-dsp/Cargo.toml`, `crates/livecraft-gui/Cargo.toml`, `crates/livecraft-cli/Cargo.toml`, add these lines at the very end of the file (after a blank line):

   ```toml
   [lints]
   workspace = true
   ```

3. In `SECURITY.md`, find:

   ```text
   - Rust with no `unsafe` code in the workspace (checked on 2026-10-27).
   ```

   Replace it with:

   ```text
   - `unsafe_code = "forbid"` for every crate through `[workspace.lints]` in `Cargo.toml`.
   ```

## Do Not

- Change files other than: `Cargo.toml`, the four crate `Cargo.toml` files, `SECURITY.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not add any other lint; stricter lints come with the crates that satisfy them.

## Verify

| Command                                                 | Expected result            |
| ------------------------------------------------------- | -------------------------- |
| `grep -c "workspace = true" crates/*/Cargo.toml`        | each file shows at least 1 |
| `cargo fmt --all -- --check`                            | exit 0                     |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0                     |
| `cargo test --workspace`                                | 44 tests pass              |
| `cargo build --release --workspace`                     | exit 0                     |

## Commit And Push

1. Tick P0-04 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add Cargo.toml crates/livecraft-core/Cargo.toml crates/livecraft-dsp/Cargo.toml crates/livecraft-gui/Cargo.toml crates/livecraft-cli/Cargo.toml SECURITY.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Add workspace lints

   - Forbid unsafe code in every crate
   - Deny dbg!, todo!, and unimplemented! through clippy
   - Record the control in SECURITY.md
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead.

## If Something Fails

- A lint fires: log a blocker (the code was expected clean); do not add `allow` attributes.
- Other Verify failures: fix within the listed files, at most 3 attempts.
- Still failing: do not commit code. Append under Blockers in `CHECKLIST.md`: `P0-04, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, stop.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, log a blocker, stop.
