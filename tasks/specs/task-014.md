# Task Specification: task-014

## 1. Metadata

- **ID:** `task-014`
- **Phase:** Phase 3: DSP Device Suite Completion & Studio-Grade Modeling
- **Title:** EQ Eight 8-Band Parametric Filter & Frequency Response Curve
- **Dependencies:** `task-013`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 29 (Audio Effects: EQ Eight)

## 2. Context & Problem Statement

Ableton Live's `EQ Eight` is an 8-band parametric biquad equalizer supporting 8 filter topologies per band (Cut 48dB, Cut 12dB, Low Shelf, Bell/Peak, Notch, High Shelf, High Cut 12dB, High Cut 48dB). While `crates/livecraft-dsp/src/eq8.rs` processes biquad filters, it lacks:

1. Dynamic filter topology switching per band.
2. A mathematical frequency response evaluation function `magnitude_at_freq(freq_hz) -> f32` to allow the GUI to draw the actual cumulative EQ response curve.

## 3. Scope of Work

1. Complete all 8 filter topologies in `crates/livecraft-dsp/src/biquad.rs` and `eq8.rs`.
2. Implement complex transfer-function evaluation for biquad filters:
   $$H(z) = \frac{b_0 + b_1 z^{-1} + b_2 z^{-2}}{a_0 + a_1 z^{-1} + a_2 z^{-2}}$$
   where $z = e^{j \omega}$ with $\omega = 2 \pi f / f_s$.
3. Provide `eq8.response_curve_points(sample_rate, num_points) -> Vec<(f32, f32)>` for frequency curve plotting.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/biquad.rs`
- `crates/livecraft-dsp/src/eq8.rs`

## 5. Files to Modify

- `crates/livecraft-dsp/src/biquad.rs`
- `crates/livecraft-dsp/src/eq8.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/` (GUI curve visualization is hooked in `task-023`).

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/biquad.rs`:
   - Implement `magnitude_response(&self, freq_hz: f32) -> f32`:
     ```rust
     pub fn magnitude_response(&self, freq_hz: f32) -> f32 {
         let omega = 2.0 * std::f32::consts::PI * freq_hz / self.sample_rate;
         let cos1 = omega.cos();
         let cos2 = (2.0 * omega).cos();
         let sin1 = omega.sin();
         let sin2 = (2.0 * omega).sin();

         let num_re = self.b0 + self.b1 * cos1 + self.b2 * cos2;
         let num_im = -self.b1 * sin1 - self.b2 * sin2;
         let den_re = 1.0 + self.a1 * cos1 + self.a2 * cos2;
         let den_im = -self.a1 * sin1 - self.a2 * sin2;

         let num_sq = num_re * num_re + num_im * num_im;
         let den_sq = den_re * den_re + den_im * den_im;
         if den_sq <= 1e-12 {
             return 1.0;
         }
         (num_sq / den_sq).sqrt()
     }
     ```
2. In `crates/livecraft-dsp/src/eq8.rs`:
   - Combine all active enabled bands:
     `let total_mag = band_mags.iter().fold(1.0, |acc, m| acc * m);`
     `let total_gain_db = 20.0 * total_mag.max(1e-5).log10();`
3. Add a unit test verifying magnitude response at band cutoff and center frequencies.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_eq8_processing
```

## 9. Acceptance Criteria

- [ ] Biquad complex frequency response evaluation is mathematically accurate.
- [ ] 8-band cascading preserves 32-bit floating point precision without instability.
- [ ] Unit tests pass with zero warnings.

## 10. Failure Handling Protocol

If den_sq denominator is zero:

- Guard with `den_sq.max(1e-12)` to prevent division by zero or NaN generation.
