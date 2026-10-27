# Task M01-05: Remove livecraft-core crate

Depends on: M01-04. Size: 5 to 8 files changed + deletion of `crates/livecraft-core`. Milestone: M01.

## Goal

Switch all workspace crates from `livecraft_core` to `livecraft_model` and `livecraft_formats`, delete the empty `crates/livecraft-core` crate, and remove `livecraft-core` from `Cargo.toml` and `xtask` LAYERS.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-04 is `[x]` and M01-05 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- Tasks M01-03 and M01-04 moved all code and tests out of `crates/livecraft-core` into `crates/livecraft-model` and `crates/livecraft-formats`.
- `plans/livecraft-1.0/tools/rewrite_imports.py` automates the import rewrite across the repository.

## Steps

1. Run the import rewrite tool:

   ```bash
   python plans/livecraft-1.0/tools/rewrite_imports.py --replace "livecraft_core::model" "livecraft_model::model" .
   python plans/livecraft-1.0/tools/rewrite_imports.py --replace "livecraft_core::command" "livecraft_model::command" .
   python plans/livecraft-1.0/tools/rewrite_imports.py --replace "livecraft_core::als" "livecraft_formats::als" .
   python plans/livecraft-1.0/tools/rewrite_imports.py --replace "livecraft_core::theme" "livecraft_formats::theme" .
   ```

2. Update crate manifests:
   - In `crates/livecraft-dsp/Cargo.toml`: replace `livecraft-core.workspace = true` with `livecraft-model.workspace = true`.
   - In `crates/livecraft-gui/Cargo.toml`: replace `livecraft-core.workspace = true` with `livecraft-model.workspace = true` and `livecraft-formats.workspace = true`.
   - In `crates/livecraft-cli/Cargo.toml`: replace `livecraft-core.workspace = true` with `livecraft-model.workspace = true` and `livecraft-formats.workspace = true`.
   - In `crates/livecraft-testkit/Cargo.toml`: replace `livecraft-core.workspace = true` with `livecraft-model.workspace = true`.
   - In root `Cargo.toml`: remove `livecraft-core` from `[workspace.dependencies]`.

3. Remove `crates/livecraft-core`:

   ```bash
   git rm -r crates/livecraft-core
   ```

4. In `xtask/src/main.rs`, update `LAYERS`:
   - Remove `livecraft-core`.
   - Update `livecraft-dsp`: `&["livecraft-model"]`.
   - Update `livecraft-gui`: `&["livecraft-model", "livecraft-formats", "livecraft-dsp"]`.
   - Update `livecraft-cli`: `&["livecraft-model", "livecraft-formats", "livecraft-dsp", "livecraft-gui"]`.
   - Update `livecraft-testkit`: `&["livecraft-model", "livecraft-dsp"]`.

5. Check for any remaining references to `livecraft_core` across the workspace:

   ```bash
   git grep "livecraft_core" -- "*.rs"
   ```

   Fix any occurrences found.

## Do Not

- Touch device or engine code in this task (handled in M01-06 and M01-07).
- Break compilation in any workspace crate.

## Verify

| Command                               | Expected result                      |
| ------------------------------------- | ------------------------------------ |
| `git grep "livecraft_core" -- "*.rs"` | no output                            |
| `cargo xtask ci`                      | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-05 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add -u` and `git add Cargo.toml Cargo.lock xtask/src/main.rs plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Remove livecraft-core and switch imports to model and formats

   - Rewrite imports to livecraft_model and livecraft_formats
   - Update dependencies across dsp, gui, cli, and testkit
   - Delete crates/livecraft-core and remove from xtask LAYERS
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
