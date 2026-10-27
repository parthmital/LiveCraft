# Task P0-09: Upgrade cpal to 0.18 and thiserror to 2

Depends on: P0-08. Size: 4 files, about 15 changed lines. Milestone: P0.

## Goal

The audio engine builds on `cpal` 0.18.2 and errors derive with `thiserror` 2, with unchanged behaviour.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-08 is `[x]` or `[~]` and P0-09 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 49 tests.

## Context

- `RESEARCH.md#audio-io-2026-10-27`: cpal 0.18.2 is current; the repo pins 0.15. `thiserror` 2.0.21 is current; the repo pins 1.0 (decision D21).
- Trial on 2026-10-27: with cpal 0.18 the only errors are in `crates/livecraft-dsp/src/engine.rs`: `SupportedStreamConfig::sample_rate()` now returns a plain `u32` (no `.0`), and `build_output_stream` takes `StreamConfig` by value (`StreamConfig` is `Copy`, so clippy rejects `.clone()`). `thiserror` 2 needed no code changes.
- Current code in `engine.rs` (`AudioEngine::new`):

  ```rust
          let sample_rate = supported_config.sample_rate().0 as f32;
  ```

  and three calls of the form:

  ```rust
              SampleFormat::F32 => device.build_output_stream(
                  &config,
  ```

## Steps

1. In `Cargo.toml` `[workspace.dependencies]`, replace `thiserror = "1.0"` with `thiserror = "2.0.21"` and `cpal = "0.15"` with `cpal = "0.18.2"`.
2. In `crates/livecraft-dsp/src/engine.rs`, find `let sample_rate = supported_config.sample_rate().0 as f32;` and replace it with `let sample_rate = supported_config.sample_rate() as f32;`.
3. In the same file, replace each of the three lines that consist of `&config,` (one in each `build_output_stream(` call) with `config,`. Run `grep -c "^ *&config,$" crates/livecraft-dsp/src/engine.rs` before (expected `3`) and after (expected `0`).
4. In `README.md`, in the Tech Stack table, change the `cpal` version cell from `0.15` to `0.18` and the `anyhow`, `thiserror` row's version cell from `1.0` to `1.0, 2` (anyhow stays 1.0).

## Do Not

- Change files other than: `Cargo.toml`, `Cargo.lock`, `crates/livecraft-dsp/src/engine.rs`, `README.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not enable cpal's `asio` feature (decision D14).

## Verify

| Command                                                                        | Expected result                                                                                                                                                           |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cargo tree -i cpal --depth 0`                                                 | `cpal v0.18.2` (or a later 0.18 patch)                                                                                                                                    |
| `cargo xtask ci`                                                               | `xtask: all checks passed`, 49 tests                                                                                                                                      |
| `cargo dev -- synth -n 60 -w saw -d 0.5 -o .agent-local/output/p0-09/tone.wav` | exit 0; the WAV file exists                                                                                                                                               |
| `cargo dev -- play --duration 2`                                               | exit 0 after about 2 seconds with meters printed (needs an audio output device; if the machine has none, record "play not verified: no output device" in the commit body) |

## Commit And Push

1. Tick P0-09 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add Cargo.toml Cargo.lock crates/livecraft-dsp/src/engine.rs README.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Upgrade cpal to 0.18 and thiserror to 2

   - Read the sample rate as a plain u32 and pass StreamConfig by value
   - Bump thiserror to 2 with no code changes
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Other compile errors from cpal: read the cpal 0.18 docs with `cargo doc -p cpal --no-deps --open` or the error text, fix only in `engine.rs`, at most 3 attempts.
- Still failing after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: do not commit code. Append under Blockers in `CHECKLIST.md`: `P0-09, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
