# Task Specification: task-020

## 1. Metadata

- **ID:** `task-020`
- **Phase:** Phase 4: Ableton Live 12 Desktop Interface Parity
- **Title:** Arrangement View Interactive Timeline & Real Audio Waveform Rendering
- **Dependencies:** `task-019`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 6 (Arrangement View, pp. 164-185)

## 2. Context & Problem Statement

In `crates/livecraft-gui/src/arrange.rs:401-418`, audio clip waveforms are rendered using a fake mathematical sine formula `((i as f32 * 0.4).sin().abs() * 8.0) + 2.0` instead of actual audio file sample data! Furthermore, arrangement clips cannot be dragged, trimmed, split (`Ctrl+E`), duplicated (`Ctrl+D`), or selected.

## 3. Scope of Work

1. Replace fake sine wave drawing with real audio waveform rendering:
   - Calculate min/max peak bins from actual audio samples in `AudioClipData`.
   - Render positive and negative amplitude envelopes mirrored across the center line.
   - Display audio clip transient markers and zero-crossing line.
2. Interactive clip manipulation:
   - Drag clip header to move clip horizontally across timeline beats or vertically across tracks.
   - Drag left or right edge to trim clip start or end boundaries.
   - Split clip at playhead position (`Ctrl+E`).
   - Duplicate clip (`Ctrl+D`).
   - Delete selected clip (`Delete` / `Backspace`).
3. Top Time Ruler:
   - Scrubbing playhead marker: clicking on the ruler sets playback position immediately.
   - Draggable loop brace flags with start/end markers.

## 4. Files to Inspect

- `crates/livecraft-gui/src/arrange.rs`
- `crates/livecraft-core/src/model.rs` (ArrangementClip)

## 5. Files to Modify

- `crates/livecraft-gui/src/arrange.rs`

## 6. What NOT to Touch

- Do NOT touch `session.rs` or `mixer.rs`.
- Do NOT touch `livecraft-dsp/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/arrange.rs`:
   - For audio clips:
     - Access sample buffer from `trk.clip_slots` or sample cache.
     - Subdivide the clip rectangle width into pixel bins (e.g. 1 bin per 2 pixels).
     - Compute `min_val` and `max_val` of audio samples falling within each bin's time span.
     - Draw vertical line from `mid_y - max_val * half_h` to `mid_y - min_val * half_h`.
     - Fill waveform body with semi-transparent clip color.
   - For clip manipulation:
     - Wrap each clip rectangle with `egui::Sense::click_and_drag()`.
     - When dragged from header: update `clip.start_beat += (drag_delta.x / pixels_per_beat) as f64`.
     - Snap start beat to grid (1 beat or 1/4 beat depending on zoom).
     - Handle `Ctrl+E`: call `clip.split_at(playhead_beats, next_id)` from `task-009`.
     - Handle `Ctrl+D`: duplicate selected clip at `start_beat + length_beats`.
2. Run `cargo test -p livecraft-gui`.

## 8. Verification Command

```bash
cargo test -p livecraft-gui
```

## 9. Acceptance Criteria

- [ ] Fake sine wave rendering is completely removed; actual audio waveform contours are rendered.
- [ ] Timeline clips can be dragged, trimmed, duplicated (`Ctrl+D`), and split (`Ctrl+E`).
- [ ] Ruler click scrubs playhead position.

## 10. Failure Handling Protocol

If waveform rendering is slow on long samples:

- Compute an overview peak pyramid (cache min/max peak pairs per 64 and 512 samples) so rendering is instant.
