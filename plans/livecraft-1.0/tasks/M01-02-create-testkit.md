# Task M01-02: Create livecraft-testkit crate

Depends on: M01-01. Size: 5 files, about 100 changed lines. Milestone: M01.

## Goal

Create `crates/livecraft-testkit` with `RenderSummary` and `render_project_summary`, add it to the workspace manifest and `xtask` LAYERS table, and refactor characterisation tests to use the shared helper.

## Before You Start

1. `git pull --ff-only`, then `git status --short`: empty.
2. In `plans/livecraft-1.0/CHECKLIST.md`, M01-01 is `[x]` and M01-02 is `[ ]`.
3. Run `cargo xtask ci`. Expected: `xtask: all checks passed`, 55 tests.

## Context

- `plans/livecraft-1.0/milestones/M01-workspace-restructure.md` lines 34-35: `livecraft-testkit` gets the first shared helper: `render_project_summary(project: &Project, bars: u32, sample_rate: u32) -> RenderSummary { rms_l, rms_r, peak_l, peak_r, frames }` used by characterisation tests.
- `xtask/src/main.rs` requires every package to have an entry in `LAYERS`.

## Steps

1. Create `crates/livecraft-testkit/Cargo.toml`:

   ```toml
   [package]
   name = "livecraft-testkit"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   license.workspace = true
   repository.workspace = true
   publish.workspace = true

   [dependencies]
   livecraft-core = { path = "../livecraft-core" }
   livecraft-dsp = { path = "../livecraft-dsp" }

   [lints]
   workspace = true
   ```

2. Create `crates/livecraft-testkit/src/lib.rs`:

   ```rust
   use livecraft_core::model::Project;
   use livecraft_dsp::graph::AudioGraph;

   #[derive(Debug, Clone, PartialEq)]
   pub struct RenderSummary {
       pub frames: usize,
       pub peak_l: f32,
       pub peak_r: f32,
       pub rms_l: f32,
       pub rms_r: f32,
   }

   pub fn render_project_summary(project: &Project, bars: u32, sample_rate: u32) -> RenderSummary {
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

       RenderSummary {
           frames: total_samples,
           peak_l,
           peak_r,
           rms_l,
           rms_r,
       }
   }
   ```

3. In `Cargo.toml`, under `[workspace.dependencies]`, add:

   ```toml
   livecraft-testkit = { path = "crates/livecraft-testkit" }
   ```

4. In `crates/livecraft-cli/Cargo.toml`, under `[dev-dependencies]`, add:

   ```toml
   livecraft-testkit.workspace = true
   ```

5. In `xtask/src/main.rs`, add `livecraft-testkit` to `LAYERS`:

   ```rust
       ("livecraft-testkit", &["livecraft-core", "livecraft-dsp"]),
   ```

6. In `crates/livecraft-cli/tests/characterisation.rs`, replace the local `compute_render_metrics` helper with `livecraft_testkit::render_project_summary`.

## Do Not

- Change any existing production behavior.
- Add unnecessary dependencies to `livecraft-testkit`.

## Verify

| Command                                               | Expected result                      |
| ----------------------------------------------------- | ------------------------------------ |
| `cargo test -p livecraft-cli --test characterisation` | 2 passed                             |
| `cargo xtask ci`                                      | `xtask: all checks passed`, 55 tests |

## Commit And Push

1. Tick M01-02 in `plans/livecraft-1.0/CHECKLIST.md`.
2. `git add crates/livecraft-testkit Cargo.toml Cargo.lock crates/livecraft-cli/Cargo.toml crates/livecraft-cli/tests/characterisation.rs xtask/src/main.rs plans/livecraft-1.0/CHECKLIST.md`
3. Commit:

   ```text
   Create livecraft-testkit with render summary helper

   - Add crates/livecraft-testkit with RenderSummary and render_project_summary
   - Add testkit to workspace dependencies and xtask LAYERS
   - Refactor characterisation tests to use livecraft-testkit
   ```

4. `git push origin main`, then `git status --short --branch`: clean, not ahead. Continue with the next line.

## If Something Fails

- Run the Unblock Procedure in `plans/livecraft-1.0/START-HERE.md`.
