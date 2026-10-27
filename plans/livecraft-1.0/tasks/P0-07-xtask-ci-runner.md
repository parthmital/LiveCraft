# Task P0-07: Add `cargo xtask ci`, the layering check, and the `cargo dev` launcher

Depends on: P0-06. Size: 7 files (3 new), about 290 lines (239 copied verbatim). Milestone: P0.

## Goal

One command, `cargo xtask ci`, runs format check, clippy, tests, release build, a crate layering check, and a Markdown link check; `cargo dev` launches the app.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-06 is ticked and P0-07 is not.
3. Run `cargo test --workspace`. Expected: 45 tests pass.

## Context

- Decision D11: one-command check is `cargo xtask ci`, launcher is `cargo dev`; no Node.js.
- The xtask source is ready and was validated on 2026-10-27 (fmt clean, clippy clean, 4 unit tests pass, `layering` passes, `docs` reports only links to files that did not exist yet): `plans/livecraft-1.0/files/P0-07/main.rs`, SHA-256 `169f11c067549c9ff6b9e2471fde7ced87724b7d8b07641b81611d6277c646b7`.
- What it does: `layering` reads `cargo metadata --no-deps` and fails if a package's normal dependency on another workspace package is not listed in its `LAYERS` table; `docs` lists tracked and untracked non-ignored `*.md` files and fails on relative links whose target file is missing (URLs, `mailto:`, and `#anchor` links are skipped; fenced code blocks are skipped).
- The `CARGO` environment variable is set by `cargo run`, so xtask calls the same cargo.

## Steps

1. Create `xtask/Cargo.toml` with exactly:

   ```toml
   [package]
   name = "xtask"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   license.workspace = true
   repository.workspace = true
   publish.workspace = true

   [dependencies]
   serde_json.workspace = true

   [lints]
   workspace = true
   ```

2. Create `xtask/src/main.rs` by copying the validated file:

   ```bash
   mkdir -p xtask/src
   cp plans/livecraft-1.0/files/P0-07/main.rs xtask/src/main.rs
   sha256sum xtask/src/main.rs
   ```

   Expected hash: `169f11c067549c9ff6b9e2471fde7ced87724b7d8b07641b81611d6277c646b7`. If it differs, recopy; do not edit the file.

3. Create `.cargo/config.toml` with exactly:

   ```toml
   [alias]
   xtask = "run --quiet --package xtask --"
   dev = "run --bin livecraft --"
   ```

4. In the root `Cargo.toml`, find:

   ```toml
       "crates/livecraft-cli",
   ]
   ```

   Replace it with:

   ```toml
       "crates/livecraft-cli",
       "xtask",
   ]
   ```

5. Run `cargo xtask layering`. Expected: `xtask: layering check passed`.

6. Run `cargo xtask docs`. Expected: `xtask: docs check passed`. If it lists broken links, fix each link in the Markdown file it names so it points to the existing file (do not delete content). The only files you may change for this are `.md` files.

7. In `AGENTS.md`, find:

   ```text
   Run format check, lint, test, and release build before every commit. Baseline on 2026-10-27: all clean, 44 tests pass.
   ```

   Replace it with:

   ```text
   - All checks in one command: `cargo xtask ci` (format check, clippy, tests, release build, crate layering, Markdown links). Run it before every commit.
   - Launch the app: `cargo dev` (GUI); `cargo dev -- <subcommand>` for the CLI, for example `cargo dev -- info demo.als`.
   - New crate: add it to `LAYERS` in `xtask/src/main.rs` with its allowed workspace dependencies, or `cargo xtask layering` fails.
   ```

8. In `README.md`, find:

   ```text
   The CI workflow ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs these on Windows and Ubuntu. Run them locally before every commit:
   ```

   Replace it with:

   ```text
   `cargo xtask ci` runs all of these plus a crate layering check and a Markdown link check; run it before every commit. `cargo dev` launches the GUI. The CI workflow ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs the same checks. The individual commands are:
   ```

9. In `ARCHITECTURE.md`, find:

   ```text
   - Not enforced by tooling yet. The plan adds a layering check to `cargo xtask ci`.
   ```

   Replace it with:

   ```text
   - Enforced by `cargo xtask layering` (part of `cargo xtask ci`): the `LAYERS` table in `xtask/src/main.rs` lists each package's allowed workspace dependencies.
   ```

   And find:

   ```text
   | `crates/livecraft-cli/src/main.rs`              | `clap` subcommands, GUI launcher                                            | `livecraft` binary                                           | every crate                  |
   ```

   Replace it with:

   ```text
   | `crates/livecraft-cli/src/main.rs`              | `clap` subcommands, GUI launcher                                            | `livecraft` binary                                           | every crate                  |
   | `xtask/src/main.rs`                             | `cargo xtask ci`, layering check, docs link check                           | `cargo xtask <ci,layering,docs>`                             | none                         |
   ```

## Do Not

- Change files other than: `xtask/Cargo.toml`, `xtask/src/main.rs`, `.cargo/config.toml`, `Cargo.toml`, `Cargo.lock`, `AGENTS.md`, `README.md`, `ARCHITECTURE.md`, other `.md` files only for step 6, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not edit `xtask/src/main.rs` after copying it.

## Verify

| Command                  | Expected result                                                                                                |
| ------------------------ | -------------------------------------------------------------------------------------------------------------- |
| `cargo xtask ci`         | ends with `xtask: all checks passed`; tests: 49 pass (45 before plus 4 xtask)                                  |
| `cargo xtask nonsense`   | prints the xtask usage line (`xtask: usage: cargo xtask` followed by the three subcommands) and exits non-zero |
| `cargo dev -- --version` | prints `livecraft 0.1.0`                                                                                       |

## Commit And Push

1. Tick P0-07 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add xtask/Cargo.toml xtask/src/main.rs .cargo/config.toml Cargo.toml Cargo.lock AGENTS.md README.md ARCHITECTURE.md plans/livecraft-1.0/CHECKLIST.md` plus any `.md` file fixed in step 6.
3. Commit:

   ```text
   Add cargo xtask ci with layering and docs checks

   - Add the xtask crate: ci runs fmt, clippy, tests, release build, layering, docs
   - Enforce allowed workspace dependencies per crate from a LAYERS table
   - Check that relative Markdown links resolve
   - Add the cargo dev launcher alias and document both commands
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead.

## If Something Fails

- `cargo xtask ci` fails in a step that passed before this task: read the whole error; fix only within the listed files; at most 3 attempts.
- Still failing: do not commit code. Append under Blockers in `CHECKLIST.md`: `P0-07, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, stop.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, log a blocker, stop.
