# Task M01-09: Move CLI to apps/livecraft

Depends on: M01-08. Size: 4 files changed + directory move. Milestone: M01.

## Goal

Move `crates/livecraft-cli` to `apps/livecraft` using `git mv`, rename the package to `livecraft`, update workspace members in root `Cargo.toml`, update `xtask` LAYERS, and verify `cargo dev` and `cargo xtask ci`.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-08 is `[x]` and M01-09 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- `reference/TARGET-ARCHITECTURE.md`: user-facing binary app lives in `apps/livecraft` (package `livecraft`, binary `livecraft`).
- Root `Cargo.toml` workspace members must include `"apps/*"`.

## Steps

1. Create `apps` directory and move `crates/livecraft-cli`:

   ```bash
   mkdir -p apps
   git mv crates/livecraft-cli apps/livecraft
   ```

2. In `Cargo.toml`:
   - Under `[workspace]`, update `members`:

     ```toml
     members = [
         "apps/*",
         "crates/*",
         "xtask",
     ]
     ```

   - Under `[workspace.dependencies]`, remove `livecraft-cli`.

3. In `apps/livecraft/Cargo.toml`, change `name`:

   ```toml
   [package]
   name = "livecraft"
   ```

4. In `xtask/src/main.rs`, update `LAYERS`:
   - Replace `("livecraft-cli", ...)` with:

     ```rust
         (
             "livecraft",
             &[
                 "livecraft-model",
                 "livecraft-formats",
                 "livecraft-engine",
                 "livecraft-devices",
                 "livecraft-ui",
             ],
         ),
     ```

5. Verify `cargo dev -- --version` prints `livecraft 0.1.0`.

## Do Not

- Change CLI command definitions, flags, or output formats.
- Rename binary `livecraft`.

## Verify

| Command | Expected result |
| --- | --- |
| `git grep "livecraft-cli" -- "*.toml" "*.rs"` | no output |
| `cargo run --bin livecraft -- --version` | `livecraft 0.1.0` |
| `cargo xtask ci` | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-09 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add apps/livecraft Cargo.toml Cargo.lock xtask/src/main.rs plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Move CLI app to apps/livecraft

   - Move crates/livecraft-cli to apps/livecraft with package name livecraft
   - Add apps/* to workspace members
   - Update xtask LAYERS with livecraft package
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
