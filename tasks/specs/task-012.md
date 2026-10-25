# Task Specification: task-012

## 1. Metadata

- **ID:** `task-012`
- **Phase:** Phase 3: DSP Device Suite Completion & Studio-Grade Modeling
- **Title:** Simpler Polyphonic Sampler (Classic, 1-Shot, Slicing)
- **Dependencies:** `task-011`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 31 (Instruments: Simpler)

## 2. Context & Problem Statement

`crates/livecraft-dsp/src/simpler.rs` currently implements a rudimentary mono sample player. Ableton Live's `Simpler` operates in three distinct modes:

1. **Classic Mode:** Polyphonic playback with start/end trim, looping, loop crossfade, and ADSR volume/filter envelopes.
2. **1-Shot Mode:** Monophonic playback from start to end without looping, with trigger or gate release modes.
3. **Slicing Mode:** Transient-based sample slicing across incoming MIDI note triggers (C1 upwards).

## 3. Scope of Work

1. Implement all three Simpler playback modes in `crates/livecraft-dsp/src/simpler.rs`:
   - Polyphonic voices (up to 16 voices) for Classic mode.
   - Start, Length, and End percentage controls (0.0 to 100.0%).
   - Crossfade looping with adjustable crossfade length.
   - Slicing mode with transient division based on threshold or grid.
2. Filter section with Lowpass, Highpass, Bandpass modes and envelope modulation.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/simpler.rs`
- `crates/livecraft-core/src/model.rs` (DeviceType::Simpler)

## 5. Files to Modify

- `crates/livecraft-dsp/src/simpler.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/` or `livecraft-cli/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/simpler.rs`:
   - Define:
     ```rust
     #[derive(Debug, Clone, Copy, PartialEq, Eq)]
     pub enum SimplerPlaybackMode {
         Classic,
         OneShot,
         Slicing,
     }
     ```
   - In `SimplerInstrument`:
     - Maintain a voice pool `voices: Vec<SimplerVoice>`.
     - In `note_on(note, velocity)`:
       - In Classic mode: map pitch relative to `root_note` (pitch transposition ratio: `2^((note - root_note) / 12)`).
       - In Slicing mode: map note offset `note - 36` to slice index, triggering playback from the slice's start sample offset.
     - In `process_sample()`: sum active voice outputs through the shared resonant filter.
2. Write unit tests loading a synthesized WAV buffer and testing Classic, 1-Shot, and Slicing modes.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib
```

## 9. Acceptance Criteria

- [ ] Classic, 1-Shot, and Slicing modes are implemented.
- [ ] Polyphonic sample playback tracks pitch accurately with crossfade looping.
- [ ] Zero heap allocations during steady-state audio processing.

## 10. Failure Handling Protocol

If WAV reading fails:

- Handle missing sample files gracefully by outputting silence rather than panicking.
