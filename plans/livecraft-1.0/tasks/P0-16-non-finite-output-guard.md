# Task P0-16: Never send NaN or infinity to the driver or a render (finding LC-004)

Depends on: P0-15. Size: 4 files, about 35 changed lines. Milestone: P0.

## Goal

Non-finite samples (NaN, infinity) from any device become silence at the final output and in offline renders; finite samples pass through bit-exactly.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-15 is `[x]` or `[~]` and P0-16 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 52 tests.

## Context

- Finding LC-004: an unstable filter or a division by zero produces NaN or infinity, which some drivers turn into full-scale noise (hearing and speaker risk). The f32 output path in `AudioGraph::process_buffer_interleaved` (`crates/livecraft-dsp/src/graph.rs`) writes samples unchecked. The `i16` and `u16` paths clamp and cast, and Rust's float-to-int casts map NaN to 0, so they are already safe. The offline bouncer (`crates/livecraft-dsp/src/bouncer.rs`) clamps with `clamp(-1.0, 1.0)`, which keeps NaN as NaN in 32-bit float files.
- Neutrality (manual chapter 39): finite values must pass unchanged, so the guard only replaces non-finite values and never limits finite ones. (The bouncer's existing clamp of 32-bit float output is a separate parity issue for milestone M08.)
- Current f32 output code in `graph.rs`:

  ```rust
              let l = scratch_l[frame_idx];
              let r = scratch_r[frame_idx];
              if channels >= 2 {
                  chunk[0] = l;
  ```

- Current bouncer code:

  ```rust
                  let s_l = active_l[i].clamp(-1.0, 1.0);
                  let s_r = active_r[i].clamp(-1.0, 1.0);
  ```

## Steps

1. Test first. Append to the end of `crates/livecraft-dsp/src/tests.rs` (the body of the crate's test module, with `use super::*;` at its top):

   ```rust

   #[test]
   fn test_sanitize_output_sample() {
       assert_eq!(sanitize_output_sample(f32::NAN), 0.0);
       assert_eq!(sanitize_output_sample(f32::INFINITY), 0.0);
       assert_eq!(sanitize_output_sample(f32::NEG_INFINITY), 0.0);
       for value in [0.0_f32, -0.0, 0.5, -1.0, 3.75, f32::MIN_POSITIVE, f32::MAX] {
           assert_eq!(sanitize_output_sample(value).to_bits(), value.to_bits());
       }
   }
   ```

2. In `crates/livecraft-dsp/src/graph.rs`, find:

   ```rust
   #[derive(Debug, Clone, Default)]
   pub struct TrackMeter {
   ```

   Replace it with:

   ```rust
   /// Replaces NaN and infinity with silence; finite samples pass through bit-exactly.
   #[inline]
   pub fn sanitize_output_sample(sample: f32) -> f32 {
       if sample.is_finite() { sample } else { 0.0 }
   }

   #[derive(Debug, Clone, Default)]
   pub struct TrackMeter {
   ```

3. In the same file, find:

   ```rust
               let l = scratch_l[frame_idx];
               let r = scratch_r[frame_idx];
               if channels >= 2 {
                   chunk[0] = l;
   ```

   Replace it with:

   ```rust
               let l = sanitize_output_sample(scratch_l[frame_idx]);
               let r = sanitize_output_sample(scratch_r[frame_idx]);
               if channels >= 2 {
                   chunk[0] = l;
   ```

4. In `crates/livecraft-dsp/src/bouncer.rs`, find:

   ```rust
                   let s_l = active_l[i].clamp(-1.0, 1.0);
                   let s_r = active_r[i].clamp(-1.0, 1.0);
   ```

   Replace it with:

   ```rust
                   let s_l = crate::graph::sanitize_output_sample(active_l[i]).clamp(-1.0, 1.0);
                   let s_r = crate::graph::sanitize_output_sample(active_r[i]).clamp(-1.0, 1.0);
   ```

5. Run `cargo fmt --all`.

6. In `SECURITY.md`, change the LC-004 row: Status `Fixed`, Location `` `sanitize_output_sample` in `crates/livecraft-dsp/src/graph.rs` ``, Fix `Non-finite samples become 0.0 in the f32 driver path and in renders; test test_sanitize_output_sample (Task P0-16)`. In the Controls list, replace `- Planned in \`plans/livecraft-1.0/\` Phase 0: input size limits and a non-finite output guard.`with`- Input size limits for \`.als\` (512 MiB XML) and WAV (2^28 samples), and a non-finite output guard on the driver and render paths.`

## Do Not

- Change files other than: `crates/livecraft-dsp/src/graph.rs`, `crates/livecraft-dsp/src/bouncer.rs`, `crates/livecraft-dsp/src/tests.rs`, `SECURITY.md`, `plans/livecraft-1.0/CHECKLIST.md`.
- Do not clamp or limit finite samples in the driver path.

## Verify

| Command                                                                                             | Expected result                      |
| --------------------------------------------------------------------------------------------------- | ------------------------------------ |
| `cargo test -p livecraft-dsp test_sanitize_output_sample`                                           | 1 passed                             |
| `cargo xtask ci`                                                                                    | `xtask: all checks passed`, 53 tests |
| `cargo dev -- render .agent-local/output/p0-11/demo.als -o .agent-local/output/p0-16/demo.wav -d 2` | exit 0, WAV written                  |

## Commit And Push

1. Tick P0-16 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-dsp/src/graph.rs crates/livecraft-dsp/src/bouncer.rs crates/livecraft-dsp/src/tests.rs SECURITY.md plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Replace non-finite output samples with silence

   - Add sanitize_output_sample and apply it to the f32 driver path and renders
   - Test that finite samples pass bit-exactly
   - Mark finding LC-004 fixed
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Fix within the listed files, at most 3 attempts. Still failing after the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`: append under Blockers in `CHECKLIST.md`: `P0-16, <date>: <what failed>`, the command, and the last 20 lines of output. Commit only `CHECKLIST.md`, push, end the run.
- `git push` rejected: `git pull --rebase`, rerun Verify, push. If the rebase conflicts: `git rebase --abort`, then the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
