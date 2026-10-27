# Task M01-03: Create livecraft-model crate

Depends on: M01-02. Size: 5 files changed + file moves. Milestone: M01.

## Goal

Create `crates/livecraft-model`, move `model.rs` and `command.rs` into it via `git mv`, move model unit tests, re-export them from `livecraft-core` to keep existing call sites working, and register `livecraft-model` in workspace dependencies and `LAYERS`.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-02 is `[x]` and M01-03 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- `plans/livecraft-1.0/milestones/M01-workspace-restructure.md` File destinations:
  `crates/livecraft-core/src/model.rs`, `command.rs` -> `crates/livecraft-model/src/`.
- `livecraft-model` has no internal workspace dependencies.
- Re-exporting from `livecraft-core` maintains full workspace compilation until Task M01-05 removes `livecraft-core`.

## Steps

1. Create `crates/livecraft-model/Cargo.toml`:

   ```toml
   [package]
   name = "livecraft-model"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   license.workspace = true
   repository.workspace = true
   publish.workspace = true

   [dependencies]
   serde.workspace = true

   [lints]
   workspace = true
   ```

2. In root `Cargo.toml`, under `[workspace.dependencies]`, add:

   ```toml
   livecraft-model = { path = "crates/livecraft-model" }
   ```

3. Move files using git:

   ```bash
   mkdir -p crates/livecraft-model/src
   git mv crates/livecraft-core/src/model.rs crates/livecraft-model/src/model.rs
   git mv crates/livecraft-core/src/command.rs crates/livecraft-model/src/command.rs
   ```

4. Create `crates/livecraft-model/src/lib.rs`:

   ```rust
   pub mod command;
   pub mod model;

   #[cfg(test)]
   mod tests;
   ```

5. In `crates/livecraft-model/src/command.rs`, update imports to use `crate::model::*` instead of `crate::model::*`.

6. Extract model tests from `crates/livecraft-core/src/tests.rs` into `crates/livecraft-model/src/tests.rs` (testing `clip_slot_and_payload_manipulation`, `scale_system_and_snapping`, `midi_transformations`, `note_name_follows_live_convention`, `command_undo_redo`, `device_defaults`, `project_creation`, `command_history_capacity`, `tuning_system`). Leave `.als` and theme tests in `crates/livecraft-core/src/tests.rs`.

7. In `crates/livecraft-core/Cargo.toml`, add `livecraft-model.workspace = true`.

8. In `crates/livecraft-core/src/lib.rs`, replace module declarations of `command` and `model` with:

   ```rust
   pub use livecraft_model::command;
   pub use livecraft_model::model;
   ```

9. In `xtask/src/main.rs`, update `LAYERS`:
   - Add `("livecraft-model", &[]),`
   - Update `livecraft-core` to allow `livecraft-model`: `("livecraft-core", &["livecraft-model"]),`

## Do Not

- Remove `livecraft-core` yet (Task M01-05 does that).
- Change public types or function signatures.

## Verify

| Command | Expected result |
| --- | --- |
| `cargo test -p livecraft-model` | tests pass |
| `cargo xtask ci` | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-03 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-model Cargo.toml Cargo.lock crates/livecraft-core xtask/src/main.rs plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Create livecraft-model and move model and command modules

   - Move model.rs and command.rs to crates/livecraft-model
   - Move model unit tests to livecraft-model
   - Re-export from livecraft-core and update xtask LAYERS
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
