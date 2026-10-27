# Task M01-01: Characterisation tests for render and inspection

Depends on: P0-18. Size: 3 files, about 100 changed lines. Milestone: M01.

## Goal

Pin existing offline render metrics (frames, peaks, RMS) and JSON project inspection with characterisation tests before moving any files, ensuring zero behavioral regression across the workspace restructure.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, P0-18 is `[x]` and M01-01 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 53 tests.

## Context

- `reference/DECISIONS.md#engineering` D20: testing crates include `insta 1.49.0 (text snapshots)`.
- Baseline 4-bar render at 120 BPM and 44.1 kHz is 352,800 frames. Empty demo project renders silence (peak 0.0, RMS 0.0). A demo with 3-note chords on track 1 (Operator) produces peak 0.26114187 and RMS 0.06759972.
- `crates/livecraft-cli/tests/cli_tests.rs` tests CLI commands; we add a dedicated `crates/livecraft-cli/tests/characterisation.rs` integration test suite.

## Steps

1. In `Cargo.toml`, under `[workspace.dependencies]`, add `insta = { version = "1.49.0", features = ["json"] }`.

2. In `crates/livecraft-cli/Cargo.toml`, add `[dev-dependencies]` section:

   ```toml
   [dev-dependencies]
   insta.workspace = true
   ```

3. Create `crates/livecraft-cli/tests/characterisation.rs`:

   ```rust
   use livecraft_core::model::{Clip, ClipPayload, MidiNote, Project};
   use livecraft_dsp::graph::AudioGraph;

   fn compute_render_metrics(project: &Project, bars: u32, sample_rate: u32) -> (usize, f32, f32, f32, f32) {
       let mut graph = AudioGraph::new(project, sample_rate as f32);
       graph.is_playing = true;
       for (trk_idx, trk) in project.tracks.iter().enumerate() {
           if let Some(slot) = trk.clip_slots.first()
               && slot.clip.is_some()
           {
               graph.trigger_clip(trk_idx, 0);
           }
       }
       let beats_total = (bars * project.transport.time_signature.0 as u32) as f64;
       let total_seconds = (beats_total / project.transport.bpm as f64) * 60.0;
       let total_samples = (total_seconds * sample_rate as f64) as usize;

       let block_size = 512;
       let mut buf_l = vec![0.0f32; block_size];
       let mut buf_r = vec![0.0f32; block_size];
       let mut rendered_samples = 0;

       let mut peak_l = 0.0f32;
       let mut peak_r = 0.0f32;
       let mut sum_sq_l = 0.0f64;
       let mut sum_sq_r = 0.0f64;

       while rendered_samples < total_samples {
           let to_render = block_size.min(total_samples - rendered_samples);
           let active_l = &mut buf_l[..to_render];
           let active_r = &mut buf_r[..to_render];

           graph.process_buffer(active_l, active_r);

           for i in 0..to_render {
               let s_l = active_l[i].abs();
               let s_r = active_r[i].abs();
               if s_l > peak_l {
                   peak_l = s_l;
               }
               if s_r > peak_r {
                   peak_r = s_r;
               }
               sum_sq_l += (active_l[i] as f64) * (active_l[i] as f64);
               sum_sq_r += (active_r[i] as f64) * (active_r[i] as f64);
           }

           rendered_samples += to_render;
       }

       let rms_l = if total_samples > 0 {
           (sum_sq_l / total_samples as f64).sqrt() as f32
       } else {
           0.0
       };
       let rms_r = if total_samples > 0 {
           (sum_sq_r / total_samples as f64).sqrt() as f32
       } else {
           0.0
       };

       (total_samples, peak_l, peak_r, rms_l, rms_r)
   }

   #[test]
   fn test_characterisation_render_metrics() {
       let project = Project::new_default("demo");
       let (frames, peak_l, peak_r, rms_l, rms_r) = compute_render_metrics(&project, 4, 44100);
       assert_eq!(frames, 352800);
       assert!((peak_l - 0.0).abs() < 1e-6);
       assert!((peak_r - 0.0).abs() < 1e-6);
       assert!((rms_l - 0.0).abs() < 1e-6);
       assert!((rms_r - 0.0).abs() < 1e-6);

       let mut chords_project = Project::new_default("ChordsDemo");
       let mut clip = Clip::new_midi(1, "Lead Chords", 4.0);
       if let ClipPayload::Midi(ref mut mdata) = clip.payload {
           mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
           mdata.notes.push(MidiNote::new(64, 0.0, 2.0, 100));
           mdata.notes.push(MidiNote::new(67, 0.0, 2.0, 100));
       }
       chords_project.tracks[0].clip_slots[0].clip = Some(clip);
       let (frames2, peak2_l, peak2_r, rms2_l, rms2_r) =
           compute_render_metrics(&chords_project, 4, 44100);
       assert_eq!(frames2, 352800);
       assert!((peak2_l - 0.26114187).abs() < 1e-6);
       assert!((peak2_r - 0.26114187).abs() < 1e-6);
       assert!((rms2_l - 0.06759972).abs() < 1e-6);
       assert!((rms2_r - 0.06759972).abs() < 1e-6);
   }

   #[test]
   fn test_characterisation_inspect_json() {
       let project = Project::new_default("demo");
       let val: serde_json::Value = serde_json::to_value(&project).expect("serialize to value");
       let json = serde_json::to_string_pretty(&val).expect("pretty print sorted json");
       insta::assert_snapshot!("demo_project_inspect", json);
   }
   ```

4. Run `cargo test -p livecraft-cli --test characterisation` with `INSTA_UPDATE=always` to create the initial snapshot file.

## Do Not

- Change existing tests or model defaults.
- Move any crates or source files in this task.

## Verify

| Command                                               | Expected result                      |
| ----------------------------------------------------- | ------------------------------------ |
| `cargo test -p livecraft-cli --test characterisation` | 2 passed                             |
| `cargo xtask ci`                                      | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-01 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add Cargo.toml Cargo.lock crates/livecraft-cli/Cargo.toml crates/livecraft-cli/tests/characterisation.rs crates/livecraft-cli/tests/snapshots plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Add characterisation tests for project render and inspection

   - Pin 4-bar render peak and RMS metrics for empty set and chords demo
   - Snapshot inspect --json output with insta 1.49.0
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- If `cargo-deny` warns on insta licenses, check `deny.toml` allowances.
- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
