# Task M01-08: Rename livecraft-gui to livecraft-ui

Depends on: M01-07. Size: 5 files changed + directory move. Milestone: M01.

## Goal

Rename `crates/livecraft-gui` to `crates/livecraft-ui` using `git mv`, update crate name and imports across the workspace to `livecraft_ui`, and update `Cargo.toml` and `xtask` LAYERS.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-07 is `[x]` and M01-08 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- `reference/TARGET-ARCHITECTURE.md`: egui desktop frontend lives in `crates/livecraft-ui`.
- Package name becomes `livecraft-ui`, library target `livecraft_ui`.

## Steps

1. Rename directory using git:

   ```bash
   git mv crates/livecraft-gui crates/livecraft-ui
   ```

2. In `crates/livecraft-ui/Cargo.toml`, change `name`:

   ```toml
   [package]
   name = "livecraft-ui"
   ```

3. In root `Cargo.toml`, under `[workspace.dependencies]`, replace `livecraft-gui` with:

   ```toml
   livecraft-ui = { path = "crates/livecraft-ui" }
   ```

4. In `crates/livecraft-cli/Cargo.toml`, replace `livecraft-gui.workspace = true` with:

   ```toml
   livecraft-ui.workspace = true
   ```

5. In `crates/livecraft-cli/src/main.rs`, replace occurrences of `livecraft_gui` with `livecraft_ui`.

6. In `xtask/src/main.rs`, update `LAYERS`:
   - Replace `("livecraft-gui", ...)` with `("livecraft-ui", &["livecraft-model", "livecraft-formats", "livecraft-engine"]),`
   - Update `livecraft-cli`'s dependencies: `&["livecraft-model", "livecraft-formats", "livecraft-engine", "livecraft-devices", "livecraft-ui"]`

## Do Not

- Change UI rendering code or widget implementations.
- Introduce new dependencies to UI.

## Verify

| Command                              | Expected result                      |
| ------------------------------------ | ------------------------------------ |
| `git grep "livecraft_gui" -- "*.rs"` | no output                            |
| `cargo test -p livecraft-ui`         | 2 passed                             |
| `cargo xtask ci`                     | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-08 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-ui crates/livecraft-cli Cargo.toml Cargo.lock xtask/src/main.rs plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Rename livecraft-gui to livecraft-ui

   - Move crates/livecraft-gui to crates/livecraft-ui
   - Rename package to livecraft-ui and update import paths
   - Update xtask LAYERS with livecraft-ui
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
