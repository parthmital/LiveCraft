# Task P0-01: Pin the toolchain and share package metadata

Depends on: nothing. Size: 8 files, about 60 changed lines. Milestone: P0.

## Goal

Every build uses Rust 1.99.1 with rustfmt and clippy, every crate inherits version, edition, licence, repository, MSRV, and `publish = false` from the workspace, line endings are LF in all checkouts, and `/.tools/` is ignored.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, no line is ticked yet.
3. Run `cargo test --workspace`. Expected: 44 tests pass (cli 4, core 13, dsp 25, gui 2).

## Context

- The workspace root is `Cargo.toml`; crates are `crates/livecraft-core`, `crates/livecraft-dsp`, `crates/livecraft-gui`, `crates/livecraft-cli`. Each crate manifest starts with `version = "0.1.0"` and `edition = "2021"` repeated.
- Decision D10: Rust 1.99.1 pinned, MSRV 1.95. Rustup will download the 1.99.1 toolchain once if only `stable` is installed; that is expected.
- `publish = false` is required later by `cargo-deny` to allow path dependencies (Task P0-12).
- `.gitattributes` today contains only `* text=auto`. All tracked files are already LF (`git ls-files --eol`), so adding `eol=lf` changes no content.
- `.gitignore` starts with a stray title line `# LiveCraft - Native Rust Implementation of Ableton Live`.

## Steps

1. Create `rust-toolchain.toml` at the repository root with exactly:

   ```toml
   [toolchain]
   channel = "1.99.1"
   components = ["rustfmt", "clippy"]
   profile = "minimal"
   ```

2. In `Cargo.toml`, find:

   ```toml
   [workspace.dependencies]
   ```

   Replace it with:

   ```toml
   [workspace.package]
   version = "0.1.0"
   edition = "2021"
   rust-version = "1.95"
   license = "MIT OR Apache-2.0"
   repository = "https://github.com/parthmital/LiveCraft"
   publish = false

   [workspace.dependencies]
   ```

3. In each of these four files: `crates/livecraft-core/Cargo.toml`, `crates/livecraft-dsp/Cargo.toml`, `crates/livecraft-gui/Cargo.toml`, `crates/livecraft-cli/Cargo.toml`, find:

   ```toml
   version = "0.1.0"
   edition = "2021"
   ```

   Replace it with:

   ```toml
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   license.workspace = true
   repository.workspace = true
   publish.workspace = true
   ```

4. Replace the whole content of `.gitattributes` with:

   ```text
   # Normalise text to LF in the repository and in every checkout.
   * text=auto eol=lf

   # Binary assets
   *.png binary
   *.wav binary
   *.als binary
   *.pdf binary
   ```

5. In `.gitignore`, find:

   ```text
   # LiveCraft - Native Rust Implementation of Ableton Live

   /target/
   ```

   Replace it with:

   ```text
   # Build output
   /target/

   # Repo-local tool installs (cargo-deny and other CLI tools, see AGENTS.md)
   /.tools/
   ```

6. Run `git add --renormalize .` then `git status --short`. Expected: only the files changed in steps 1 to 5 appear (and nothing else; if other files appear, run `git restore --staged <file>` for each and `git restore <file>`).

7. Run `rustup show active-toolchain`. Expected: the line starts with `1.99.1-` and ends with `(overridden by '...rust-toolchain.toml')`.

8. In `AGENTS.md`, find:

   ```text
   - Release build: `cargo build --release --workspace`
   ```

   Replace it with:

   ```text
   - Release build: `cargo build --release --workspace`
   - Toolchain: pinned to Rust 1.99.1 by `rust-toolchain.toml`; crate metadata comes from `[workspace.package]` in `Cargo.toml`.
   ```

## Do Not

- Change files other than: `rust-toolchain.toml`, `Cargo.toml`, the four crate `Cargo.toml` files, `.gitattributes`, `.gitignore`, `AGENTS.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not change the edition (Task P0-08 does) or any dependency version.

## Verify

| Command                                                                                                   | Expected result       |
| --------------------------------------------------------------------------------------------------------- | --------------------- |
| `rustup show active-toolchain`                                                                            | starts with `1.99.1-` |
| `cargo metadata --no-deps --format-version 1` piped to `grep -o '"license":"MIT OR Apache-2.0"' \| wc -l` | `4`                   |
| `cargo fmt --all -- --check`                                                                              | no output, exit 0     |
| `cargo clippy --workspace --all-targets -- -D warnings`                                                   | exit 0                |
| `cargo test --workspace`                                                                                  | 44 tests pass, 0 fail |
| `cargo build --release --workspace`                                                                       | exit 0                |

## Commit And Push

1. Tick P0-01 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add rust-toolchain.toml Cargo.toml crates/livecraft-core/Cargo.toml crates/livecraft-dsp/Cargo.toml crates/livecraft-gui/Cargo.toml crates/livecraft-cli/Cargo.toml .gitattributes .gitignore AGENTS.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Pin toolchain and share package metadata

   - Add rust-toolchain.toml pinning Rust 1.99.1 with rustfmt and clippy
   - Move version, edition, MSRV, licence, repository, publish into workspace.package
   - Force LF line endings and mark binary assets in .gitattributes
   - Ignore the repo-local /.tools/ directory
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead.

## If Something Fails

- `rustup` cannot download 1.99.1 (offline): log a blocker; do not change the channel.
- A Verify command fails: read the whole error, fix it within the listed files, rerun every Verify command. At most 3 attempts.
- Still failing: do not commit code. Append under Blockers in `CHECKLIST.md`: `P0-01, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, stop.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, log a blocker, stop.
