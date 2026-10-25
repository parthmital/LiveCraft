# Task Specification: task-021

## 1. Metadata

- **ID:** `task-021`
- **Phase:** Phase 4: Ableton Live 12 Desktop Interface Parity
- **Title:** Full 128-Note Piano Roll with Zoom/Scroll & Scale/Fold Mode
- **Dependencies:** `task-020`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 10 (Editing MIDI, pp. 251-290)

## 2. Context & Problem Statement

In `crates/livecraft-gui/src/pianoroll.rs:20-22`, the piano roll is hardcoded to only 36 pitches (C3 48 to B5 83)! Notes outside this narrow 3-octave window cannot be seen or edited, and vertical scrolling is completely missing. Furthermore, note drawing (`✎` Draw Mode), velocity lane editing, and note selection handles are unpolished.

## 3. Scope of Work

1. Expand Piano Roll to support the complete standard 128-note MIDI range (0 to 127):
   - Vertical scrolling and zooming via mouse wheel / trackpad.
   - Piano keyboard on the left margin rendering white and black keys with note pitch labels (`C-1` to `G9`, with bold `C3`, `C4`, `C5` octave labels).
   - Fold to Scale mode: when enabled, hides non-scale keys and expands in-scale rows to fill view.
   - Scale key highlighting: subtle colored tinting on rows belonging to the active scale.
2. Note creation and editing:
   - Double-click to insert note; double-click or right-click to delete note.
   - Drag note to change pitch and start beat.
   - Drag right edge of note to change note duration.
   - Draw mode (`✎` toggle or 'B' key): click/drag inserts notes quantized to grid.
3. Velocity lane at the bottom of the Piano Roll:
   - Vertical velocity stems with circular handles (1 to 127).
   - Draggable handle height adjusting selected note velocities.

## 4. Files to Inspect

- `crates/livecraft-gui/src/pianoroll.rs`
- `crates/livecraft-core/src/model.rs` (MidiNote, ScaleConfig)

## 5. Files to Modify

- `crates/livecraft-gui/src/pianoroll.rs`

## 6. What NOT to Touch

- Do NOT touch `session.rs` or `mixer.rs`.
- Do NOT touch `livecraft-dsp/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/pianoroll.rs`:
   - Store scroll offset in egui state: `scroll_pitch_offset: f32` and `zoom_y: f32`.
   - Wrap piano roll in an `egui::ScrollArea::both()`.
   - Calculate visible pitch range based on scroll position and height.
   - If Fold to Scale is active, filter `visible_pitches = (0..=127).rev().filter(|&p| scale.is_in_scale(p)).collect()`.
   - Render piano keyboard keys on left with sharp/flat contrast and octave labels.
   - Render grid lines: beat lines (solid) and 16th-note lines (dashed/subtle).
   - Add velocity lane below piano roll with vertical fader stems.
2. Run `cargo test -p livecraft-gui`.

## 8. Verification Command

```bash
cargo test -p livecraft-gui
```

## 9. Acceptance Criteria

- [ ] Piano roll spans all 128 MIDI notes with fluid vertical scrolling.
- [ ] Fold to Scale mode and Scale highlighting function accurately across all 12 musical scales.
- [ ] Double-click creates notes, edge-drag resizes notes, and velocity lane edits velocity stems.

## 10. Failure Handling Protocol

If notes render outside clip boundaries:

- Clamp note start times to `0.0 .. clip.length_beats`.
