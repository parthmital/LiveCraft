# Task Specification: task-016

## 1. Metadata

- **ID:** `task-016`
- **Phase:** Phase 3: DSP Device Suite Completion & Studio-Grade Modeling
- **Title:** Modulation & Space Effects (Delay, Reverb, Chorus, Saturator, Redux)
- **Dependencies:** `task-015`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 29 (Audio Effects Reference)

## 2. Context & Problem Statement

The modulation and time-domain effects (`delay.rs`, `reverb.rs`, `chorus.rs`, `saturator.rs`, `redux.rs`) must meet Ableton Live 12 studio-grade standards:

- `Delay`: Dual-delay lines with ping-pong mode, tempo-synced musical beat divisions (1/16, 1/8, 1/4, dotted, triplet), and low/high cut filter in the feedback path.
- `Reverb`: Schroeder-Freeverb algorithmic reverberator with 8 parallel comb filters and 4 all-pass diffusers, damping, pre-delay, and stereo width.
- `Chorus`: Multi-tap delay line with quadrature phase LFO modulation ($90^\circ$ stereo offset).
- `Saturator`: Multi-curve waveshaping transfer functions:
  - Soft Sine: $\tanh(x)$ or sine-clipping.
  - Hard Clip: hard threshold clamping.
  - Analog Warmth: asymmetric even-harmonic transfer curve.
  - Foldback: non-linear foldover distortion.
- `Redux`: Sample-rate downsampling with linear hold and bit-depth quantization distortion (1 to 16 bits).

## 3. Scope of Work

1. Complete all 5 effect processors in `crates/livecraft-dsp/src/`.
2. Ensure every processor implements `DspProcessor` from `task-002`.
3. Verify bit-exact neutral operation when `dry_wet = 0.0` or device is bypassed.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/delay.rs`
- `crates/livecraft-dsp/src/reverb.rs`
- `crates/livecraft-dsp/src/chorus.rs`
- `crates/livecraft-dsp/src/saturator.rs`
- `crates/livecraft-dsp/src/redux.rs`

## 5. Files to Modify

- `crates/livecraft-dsp/src/delay.rs`
- `crates/livecraft-dsp/src/reverb.rs`
- `crates/livecraft-dsp/src/chorus.rs`
- `crates/livecraft-dsp/src/saturator.rs`
- `crates/livecraft-dsp/src/redux.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/` or `livecraft-cli/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/saturator.rs`:
   - Implement the 4 transfer functions in `apply_curve(x: f32, curve: u8, drive_lin: f32) -> f32`.
   - Add transfer function calculation `curve_points(num_points) -> Vec<(f32, f32)>` for GUI plotting.
2. In `crates/livecraft-dsp/src/redux.rs`:
   - Implement sample rate reduction counter and bit-depth quantizer:
     `let step = 2.0f32.powf(16.0 - bits);`
     `let quantized = (sample * 32768.0 / step).round() * step / 32768.0;`
3. In `crates/livecraft-dsp/src/delay.rs`:
   - Implement ping-pong cross-feedback and lowpass/highpass filter on feedback lines.
4. In `crates/livecraft-dsp/src/reverb.rs`:
   - Verify comb filter delay lengths are prime numbers to eliminate metallic resonances.
5. In `crates/livecraft-dsp/src/chorus.rs`:
   - Implement $90^\circ$ quadrature phase stereo LFO.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_saturator_processing
```

## 9. Acceptance Criteria

- [ ] All 5 audio effects pass tests with 0 warnings.
- [ ] Bypassed or 0% wet devices produce bit-for-bit null test with dry input.
- [ ] No heap allocations on audio callbacks.

## 10. Failure Handling Protocol

If feedback delay lines explode:

- Clamp feedback parameter to maximum 0.95 to guarantee BIBO (bounded-input bounded-output) stability.
