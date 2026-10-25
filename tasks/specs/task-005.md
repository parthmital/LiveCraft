# Task Specification: task-005

## 1. Metadata

- **ID:** `task-005`
- **Phase:** Phase 1: Real-Time Audio Engine & DSP Architecture Hardening
- **Title:** In/Auto/Off Monitoring Logic & Multi-Channel Return Summing
- **Dependencies:** `task-004`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 17 (Routing & I/O) & Chapter 18 (Mixing)

## 2. Context & Problem Statement

The LiveCraft mixer has UI buttons for "In", "Auto", and "Off" monitoring (`_in_btn`, `_auto_btn`, `_off_btn` in `crates/livecraft-gui/src/mixer.rs:31-36`), but they are completely disconnected from the audio graph and do nothing. Furthermore, tracks only support 2 hardcoded sends (`send_a` and `send_b`); arbitrary return tracks (A through H) cannot be added or routed dynamically.

## 3. Scope of Work

1. Add `MonitoringMode` (`In`, `Auto`, `Off`) to `Track` in `livecraft-core::model`.
2. Connect `MonitoringMode` in `livecraft-dsp::graph`:
   - `In`: Track monitors live input continuously; clip playback is muted.
   - `Auto`: Track monitors live input only when track is Armed (`is_armed == true`); otherwise plays clip content.
   - `Off`: Track never monitors live input; plays clip content regardless of arm state.
3. Generalize send routing to support an arbitrary number of Return tracks (`return_tracks: Vec<Track>`), accumulating send signals into each return track dynamically.

## 4. Files to Inspect

- `crates/livecraft-core/src/model.rs` (Track struct)
- `crates/livecraft-dsp/src/graph.rs` (lines 700-760)

## 5. Files to Modify

- `crates/livecraft-core/src/model.rs`
- `crates/livecraft-dsp/src/graph.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-core/src/als.rs` (schema mapping is in `task-006`).
- Do NOT touch GUI rendering in `mixer.rs` (GUI wiring is in `task-019`).

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-core/src/model.rs`:
   - Add:
     ```rust
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
     pub enum MonitoringMode {
         In,
         Auto,
         Off,
     }
     ```
   - Add `pub monitoring: MonitoringMode` to `Track` with default `MonitoringMode::Auto`.
2. In `crates/livecraft-dsp/src/graph.rs`:
   - In `process_buffer`, evaluate track monitoring:
     ```rust
     let should_play_clips = match trk.monitoring {
         MonitoringMode::In => false,
         MonitoringMode::Auto => !trk.is_armed,
         MonitoringMode::Off => true,
     };
     ```
   - Loop over `return_tracks` dynamically:
     ```rust
     for (ret_idx, ret_state) in self.return_states.iter_mut().enumerate() {
         let send_sum_l = ...;
         let send_sum_r = ...;
         let (ret_l, ret_r) = ret_state.process_sample(send_sum_l, send_sum_r);
         master_l += ret_l;
         master_r += ret_r;
     }
     ```
3. Run `cargo test -p livecraft-dsp --lib tests::test_audio_graph_summing`.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_audio_graph_summing
```

## 9. Acceptance Criteria

- [ ] `MonitoringMode` enum is implemented and serialized in `model.rs`.
- [ ] Armed tracks with `Auto` monitoring mute clip playback to accept live input.
- [ ] Multiple return tracks receive send levels proportionally and sum to Master without clipping.

## 10. Failure Handling Protocol

If return tracks sum exceeds +0 dBFS and causes digital clipping:

- Ensure Master limiter or 32-bit floating point summing headroom prevents distortion.
