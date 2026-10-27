# Task M01-04: Create livecraft-formats crate

Depends on: M01-03. Size: 5 files changed + file moves. Milestone: M01.

## Goal

Create `crates/livecraft-formats`, move `als.rs` and `theme.rs` into it via `git mv`, move format unit tests, re-export from `livecraft-core` for compatibility, and register `livecraft-formats` in workspace dependencies and `LAYERS`.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-03 is `[x]` and M01-04 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- `plans/livecraft-1.0/milestones/M01-workspace-restructure.md` File destinations:
  `crates/livecraft-core/src/als.rs`, `theme.rs` -> `crates/livecraft-formats/src/`.
- `livecraft-formats` depends only on `livecraft-model` among workspace crates.
- Dependencies needed from workspace: `quick-xml`, `flate2`, `thiserror`.

## Steps

1. Create `crates/livecraft-formats/Cargo.toml`:

   ```toml
   [package]
   name = "livecraft-formats"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   license.workspace = true
   repository.workspace = true
   publish.workspace = true

   [dependencies]
   livecraft-model = { path = "../livecraft-model" }
   quick-xml.workspace = true
   flate2.workspace = true
   thiserror.workspace = true
   serde.workspace = true
   anyhow.workspace = true

   [lints]
   workspace = true
   ```

2. In root `Cargo.toml`, under `[workspace.dependencies]`, add:

   ```toml
   livecraft-formats = { path = "crates/livecraft-formats" }
   ```

3. Move files using git:

   ```bash
   mkdir -p crates/livecraft-formats/src
   git mv crates/livecraft-core/src/als.rs crates/livecraft-formats/src/als.rs
   git mv crates/livecraft-core/src/theme.rs crates/livecraft-formats/src/theme.rs
   ```

4. Create `crates/livecraft-formats/src/lib.rs`:

   ```rust
   pub mod als;
   pub mod theme;

   #[cfg(test)]
   mod tests;
   ```

5. In `crates/livecraft-formats/src/als.rs` and `theme.rs`, update imports that referenced `crate::model` to `livecraft_model::model`.

6. Move remaining tests from `crates/livecraft-core/src/tests.rs` into `crates/livecraft-formats/src/tests.rs` (testing `test_als_decompression_rejects_oversized_output`, `test_als_xml_export_and_import`, `test_als_full_roundtrip_with_devices_and_clips`, `test_parse_real_ableton_template`, `test_theme_colors`, `test_theme_hex_variants`).

7. In `crates/livecraft-core/Cargo.toml`, add `livecraft-formats.workspace = true`.

8. In `crates/livecraft-core/src/lib.rs`, replace module declarations of `als` and `theme` with:

   ```rust
   pub use livecraft_formats::als;
   pub use livecraft_formats::theme;
   ```

9. In `xtask/src/main.rs`, update `LAYERS`:
   - Add `("livecraft-formats", &["livecraft-model"]),`
   - Update `livecraft-core`: `("livecraft-core", &["livecraft-model", "livecraft-formats"]),`

## Do Not

- Remove `livecraft-core` in this task (Task M01-05 does that).
- Change parsing algorithms or public API.

## Verify

| Command                           | Expected result                      |
| --------------------------------- | ------------------------------------ |
| `cargo test -p livecraft-formats` | tests pass                           |
| `cargo xtask ci`                  | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-04 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-formats Cargo.toml Cargo.lock crates/livecraft-core xtask/src/main.rs plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Create livecraft-formats and move als and theme modules

   - Move als.rs and theme.rs to crates/livecraft-formats
   - Move ALS and theme tests to livecraft-formats
   - Re-export from livecraft-core and update xtask LAYERS
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
