# Task M01-07: Create livecraft-engine crate and trim livecraft-dsp

Depends on: M01-06. Size: 6 files changed + file moves. Milestone: M01.

## Goal

Create `crates/livecraft-engine`, move `engine.rs`, `graph.rs`, `bouncer.rs`, `audio_clip.rs` and integration tests into it via `git mv`, reduce `crates/livecraft-dsp` to pure DSP primitives (`biquad.rs`), update call sites to use `livecraft-engine`, and update `xtask` LAYERS.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-06 is `[x]` and M01-07 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- `reference/TARGET-ARCHITECTURE.md`:
  - `livecraft-dsp`: pure DSP primitives, depends on nothing internal.
  - `livecraft-engine`: graph compiler, executor, transport, PDC, metering, I/O backends, offline renderer; depends on model, dsp, devices, formats.
- Moves `wav_limits.rs` integration test from `livecraft-dsp` to `livecraft-engine`.

## Steps

1. Create `crates/livecraft-engine/Cargo.toml`:

   ```toml
   [package]
   name = "livecraft-engine"
   version.workspace = true
   edition.workspace = true
   license.workspace = true
   repository.workspace = true

   [dependencies]
   livecraft-model.workspace = true
   livecraft-dsp.workspace = true
   livecraft-devices.workspace = true
   livecraft-formats.workspace = true
   cpal.workspace = true
   hound.workspace = true
   crossbeam-channel.workspace = true
   anyhow.workspace = true
   log.workspace = true
   ```

2. In root `Cargo.toml`, under `[workspace.dependencies]`, add:

   ```toml
   livecraft-engine = { path = "crates/livecraft-engine" }
   ```

3. Move files using git:

   ```bash
   mkdir -p crates/livecraft-engine/src
   mkdir -p crates/livecraft-engine/tests
   git mv crates/livecraft-dsp/src/engine.rs crates/livecraft-engine/src/engine.rs
   git mv crates/livecraft-dsp/src/graph.rs crates/livecraft-engine/src/graph.rs
   git mv crates/livecraft-dsp/src/bouncer.rs crates/livecraft-engine/src/bouncer.rs
   git mv crates/livecraft-dsp/src/audio_clip.rs crates/livecraft-engine/src/audio_clip.rs
   git mv crates/livecraft-dsp/tests/wav_limits.rs crates/livecraft-engine/tests/wav_limits.rs
   ```

4. Create `crates/livecraft-engine/src/lib.rs`:

   ```rust
   pub mod audio_clip;
   pub mod bouncer;
   pub mod engine;
   pub mod graph;

   #[cfg(test)]
   mod tests;
   ```

5. Move remaining engine/graph/bouncer tests from `crates/livecraft-dsp/src/tests.rs` into `crates/livecraft-engine/src/tests.rs`.

6. Reduce `crates/livecraft-dsp`:
   - In `crates/livecraft-dsp/src/lib.rs`, keep only `pub mod biquad;` and `#[cfg(test)] mod tests;` (testing `test_biquad_all_filter_types`).
   - In `crates/livecraft-dsp/Cargo.toml`, remove `livecraft-model`, `livecraft-devices`, `cpal`, `hound`, `crossbeam-channel`, `anyhow`.

7. Update callers in `crates/livecraft-gui`, `crates/livecraft-cli`, and `crates/livecraft-testkit` to import `livecraft_engine` (and `livecraft_devices` where instruments/effects are referenced), and update their `Cargo.toml` dependencies.

8. In `xtask/src/main.rs`, update `LAYERS`:
   - `("livecraft-dsp", &[]),`
   - `("livecraft-engine", &["livecraft-model", "livecraft-dsp", "livecraft-devices", "livecraft-formats"]),`
   - Update `livecraft-gui`: `&["livecraft-model", "livecraft-formats", "livecraft-engine"]`.
   - Update `livecraft-cli`: `&["livecraft-model", "livecraft-formats", "livecraft-engine", "livecraft-devices", "livecraft-gui"]`.
   - Update `livecraft-testkit`: `&["livecraft-model", "livecraft-engine"]`.

## Do Not

- Change audio processing logic, channel layout, or buffer handling.
- Break zero-allocation rules on audio processing paths.

## Verify

| Command                          | Expected result                      |
| -------------------------------- | ------------------------------------ |
| `cargo test -p livecraft-dsp`    | 1 passed                             |
| `cargo test -p livecraft-engine` | tests pass                           |
| `cargo xtask ci`                 | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-07 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-engine crates/livecraft-dsp crates/livecraft-gui crates/livecraft-cli crates/livecraft-testkit Cargo.toml Cargo.lock xtask/src/main.rs plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Create livecraft-engine and trim livecraft-dsp to primitives

   - Move engine, graph, bouncer, audio_clip to crates/livecraft-engine
   - Reduce livecraft-dsp to biquad filter primitive with no internal dependencies
   - Update GUI, CLI, and testkit to use livecraft-engine and update xtask LAYERS
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
