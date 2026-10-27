# Task P0-02: Configure logging without `set_var` and replace `eprintln!` in libraries

Depends on: P0-01. Size: 4 files, about 25 changed lines. Milestone: P0.

## Goal

The CLI configures `env_logger` through its builder (no `std::env::set_var`, which becomes `unsafe` in Rust 2024), and library crates report errors through the `log` facade instead of `eprintln!`.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-01 is `[x]` or `[~]` and P0-02 is `[ ]`.
3. Run `cargo test --workspace`. Expected: 44 tests pass.

## Context

- `crates/livecraft-cli/src/main.rs` lines 186 to 189 today:

  ```rust
      if args.verbose {
          std::env::set_var("RUST_LOG", "debug");
      }
      let _ = env_logger::try_init();
  ```

  Behaviour to keep: without `-v`, `RUST_LOG` controls the level and the default is `error`; with `-v`, the level is `debug` regardless of `RUST_LOG`.

- Decision D22: libraries use the `log` facade; the binary initialises `env_logger`. `log` is already a dependency of `livecraft-dsp` and `livecraft-gui`.
- `eprintln!` in library code: `crates/livecraft-dsp/src/engine.rs` (2 places) and `crates/livecraft-gui/src/app.rs` (2 places). CLI `println!` and `eprintln!` in `main.rs` are user-facing output and stay.

## Steps

1. In `crates/livecraft-cli/src/main.rs`, find:

   ```rust
       if args.verbose {
           std::env::set_var("RUST_LOG", "debug");
       }
       let _ = env_logger::try_init();
   ```

   Replace it with:

   ```rust
       let mut logger =
           env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error"));
       if args.verbose {
           logger.filter_level(log::LevelFilter::Debug);
       }
       let _ = logger.try_init();
   ```

2. In `crates/livecraft-dsp/src/engine.rs`, find:

   ```rust
           let err_fn = |err| eprintln!("Audio stream error: {}", err);
   ```

   Replace it with:

   ```rust
           let err_fn = |err| log::error!("Audio stream error: {err}");
   ```

3. In the same file, find:

   ```rust
               eprintln!("Warning: Failed to send audio parameter update: {}", e);
   ```

   Replace it with:

   ```rust
               log::warn!("Failed to send audio parameter update: {e}");
   ```

4. In `crates/livecraft-gui/src/app.rs`, find:

   ```rust
                   eprintln!("Screenshot timeout");
   ```

   Replace it with:

   ```rust
                   log::error!("Screenshot timeout");
   ```

5. In the same file, find:

   ```rust
                               eprintln!("Failed to save screenshot: {}", e);
   ```

   Replace it with:

   ```rust
                               log::error!("Failed to save screenshot: {e}");
   ```

6. In `AGENTS.md`, find:

   ```text
   - Errors: library code returns `Result` with `thiserror` enums or `anyhow::Result`; no `unwrap()` or `expect()` outside tests.
   ```

   Replace it with:

   ```text
   - Errors: library code returns `Result` with `thiserror` enums or `anyhow::Result`; no `unwrap()` or `expect()` outside tests.
   - Logging: library crates use `log::error!`, `log::warn!`, `log::info!`, `log::debug!`; never `eprintln!` or `println!`. Only `crates/livecraft-cli` prints to the terminal. Never log on the audio thread.
   ```

7. Run `grep -rn "eprintln!\|set_var" crates/livecraft-core/src crates/livecraft-dsp/src crates/livecraft-gui/src`. Expected: no output.

## Do Not

- Change files other than: `crates/livecraft-cli/src/main.rs`, `crates/livecraft-dsp/src/engine.rs`, `crates/livecraft-gui/src/app.rs`, `AGENTS.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not change any other `println!` or `eprintln!` in `main.rs`.

## Verify

| Command                                                                              | Expected result                                 |
| ------------------------------------------------------------------------------------ | ----------------------------------------------- |
| `cargo fmt --all -- --check`                                                         | exit 0                                          |
| `cargo clippy --workspace --all-targets -- -D warnings`                              | exit 0                                          |
| `cargo test --workspace`                                                             | 44 tests pass                                   |
| `cargo build --release --workspace`                                                  | exit 0                                          |
| `grep -rn "set_var" crates`                                                          | no output                                       |
| `cargo run --bin livecraft -- -v info crates/livecraft-cli/tests/does-not-exist.als` | exits non-zero with an error message (no panic) |

## Commit And Push

1. Tick P0-02 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-cli/src/main.rs crates/livecraft-dsp/src/engine.rs crates/livecraft-gui/src/app.rs AGENTS.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Configure logging without set_var and use log in libraries

   - Build env_logger from RUST_LOG with an error default and -v forcing debug
   - Replace eprintln! in the audio engine and GUI with log macros
   - Document the logging rule in AGENTS.md
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- A Verify command fails: read the whole error, fix it within the listed files, rerun every Verify command. At most 3 attempts.
- Still failing after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: do not commit code. Append under Blockers in `CHECKLIST.md`: `P0-02, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
