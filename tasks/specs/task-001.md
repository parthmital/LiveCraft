# Task Specification: task-001

## 1. Metadata

- **ID:** `task-001`
- **Phase:** Phase 1: Real-Time Audio Engine & DSP Architecture Hardening
- **Title:** Lock-Free Parameter Dispatch & Audio State Decoupling
- **Dependencies:** None (Entry point)
- **Manual Grounding:** Ableton Live 12 Manual Chapter 38 (Audio Thread Safety: zero locks, zero allocations)

## 2. Context & Problem Statement

Currently, `livecraft_dsp::engine::AudioEngine` uses an `Arc<RwLock<Project>>` to communicate project changes to the audio thread. On every single frame in `crates/livecraft-gui/src/app.rs:260`, the GUI calls `eng.sync_project(&self.project)`, which clones the entire `Project` struct 60–144 times per second. This causes severe heap churn and lock contention with `try_read()` on the audio callback, violating Ableton Live 12 Chapter 38's strict lock-free real-time audio constraint.

## 3. Scope of Work

1. Replace whole-project cloning on the audio thread with a lock-free parameter update queue or atomic parameter store.
2. In `crates/livecraft-dsp/src/engine.rs`:
   - Introduce a thread-safe parameter event queue or atomic message ring buffer (`crossbeam-channel` or pre-allocated atomic array).
   - Change `sync_project` so it only dispatches diffs/events (such as volume change, pan change, mute/solo toggle, tempo change) instead of cloning `Project`.
   - Remove the `RwLock<Project>` from the real-time audio callback path.
3. In `crates/livecraft-gui/src/app.rs`:
   - Remove the unconditional per-frame `eng.sync_project(&self.project)`.
   - Dispatch discrete parameter changes only when user interaction or commands actually mutate state.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/engine.rs`
- `crates/livecraft-gui/src/app.rs` (lines 255-270)
- `crates/livecraft-dsp/src/graph.rs` (lines 530-575)

## 5. Files to Modify

- `crates/livecraft-dsp/src/engine.rs`
- `crates/livecraft-gui/src/app.rs`
- `crates/livecraft-dsp/src/graph.rs`

## 6. What NOT to Touch

- Do NOT touch `crates/livecraft-core/src/als.rs` or `model.rs`.
- Do NOT alter the visual widget rendering logic in `widgets.rs` or `transport.rs`.
- Do NOT break existing tests in `livecraft-dsp`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/engine.rs`:
   - Define an enum `AudioParamMsg`:
     ```rust
     pub enum AudioParamMsg {
         SetTempo(f32),
         SetTrackVolume { track_idx: usize, volume_db: f32 },
         SetTrackPan { track_idx: usize, pan: f32 },
         SetTrackMute { track_idx: usize, muted: bool },
         SetTrackSolo { track_idx: usize, soloed: bool },
         SetDeviceParam { track_idx: usize, device_id: u64, param_key: String, value: f32 },
         TriggerClip { track_idx: usize, slot_idx: usize },
         StopClip { track_idx: usize },
         StopAll,
     }
     ```
   - Use `crossbeam_channel::Sender<AudioParamMsg>` and `crossbeam_channel::Receiver<AudioParamMsg>` (bounded to 1024 messages) to pass parameter updates from GUI/CLI into `AudioEngine`.
   - In `write_audio`, drain all pending messages via `receiver.try_recv()` without blocking, applying updates directly to `AudioGraph` state.
   - Retain `sync_project` as a one-time project loader when opening/loading a project, but ensure steady-state updates use `send_param_msg`.
2. In `crates/livecraft-gui/src/app.rs`:
   - Remove `eng.sync_project(&self.project)` from the per-frame `update` function.
   - When a fader or knob changes in UI, send the specific `AudioParamMsg` to `eng`.
3. Verify that `cargo check` and existing `livecraft-dsp` tests compile cleanly.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_device_parameter_synchronization
```

## 9. Acceptance Criteria

- [ ] No `Project.clone()` occurs on every GUI render frame.
- [ ] The real-time audio callback drains parameter messages via `try_recv()` without locking `RwLock`.
- [ ] `cargo test -p livecraft-dsp --lib` passes with 0 failures and 0 warnings.

## 10. Failure Handling Protocol

If compilation fails due to `crossbeam_channel` dependency:

- Check `crates/livecraft-dsp/Cargo.toml` and ensure `crossbeam-channel = "0.5"` is present.
  If tests fail:
- Ensure the bounded queue does not block if full (use `try_send` and log warning if queue is full).
