# Task Specification: task-013

## 1. Metadata

- **ID:** `task-013`
- **Phase:** Phase 3: DSP Device Suite Completion & Studio-Grade Modeling
- **Title:** Drum Rack 16-Pad Matrix, Choke Groups & Solo/Mute Routing
- **Dependencies:** `task-012`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 24 (Instrument, Drum and Effect Racks)

## 2. Context & Problem Statement

Ableton Live's `Drum Rack` provides a 16-pad playable grid (MIDI notes 36 to 51) where each pad contains its own sample or synthesized drum generator, independent volume, pan, mute, solo, and choke groups (e.g. pad 42 closed hi-hat chokes pad 46 open hi-hat). In `crates/livecraft-dsp/src/drumrack.rs`, pads are partially synthesized, but choke group assignment and pad solo/mute matrix routing are incomplete.

## 3. Scope of Work

1. Fully implement the 16-pad Drum Rack matrix in `crates/livecraft-dsp/src/drumrack.rs`:
   - 16 distinct pads mapped to MIDI notes 36–51 (C1 to D#2): Kick, Snare, Rim, Clap, Closed Hat, Open Hat, Pedal Hat, Low Tom, Mid Tom, High Tom, Crash, Ride, Shaker, Cowbell, Tambourine, Percussion.
   - Choke group engine: up to 16 choke groups; triggering any pad in a choke group immediately mutes/releases all other playing voices in that choke group.
   - Per-pad solo and mute routing: if any pad is soloed, only soloed pads sound; muted pads produce silence.
   - Per-pad stereo pan and volume faders.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/drumrack.rs`

## 5. Files to Modify

- `crates/livecraft-dsp/src/drumrack.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/` or `livecraft-cli/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/drumrack.rs`:
   - Ensure all 16 standard pads are initialized with their default notes and choke group assignments (e.g. Closed Hat note 42 and Open Hat note 46 assigned to Choke Group 1).
   - In `note_on(note, velocity)`:
     - Find the targeted pad.
     - If the pad belongs to `choke_group: Some(cg)`:
       - Iterate over all other pads with the same `choke_group` and call `.choke()`.
     - Trigger the pad voice.
   - In `process_sample()`:
     - Check `let any_pad_solo = self.pads.iter().any(|p| p.is_soloed);`
     - For each pad: if `p.is_muted || (any_pad_solo && !p.is_soloed)`, skip sample summing.
     - Sum pan-weighted stereo audio:
       `sum_l += pad_l * vol_lin * pan_l; sum_r += pad_r * vol_lin * pan_r;`
2. Test choke group cutoff with `test_drum_rack_synthesis_and_choke`.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_drum_rack_synthesis_and_choke
```

## 9. Acceptance Criteria

- [ ] 16 drum pads trigger authentic percussive waveforms with velocity sensitivity.
- [ ] Choke group muting cuts off ringing open hi-hats immediately upon closed hi-hat onset.
- [ ] Pad solo and mute operations function accurately without memory allocations.

## 10. Failure Handling Protocol

If choke causes clicks:

- Implement a 5-millisecond quick exponential fade-out rather than an instantaneous step to zero.
