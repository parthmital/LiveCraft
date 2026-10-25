# Task Specification: task-022

## 1. Metadata

- **ID:** `task-022`
- **Phase:** Phase 4: Ableton Live 12 Desktop Interface Parity
- **Title:** Live 12 MIDI Transformations & Generative Tools Inspector Panel
- **Dependencies:** `task-021`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 11 (MIDI Tools: Transformations and Generators, pp. 293-329)

## 2. Context & Problem Statement

Ableton Live 12 introduces dedicated MIDI Transformation and Generative tool panels in the Clip View. While `livecraft-core::model::MidiClipData` implements the transformation algorithms (Quantize, Transpose, Chop, Strum, Humanize, Ornament, Span, Invert, Reverse, Time Warp, Velocity Ramp, Diatonic Stacks, Euclidean), in `crates/livecraft-gui/src/pianoroll.rs` they are placed in a crowded horizontal button row with no parameter adjustment controls (e.g. strum direction, chop subdivisions, humanize depth, Euclidean pulse count).

## 3. Scope of Work

1. Build a tabbed Ableton Live 12 MIDI Tools Inspector side-panel in Clip View:
   - **Transformations Tab:**
     - **Quantize:** Grid chooser (1/4, 1/8, 1/16, 1/32) and Amount slider (0-100%).
     - **Chop:** Subdivisions slider (1 to 64 parts) and Apply button.
     - **Strum:** Spread slider and Direction toggle (Up / Down).
     - **Humanize:** Timing jitter and Velocity variation sliders.
     - **Span:** Legato extension button.
     - **Invert & Reverse:** Pitch inversion and time reversal buttons.
     - **Velocity Ramp:** Start and End velocity boxes.
   - **Generators Tab:**
     - **Stacks:** Chord degree selector, Triad/7th/9th type, and Generate button.
     - **Euclidean:** Pulses slider (1 to 16), Steps slider (1 to 32), and Root pitch selector.
2. Connect controls so clicking Apply transforms the selected notes in the active clip with undo history tracking.

## 4. Files to Inspect

- `crates/livecraft-gui/src/pianoroll.rs`
- `crates/livecraft-core/src/model.rs` (lines 65-220 and 310-335)

## 5. Files to Modify

- `crates/livecraft-gui/src/pianoroll.rs`

## 6. What NOT to Touch

- Do NOT touch `session.rs` or `arrange.rs`.
- Do NOT touch `livecraft-dsp/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/pianoroll.rs`:
   - Add a collapsible left inspector panel within Clip View (width ~180px).
   - Add tab switcher: `[ Transformations | Generators ]`.
   - Implement controls for each tool with LCD parameter wells and "Apply" buttons.
   - When an Apply button is clicked:
     - Execute the corresponding method on `midi_data`.
     - Push undo step onto `command_history`.
2. Run `cargo test -p livecraft-gui`.

## 8. Verification Command

```bash
cargo test -p livecraft-gui
```

## 9. Acceptance Criteria

- [ ] Tabbed inspector provides parameter controls for all 10 transformations and 2 generative tools.
- [ ] Applying transformations modifies notes in real-time and registers with undo/redo.
- [ ] UI matches Ableton Live 12 compact typography and layout.

## 10. Failure Handling Protocol

If notes array is empty when applying transformation:

- Return early with a status bar notification: "No notes selected to transform".
