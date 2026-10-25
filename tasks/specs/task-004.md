# Task Specification: task-004

## 1. Metadata

- **ID:** `task-004`
- **Phase:** Phase 1: Real-Time Audio Engine & DSP Architecture Hardening
- **Title:** Audio Clip Continuous Sample Streaming & Hermite Interpolation
- **Dependencies:** `task-003`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 9 (Audio Clips & Warping) & Chapter 39 (Audio Fact Sheet)

## 2. Context & Problem Statement

In `crates/livecraft-dsp/src/graph.rs:872`, `sample_clip` is only evaluated once per sub-block (every 32 samples). Inside `process_buffer`, the scalar loop reuses the exact same `current_audio_in` value 32 times consecutively. This produces an extreme 32x downsampling staircase artifact, ruining audio playback with harsh aliasing distortion.

## 3. Scope of Work

1. Refactor audio clip playback in `TrackDspState` and `AudioSampleCache`:
   - Calculate audio sample index per sample (or per continuous buffer stream), not once every 32 samples.
   - Implement cubic Hermite or 4-point polynomial interpolation for warped and unwarped audio playback.
2. Guarantee that loading or caching audio sample buffers allocates only on project load or background cache warmup, never on the real-time audio thread.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/audio_clip.rs`
- `crates/livecraft-dsp/src/graph.rs` (lines 75-105, 705-720, and 870-875)

## 5. Files to Modify

- `crates/livecraft-dsp/src/audio_clip.rs`
- `crates/livecraft-dsp/src/graph.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-core/src/model.rs` or `als.rs`.
- Do NOT touch synthesizer code in `operator.rs` or `simpler.rs`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/audio_clip.rs`:
   - Implement 4-point cubic Hermite interpolation:
     ```rust
     #[inline(always)]
     pub fn hermite_interp(y0: f32, y1: f32, y2: f32, y3: f32, mu: f32) -> f32 {
         let mu2 = mu * mu;
         let a0 = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
         let a1 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
         let a2 = -0.5 * y0 + 0.5 * y2;
         let a3 = y1;
         a0 * mu * mu2 + a1 * mu2 + a2 * mu + a3
     }
     ```
   - Provide a block-rendering method `render_clip_samples(clip, start_beat, end_beat, bpm, out_l, out_r)`.
2. In `crates/livecraft-dsp/src/graph.rs`:
   - In `process_buffer`, generate continuous sample buffers for audio clips across the entire sub-block length, rather than holding a single static `(f32, f32)` pair for 32 samples.
3. Verify with `test_arrangement_timeline_playback`.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_arrangement_timeline_playback
```

## 9. Acceptance Criteria

- [ ] No staircased repeated samples in audio clip playback.
- [ ] Continuous pitch/time interpolation passes smoothly without clicks or aliasing.
- [ ] Offline bouncer and real-time playback render audio clips transparently.

## 10. Failure Handling Protocol

If buffer index out of bounds occurs during interpolation:

- Ensure boundary clamping: `clamp(0, samples.len() - 1)` for all 4 interpolation points.
