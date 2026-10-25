# Task Specification: task-003

## 1. Metadata

- **ID:** `task-003`
- **Phase:** Phase 1: Real-Time Audio Engine & DSP Architecture Hardening
- **Title:** Sample-Accurate Voice Allocation & Polyphonic Note Lifecycle
- **Dependencies:** `task-002`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 40 (MIDI Fact Sheet: Sample-Accurate Scheduling)

## 2. Context & Problem Statement

In `crates/livecraft-dsp/src/graph.rs:837-869`, `process_clip_payload` uses a simple `active_notes: [bool; 128]` array. If two notes share the same pitch (e.g. repeated 16th notes or overlapping chords), `note_off` from the first note instantly kills the second note, causing stuck notes or abrupt truncation. Furthermore, `OperatorSynth::note_on` completely discards note velocity (`_velocity: u8`), ignoring note dynamics entirely.

## 3. Scope of Work

1. Replace `active_notes: [bool; 128]` with an explicit `ActiveNoteTracker` that tracks note instances by `(pitch, note_id, voice_idx)`.
2. Connect velocity scaling to `OperatorSynth` and `SimplerInstrument` so that note velocity (1–127) modulates initial gain and filter envelope amount.
3. Implement voice stealing in `OperatorSynth` (prioritizing: free voices -> releasing voices -> oldest voice) without clicks (quick fade-out envelope).

## 4. Files to Inspect

- `crates/livecraft-dsp/src/graph.rs` (lines 75-110 and 830-870)
- `crates/livecraft-dsp/src/operator.rs` (lines 40-75 and 140-180)

## 5. Files to Modify

- `crates/livecraft-dsp/src/graph.rs`
- `crates/livecraft-dsp/src/operator.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-core/src/model.rs`.
- Do NOT touch audio clip playback logic (covered in `task-004`).

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/operator.rs`:
   - In `Voice`:
     - Add `pub velocity: f32` (normalized 0.0 to 1.0).
     - Add `pub note_id: u64`.
   - In `note_on(note: u8, velocity: u8)`:
     - Remove the underscore from `velocity`. Store `let norm_vel = (velocity as f32 / 127.0).clamp(0.01, 1.0)`.
     - Modulate voice output amplitude by `norm_vel`.
2. In `crates/livecraft-dsp/src/graph.rs`:
   - Replace `active_notes: [bool; 128]` with:
     ```rust
     #[derive(Debug, Clone)]
     pub struct ActiveNoteInstance {
         pub pitch: u8,
         pub note_index: usize,
         pub start_beat: f64,
         pub duration_beats: f64,
     }
     ```
   - In `dispatch_clip_events`, detect note onsets within the current audio sub-block window (`clip_pos .. clip_pos + dt_beats * block_len`).
   - Trigger `note_on` when `clip_pos` crosses note onset; trigger `note_off` when `clip_pos` crosses note offset.
3. Write unit tests for overlapping notes of identical pitch to verify that subsequent note onsets re-trigger correctly.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_operator_synth_note
```

## 9. Acceptance Criteria

- [ ] Note velocity is preserved and affects synthesis loudness.
- [ ] Overlapping notes with identical pitch do not cancel each other out prematurely.
- [ ] Voices in `Operator` are allocated dynamically with smooth voice stealing.

## 10. Failure Handling Protocol

If polyphony tests fail:

- Check that voice release stage has non-zero decay time so notes ring out naturally rather than cutting instantly to 0.
