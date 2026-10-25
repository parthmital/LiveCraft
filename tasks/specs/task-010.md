# Task Specification: task-010

## 1. Metadata

- **ID:** `task-010`
- **Phase:** Phase 2: Core Data Model & Authentic Ableton 12 Compatibility
- **Title:** Scala .scl Microtonal Engine Integration Across Synthesizers
- **Dependencies:** `task-009`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 15 (Using Tuning Systems)

## 2. Context & Problem Statement

Ableton Live 12 introduces native microtonal tuning systems using Scala `.scl` files and reference frequency calibrations (e.g. A3 = 440 Hz). While `livecraft-core::model::TuningSystem` parses Scala text, its calculated frequencies are never passed to the synthesizer voices in `livecraft-dsp::operator` or `simpler`. Synthesizers still compute frequency using standard 12-TET math: `440.0 * 2^((note - 69) / 12)`.

## 3. Scope of Work

1. Pass the active `TuningSystem` from `Project` into `AudioGraph` and `OperatorSynth`.
2. When a note is played, use `tuning.note_to_freq(pitch)` rather than hardcoded 12-TET math.
3. Support microtonal pitch bends and custom root calibrations (e.g. 432 Hz, Bohlen-Pierce scale, Slendro/Pelog).

## 4. Files to Inspect

- `crates/livecraft-core/src/model.rs` (TuningSystem definition)
- `crates/livecraft-dsp/src/operator.rs` (lines 40-75)
- `crates/livecraft-dsp/src/graph.rs`

## 5. Files to Modify

- `crates/livecraft-core/src/model.rs`
- `crates/livecraft-dsp/src/operator.rs`
- `crates/livecraft-dsp/src/graph.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/` or `livecraft-cli/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/operator.rs`:
   - In `OperatorSynth`:
     - Add `pub fn note_on_with_freq(&mut self, note: u8, velocity: u8, freq_hz: f32)`.
     - In `Voice::note_on`: update oscillator phase increment using `freq_hz / sample_rate * 2.0 * PI`.
2. In `crates/livecraft-dsp/src/graph.rs`:
   - Store an optional `TuningSystem` in `AudioGraph`.
   - When triggering `note_on`, compute frequency:
     ```rust
     let freq = if let Some(ref t) = self.tuning {
         t.note_to_freq(note.pitch) as f32
     } else {
         440.0 * 2.0f32.powf((note.pitch as f32 - 69.0) / 12.0)
     };
     ```
3. Write a unit test verifying that loading a Scala 5-TET Slendro scale calculates non-12-TET frequencies accurately during synthesis.

## 8. Verification Command

```bash
cargo test -p livecraft-core --lib tests::test_tuning_system
```

## 9. Acceptance Criteria

- [ ] Non-12-TET Scala tuning systems accurately govern synthesizer pitch.
- [ ] Standard 12-TET tuning remains bit-accurate when no microtonal tuning is selected (A4 = 440.0 Hz).
- [ ] Unit tests pass cleanly.

## 10. Failure Handling Protocol

If frequency is 0 or negative:

- Clamp output frequency to `20.0 .. 20000.0 Hz`.
