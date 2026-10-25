# Task Specification: task-015

## 1. Metadata

- **ID:** `task-015`
- **Phase:** Phase 3: DSP Device Suite Completion & Studio-Grade Modeling
- **Title:** Dynamics Suite (Compressor Knee, Brickwall Limiter, Gate Hysteresis)
- **Dependencies:** `task-014`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 29 (Audio Effects: Compressor, Limiter, Gate)

## 2. Context & Problem Statement

The dynamics processors in `crates/livecraft-dsp/` (`compressor.rs`, `limiter.rs`, `gate.rs`) must meet studio-grade Ableton Live 12 specifications:

1. `Compressor`: Needs soft-knee compression curve, peak vs RMS level detection, and real-time gain-reduction metering (`gain_reduction_db`).
2. `Limiter`: Needs transparent lookahead buffer (1.5ms, 3ms, 6ms), brickwall ceiling enforcement, and soft-clip saturation stage.
3. `Gate`: Needs dual-threshold hysteresis (`Threshold` and `Return`), lookahead, hold timer, attack, and attenuation floor (-inf to 0 dB).

## 3. Scope of Work

1. Refactor `CompressorEffect` in `crates/livecraft-dsp/src/compressor.rs`:
   - Implement soft-knee gain calculation:
     For input level within $[T - K/2, T + K/2]$, smooth smoothly between $1:1$ and $R:1$.
   - Expose `pub fn gain_reduction_db(&self) -> f32` for GUI meter display.
2. Refactor `LimiterEffect` in `crates/livecraft-dsp/src/limiter.rs`:
   - Implement circular lookahead ring-buffer ensuring true zero-overshoot brickwall limiting.
3. Refactor `GateEffect` in `crates/livecraft-dsp/src/gate.rs`:
   - Enforce hysteresis: gate opens at $T$, but only closes when signal drops below $T - \text{Return}$.
   - Add hold timer counting down in audio samples before release stage begins.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/compressor.rs`
- `crates/livecraft-dsp/src/limiter.rs`
- `crates/livecraft-dsp/src/gate.rs`

## 5. Files to Modify

- `crates/livecraft-dsp/src/compressor.rs`
- `crates/livecraft-dsp/src/limiter.rs`
- `crates/livecraft-dsp/src/gate.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/` or `livecraft-cli/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/compressor.rs`:
   - Implement soft knee characteristic curve.
   - Store current gain reduction: `self.gain_reduction_db = (linear_gain).max(1e-5).log10() * 20.0;`
2. In `crates/livecraft-dsp/src/limiter.rs`:
   - Allocate pre-sized lookahead buffer `[f32; 1024]` per channel.
   - Enforce brickwall ceiling: `out_sample = out_sample.clamp(-ceiling_lin, ceiling_lin);`
3. In `crates/livecraft-dsp/src/gate.rs`:
   - Implement state machine: `Closed`, `Attack`, `Open`, `Hold`, `Release`.
   - Gate opening condition: `input_level >= threshold_lin`.
   - Gate closing condition: `input_level < return_threshold_lin` and `hold_counter == 0`.
4. Run `cargo test -p livecraft-dsp --lib tests::test_compressor_gain_reduction`.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_compressor_gain_reduction
```

## 9. Acceptance Criteria

- [ ] Compressor computes accurate gain reduction with soft-knee smoothing.
- [ ] Limiter enforces brickwall ceiling without digital overs.
- [ ] Gate hysteresis prevents chattering around threshold level.

## 10. Failure Handling Protocol

If click artifacts occur on sudden transients:

- Verify attack smoothing time constant is $\ge 0.1\text{ ms}$ and exponential gain follower smooths transitions.
