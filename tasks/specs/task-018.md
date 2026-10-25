# Task Specification: task-018

## 1. Metadata

- **ID:** `task-018`
- **Phase:** Phase 4: Ableton Live 12 Desktop Interface Parity
- **Title:** Session View Clip Matrix & Track Width Resizing Dynamics
- **Dependencies:** `task-017`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 7 (Session View, pp. 186-192)

## 2. Context & Problem Statement

Ableton Live 12's Session View features a responsive clip grid where tracks can be resized in width (narrowing to icon-only mode or expanding for full name labels), with Alt+Drag resizing all tracks simultaneously (Manual p. 190). In `crates/livecraft-gui/src/session.rs`, track width is hardcoded to `96.0`, clip slots lack circular stop indicators on empty slots, and Master scene launch rows lack per-scene tempo and time signature LCDs.

## 3. Scope of Work

1. Implement interactive track width resizing in Session View:
   - Draggable right border on track headers.
   - Holding `Alt` while dragging resizes all tracks simultaneously.
   - Clamping width between minimum 36.0 px (compact mode) and 240.0 px.
2. Polish Session View clip slots:
   - Solid left color badge pill with clip name.
   - Green launch triangle button on hover/playback.
   - Empty clip slots display a circular stop button dot.
   - Animated loop progress bar filling across the clip background during playback.
   - Track stop buttons at the bottom of the slot matrix.
3. Polish Master Scene Column:
   - Green Scene Launch triangle for each row.
   - Per-scene tempo and time signature LCD displays.
   - "Stop All Clips" master button at the base of the scene column.

## 4. Files to Inspect

- `crates/livecraft-gui/src/session.rs`
- `crates/livecraft-gui/src/style.rs`

## 5. Files to Modify

- `crates/livecraft-gui/src/session.rs`

## 6. What NOT to Touch

- Do NOT touch `arrange.rs` or `pianoroll.rs`.
- Do NOT touch `livecraft-dsp/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/session.rs`:
   - Store `track_width: f32` in persistent egui memory (default 96.0).
   - In track header rendering, allocate a 4px wide drag handle on the right edge.
   - If dragged: update `track_width += drag_delta.x`. If `ui.input(|i| i.modifiers.alt)` is pressed, update all tracks.
   - When `track_width < 50.0`, enter compact mode: render only track number and launch triangle.
   - For empty slots: draw subtle dark well (`#222222`) with small circular stop dot (`#444444`, radius 2.5px).
   - For Master column: render Scene Launch triangle (`#38d855`), Scene Name, and optional Scene BPM.
2. Run `cargo test -p livecraft-gui`.

## 8. Verification Command

```bash
cargo test -p livecraft-gui
```

## 9. Acceptance Criteria

- [ ] Track width can be interactively resized via header dragging.
- [ ] `Alt + Drag` resizes all tracks in unison.
- [ ] Empty slots show authentic circular stop dots; filled slots show colored pills and launch triggers.
- [ ] Master scene launch buttons trigger scene clips accurately.

## 10. Failure Handling Protocol

If horizontal scroll jumps during drag:

- Use `egui::Sense::drag()` on dedicated handle rect rather than the entire header.
