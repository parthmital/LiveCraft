# Task Specification: task-009

## 1. Metadata

- **ID:** `task-009`
- **Phase:** Phase 2: Core Data Model & Authentic Ableton 12 Compatibility
- **Title:** Arrangement Clip Non-Destructive Editing (Split, Duplicate, Trim)
- **Dependencies:** `task-008`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 6 (Arrangement View: Clip Editing)

## 2. Context & Problem Statement

Currently, `ArrangementClip` is a simple wrapper `(id, clip, start_beat, length_beats)` with no editing methods. In Ableton Live, users split clips at the playhead (`Ctrl+E`), duplicate clips (`Ctrl+D`), trim clip start and end boundaries, and consolidate clips (`Ctrl+J`). These operations must be implemented in the domain model before they can be bound to GUI and keyboard shortcuts.

## 3. Scope of Work

1. Implement non-destructive timeline editing operations in `livecraft-core::model`:
   - `split_at(split_beat: f64) -> Option<(ArrangementClip, ArrangementClip)>`: Slices a clip into two independent parts with offset markers.
   - `duplicate(offset_beats: f64) -> ArrangementClip`: Copies the clip placed at a musical offset.
   - `trim_start(delta_beats: f64)`: Adjusts clip start beat and internal start marker non-destructively.
   - `trim_end(delta_beats: f64)`: Adjusts clip length.
2. Ensure MIDI notes within sliced clips are partitioned or truncated cleanly at the split point without note data loss.

## 4. Files to Inspect

- `crates/livecraft-core/src/model.rs` (lines 440-460)

## 5. Files to Modify

- `crates/livecraft-core/src/model.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/` (GUI interactive dragging is in `task-020`).
- Do NOT touch audio DSP engine.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-core/src/model.rs`:
   - Extend `ArrangementClip`:
     ```rust
     impl ArrangementClip {
         pub fn split_at(&self, split_beat: f64, new_id: u64) -> Option<(Self, Self)> {
             if split_beat <= self.start_beat || split_beat >= self.start_beat + self.length_beats {
                 return None;
             }
             let first_len = split_beat - self.start_beat;
             let second_len = self.length_beats - first_len;

             let mut first = self.clone();
             first.length_beats = first_len;

             let mut second = self.clone();
             second.id = new_id;
             second.start_beat = split_beat;
             second.length_beats = second_len;

             // Adjust internal markers & payload
             match (&mut first.clip.payload, &mut second.clip.payload) {
                 (ClipPayload::Midi(ref mut m1), ClipPayload::Midi(ref mut m2)) => {
                     let rel_split = first_len;
                     m1.notes.retain(|n| n.start_beat < rel_split);
                     for n in &mut m1.notes {
                         if n.start_beat + n.duration_beats > rel_split {
                             n.duration_beats = rel_split - n.start_beat;
                         }
                     }
                     m2.notes.retain(|n| n.start_beat + n.duration_beats > rel_split);
                     for n in &mut m2.notes {
                         if n.start_beat < rel_split {
                             let cut = rel_split - n.start_beat;
                             n.duration_beats -= cut;
                             n.start_beat = 0.0;
                         } else {
                             n.start_beat -= rel_split;
                         }
                     }
                 }
                 _ => {}
             }
             Some((first, second))
         }
     }
     ```
2. Write unit tests for splitting MIDI clips and trimming clip boundaries.

## 8. Verification Command

```bash
cargo test -p livecraft-core --lib tests::test_clip_slot_and_payload_manipulation
```

## 9. Acceptance Criteria

- [ ] Splitting a clip at a beat boundary produces two valid contiguous clips.
- [ ] Notes spanning the split point are divided appropriately without pitch alteration.
- [ ] Trimming boundaries preserves internal note timing.

## 10. Failure Handling Protocol

If note durations become negative:

- Ensure `.max(0.01)` clamping on `n.duration_beats`.
