# Task M01-06: Create livecraft-devices crate

Depends on: M01-05. Size: 5 files changed + file moves. Milestone: M01.

## Goal

Create `crates/livecraft-devices`, move effects, instruments, and `processor.rs` into it via `git mv`, move device tests, re-export from `livecraft-dsp` to keep engine and GUI compiling, and register `livecraft-devices` in workspace dependencies and `LAYERS`.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-05 is `[x]` and M01-06 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- `plans/livecraft-1.0/milestones/M01-workspace-restructure.md` File destinations:
  - Effects: `autofilter.rs`, `autopan.rs`, `chorus.rs`, `compressor.rs`, `delay.rs`, `eq3.rs`, `eq8.rs`, `gate.rs`, `limiter.rs`, `redux.rs`, `reverb.rs`, `saturator.rs` -> `crates/livecraft-devices/src/effects/`
  - Instruments: `operator.rs`, `simpler.rs`, `drumrack.rs` -> `crates/livecraft-devices/src/instruments/`
  - Processor: `processor.rs` -> `crates/livecraft-devices/src/processor.rs`
- `livecraft-devices` depends on `livecraft-model` and `livecraft-dsp` (for `BiquadFilter`).

## Steps

1. Create `crates/livecraft-devices/Cargo.toml`:

   ```toml
   [package]
   name = "livecraft-devices"
   version.workspace = true
   edition.workspace = true
   license.workspace = true
   repository.workspace = true

   [dependencies]
   livecraft-model.workspace = true
   livecraft-dsp.workspace = true
   log.workspace = true
   ```

2. In root `Cargo.toml`, under `[workspace.dependencies]`, add:

   ```toml
   livecraft-devices = { path = "crates/livecraft-devices" }
   ```

3. Move files using git:

   ```bash
   mkdir -p crates/livecraft-devices/src/effects
   mkdir -p crates/livecraft-devices/src/instruments
   git mv crates/livecraft-dsp/src/autofilter.rs crates/livecraft-devices/src/effects/autofilter.rs
   git mv crates/livecraft-dsp/src/autopan.rs crates/livecraft-devices/src/effects/autopan.rs
   git mv crates/livecraft-dsp/src/chorus.rs crates/livecraft-devices/src/effects/chorus.rs
   git mv crates/livecraft-dsp/src/compressor.rs crates/livecraft-devices/src/effects/compressor.rs
   git mv crates/livecraft-dsp/src/delay.rs crates/livecraft-devices/src/effects/delay.rs
   git mv crates/livecraft-dsp/src/eq3.rs crates/livecraft-devices/src/effects/eq3.rs
   git mv crates/livecraft-dsp/src/eq8.rs crates/livecraft-devices/src/effects/eq8.rs
   git mv crates/livecraft-dsp/src/gate.rs crates/livecraft-devices/src/effects/gate.rs
   git mv crates/livecraft-dsp/src/limiter.rs crates/livecraft-devices/src/effects/limiter.rs
   git mv crates/livecraft-dsp/src/redux.rs crates/livecraft-devices/src/effects/redux.rs
   git mv crates/livecraft-dsp/src/reverb.rs crates/livecraft-devices/src/effects/reverb.rs
   git mv crates/livecraft-dsp/src/saturator.rs crates/livecraft-devices/src/effects/saturator.rs
   git mv crates/livecraft-dsp/src/operator.rs crates/livecraft-devices/src/instruments/operator.rs
   git mv crates/livecraft-dsp/src/simpler.rs crates/livecraft-devices/src/instruments/simpler.rs
   git mv crates/livecraft-dsp/src/drumrack.rs crates/livecraft-devices/src/instruments/drumrack.rs
   git mv crates/livecraft-dsp/src/processor.rs crates/livecraft-devices/src/processor.rs
   ```

4. Create `crates/livecraft-devices/src/effects/mod.rs` and `crates/livecraft-devices/src/instruments/mod.rs` re-exporting the modules.

5. Create `crates/livecraft-devices/src/lib.rs`:

   ```rust
   pub mod effects;
   pub mod instruments;
   pub mod processor;

   #[cfg(test)]
   mod tests;
   ```

6. In device files, update `crate::biquad::BiquadFilter` to `livecraft_dsp::biquad::BiquadFilter`.

7. Move device tests from `crates/livecraft-dsp/src/tests.rs` into `crates/livecraft-devices/src/tests.rs`.

8. In `crates/livecraft-dsp/Cargo.toml`, add `livecraft-devices.workspace = true`.

9. In `crates/livecraft-dsp/src/lib.rs`, re-export the moved device modules so existing engine code compiles:

   ```rust
   pub use livecraft_devices::{effects, instruments, processor};
   ```

10. In `xtask/src/main.rs`, update `LAYERS`:
    - Add `("livecraft-devices", &["livecraft-model", "livecraft-dsp"]),`
    - Update `livecraft-dsp` to allow `livecraft-devices`: `("livecraft-dsp", &["livecraft-model", "livecraft-devices"]),`

## Do Not

- Move engine or graph modules in this task (Task M01-07 does that).
- Change DSP algorithms or parameter curves.

## Verify

| Command                           | Expected result                      |
| --------------------------------- | ------------------------------------ |
| `cargo test -p livecraft-devices` | tests pass                           |
| `cargo xtask ci`                  | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-06 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-devices Cargo.toml Cargo.lock crates/livecraft-dsp xtask/src/main.rs plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Create livecraft-devices and move effects, instruments, processor

   - Move audio effects, instruments, and DspProcessor to crates/livecraft-devices
   - Move device tests to livecraft-devices
   - Re-export from livecraft-dsp and update xtask LAYERS
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
