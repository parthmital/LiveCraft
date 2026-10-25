# Task Specification: task-011

## 1. Metadata

- **ID:** `task-011`
- **Phase:** Phase 3: DSP Device Suite Completion & Studio-Grade Modeling
- **Title:** Operator 4-Operator FM Synthesizer with 11 Routing Algorithms
- **Dependencies:** `task-010`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 31 (Instruments: Operator)

## 2. Context & Problem Statement

Currently, `crates/livecraft-dsp/src/operator.rs` is a toy 2-oscillator additive synthesizer (`(s1 + s2) * 0.5`). Ableton Live's `Operator` is a 4-oscillator (A, B, C, D) frequency modulation (FM) and subtractive synthesizer featuring 11 distinct routing algorithms (A modulates B, B modulates C, parallel carriers, feedback on operator D, etc.), multi-waveform oscillators (Sine, Saw, Square, Triangle, Noise), and 24dB resonant filters.

## 3. Scope of Work

1. Re-architect `OperatorSynth` into a true 4-operator FM engine:
   - Operators A, B, C, D: each with its own Waveform, Coarse/Fine ratio multiplier, Level, and ADSR envelope.
   - 11 FM Routing Algorithms matching Ableton Live Operator (Algorithms 1 through 11).
   - Feedback modulation loop on Operator D.
   - Master filter (Lowpass 12/24dB, Highpass, Bandpass, Notch) with filter envelope amount and resonance.
   - Pitch envelope with glide / portamento support.
2. 16-voice polyphony with sample-accurate note-on / note-off dispatch.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/operator.rs`
- `docs/DEVICES_REFERENCE.md` (Operator section)

## 5. Files to Modify

- `crates/livecraft-dsp/src/operator.rs`

## 6. What NOT to Touch

- Do NOT touch `biquad.rs` filter math.
- Do NOT touch `livecraft-gui/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/operator.rs`:
   - Define `OperatorUnit`:
     ```rust
     #[derive(Debug, Clone)]
     pub struct OperatorUnit {
         pub wave: Waveform,
         pub level: f32,
         pub coarse: f32, // Multiplier (0.5, 1, 2, 3...)
         pub fine: f32,
         pub phase: f32,
         pub envelope: AdsrEnvelope,
     }
     ```
   - In `Voice`:
     - Maintain 4 `OperatorUnit` instances (A, B, C, D).
     - Implement the 11 FM algorithms:
       - Alg 1: D -> C -> B -> A (Cascade FM)
       - Alg 2: (D + C) -> B -> A
       - Alg 3: (D -> C) + (B -> A)
       - Alg 4: (D -> C -> B) + A
       - Alg 5: (D -> B) + (C -> B) -> A
       - Alg 6: D -> (C + B + A)
       - Alg 7: (D -> C) + B + A
       - Alg 8: D + C + B + A (Pure 4-osc additive)
       - Alg 9, 10, 11: Hybrid parallel/carrier routings.
   - Apply FM phase modulation:
     `let phase_mod = modulator_output * modulation_index;`
     `let carrier_sample = (phase + phase_mod).sin();`
   - Feed combined carriers into resonant biquad filter.
2. Write unit tests verifying that all 11 algorithms generate valid audio waveforms without NaN or infinite values.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_operator_all_waveforms
```

## 9. Acceptance Criteria

- [ ] 4 independent FM operators with dedicated ADSR envelopes.
- [ ] 11 selectable routing algorithms functioning accurately.
- [ ] Operator D feedback produces rich harmonic distortion and noise textures.
- [ ] Zero audio allocations in `process_sample`.

## 10. Failure Handling Protocol

If phase modulation produces numeric explosion:

- Wrap phase inside `0.0 .. 2.0 * PI` and clamp modulation index within stable limits (`0.0 .. 16.0`).
