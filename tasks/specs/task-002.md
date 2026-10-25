# Task Specification: task-002

## 1. Metadata

- **ID:** `task-002`
- **Phase:** Phase 1: Real-Time Audio Engine & DSP Architecture Hardening
- **Title:** Dynamic Modular DSP Processor Chain & Topological Order
- **Dependencies:** `task-001`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 17 (Routing) & Chapter 29 (Audio Effect Chains)

## 2. Context & Problem Statement

In `crates/livecraft-dsp/src/graph.rs:328-420`, `TrackDspState` executes audio effects in a single, hardcoded order (`synth -> eq8 -> compressor -> saturator -> auto_filter -> ...`), completely ignoring the actual order of devices in `track.device_chain`. Furthermore, each track can have at most one device of each type; adding two Delays or two EQs results in one silently overwriting the other.

## 3. Scope of Work

1. Introduce a unified `DspProcessor` trait for all audio effects and instruments:
   ```rust
   pub trait DspProcessor: Send + Sync {
       fn process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32);
       fn update_param(&mut self, key: &str, value: f32);
       fn reset(&mut self);
       fn is_enabled(&self) -> bool;
       fn set_enabled(&mut self, enabled: bool);
       fn device_id(&self) -> u64;
   }
   ```
2. Refactor `TrackDspState`:
   - Replace fixed fields (`eq8`, `delay`, `reverb`, etc.) with `pub processors: Vec<Box<dyn DspProcessor>>`.
   - Process audio samples sequentially through `processors` in the exact index order defined by the user.
3. Support multiple instances of the same device type on a single track.

## 4. Files to Inspect

- `crates/livecraft-dsp/src/graph.rs` (lines 55-150 and 328-420)
- `crates/livecraft-core/src/model.rs` (Device definition)

## 5. Files to Modify

- `crates/livecraft-dsp/src/graph.rs`
- `crates/livecraft-dsp/src/lib.rs`

## 6. What NOT to Touch

- Do NOT touch `crates/livecraft-gui/` or `crates/livecraft-cli/`.
- Do NOT rewrite individual device DSP implementations (leave `biquad.rs`, `compressor.rs`, etc. internal math intact).

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-dsp/src/lib.rs` (or a dedicated module `processor.rs`):
   - Define the `DspProcessor` trait.
   - Implement `DspProcessor` for existing structs: `EqEight`, `CompressorEffect`, `DelayEffect`, `ReverbEffect`, `LimiterEffect`, `UtilityEffect`, `SaturatorEffect`, `AutoFilterEffect`, `ChorusEffect`, `AutoPanEffect`, `GateEffect`, `EqThreeEffect`, `ReduxEffect`.
2. In `crates/livecraft-dsp/src/graph.rs`:
   - Update `TrackDspState`:
     ```rust
     pub struct TrackDspState {
         pub track_id: u64,
         pub synth: Option<OperatorSynth>,
         pub simpler: Option<SimplerInstrument>,
         pub drum_rack: Option<DrumRackInstrument>,
         pub processors: Vec<Box<dyn DspProcessor>>,
         // ... meters and caching
     }
     ```
   - In `process_sample`:
     ```rust
     let (mut l, mut r) = (in_l, in_r);
     // Source synthesis
     if let Some(ref mut s) = self.synth { ... }
     // Chain processing in sequential order
     for proc in &mut self.processors {
         if proc.is_enabled() {
             let (out_l, out_r) = proc.process_stereo(l, r);
             l = out_l;
             r = out_r;
         }
     }
     (l, r)
     ```
   - In `sync_devices`, construct processors dynamically matching the IDs and types in `track.device_chain`.
3. Add a unit test verifying that placing `Delay` before `Compressor` vs `Compressor` before `Delay` processes in true chain order.

## 8. Verification Command

```bash
cargo test -p livecraft-dsp --lib tests::test_graph_dynamic_track_sync
```

## 9. Acceptance Criteria

- [ ] Tracks can host multiple instances of the same effect (e.g. 2 Delays or 2 EQs).
- [ ] Effect execution order strictly matches the order in `track.device_chain`.
- [ ] Existing DSP unit tests continue to pass.

## 10. Failure Handling Protocol

If lifetime errors occur with `Box<dyn DspProcessor>`:

- Ensure the trait is bound by `'static + Send + Sync`.
