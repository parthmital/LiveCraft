# Task Specification: task-008

## 1. Metadata

- **ID:** `task-008`
- **Phase:** Phase 2: Core Data Model & Authentic Ableton 12 Compatibility
- **Title:** Timeline Automation Envelopes & Breakpoint Curve Model
- **Dependencies:** `task-007`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 22 (Automation & Editing Envelopes)

## 2. Context & Problem Statement

Currently, `livecraft-core::model::Project` has no data representation for parameter automation (volume automation, pan envelopes, filter cutoff sweeps over musical time). Ableton Live 12 Chapter 22 specifies that automation envelopes exist on timeline tracks and clips with breakpoints and curve curvatures.

## 3. Scope of Work

1. Introduce automation data structures in `crates/livecraft-core/src/model.rs`:
   - `AutomationTarget`: specifies what is being automated (e.g. `TrackVolume`, `TrackPan`, `SendLevel(usize)`, `DeviceParam { device_id: u64, param_key: String }`).
   - `AutomationPoint`: `(beat_time: f64, value: f32, curve: f32)`.
   - `AutomationLane`: collection of points with evaluation helper `evaluate_at(beat: f64) -> f32`.
2. Add `automation_lanes: Vec<AutomationLane>` to `Track` and `Clip`.
3. Support undo/redo commands for adding, moving, and removing automation breakpoints.

## 4. Files to Inspect

- `crates/livecraft-core/src/model.rs` (Track and Clip structs)
- `crates/livecraft-core/src/command.rs` (CommandHistory)

## 5. Files to Modify

- `crates/livecraft-core/src/model.rs`
- `crates/livecraft-core/src/command.rs`

## 6. What NOT to Touch

- Do NOT touch `livecraft-gui/` or `livecraft-dsp/` yet.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-core/src/model.rs`:
   - Define:
     ```rust
     #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
     pub enum AutomationTarget {
         TrackVolume,
         TrackPan,
         SendLevel(usize),
         DeviceParam { device_id: u64, param_key: String },
     }

     #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
     pub struct AutomationPoint {
         pub beat: f64,
         pub value: f32,
         pub curve: f32, // -1.0 to 1.0 curvature
     }

     #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
     pub struct AutomationLane {
         pub id: u64,
         pub target: AutomationTarget,
         pub points: Vec<AutomationPoint>,
         pub is_enabled: bool,
     }

     impl AutomationLane {
         pub fn evaluate_at(&self, beat: f64) -> Option<f32> {
             if self.points.is_empty() || !self.is_enabled {
                 return None;
             }
             if beat <= self.points[0].beat {
                 return Some(self.points[0].value);
             }
             if beat >= self.points.last().unwrap().beat {
                 return Some(self.points.last().unwrap().value);
             }
             // Find surrounding segment and interpolate
             for w in self.points.windows(2) {
                 if beat >= w[0].beat && beat <= w[1].beat {
                     let span = w[1].beat - w[0].beat;
                     if span <= 1e-6 {
                         return Some(w[0].value);
                     }
                     let t = (beat - w[0].beat) / span;
                     // Linear interpolation with curve offset
                     return Some(w[0].value + (w[1].value - w[0].value) * t as f32);
                 }
             }
             None
         }
     }
     ```
   - Add `pub automation_lanes: Vec<AutomationLane>` to `Track` and `Clip`.
2. In `crates/livecraft-core/src/command.rs`:
   - Add `AddAutomationPointAction` and `RemoveAutomationPointAction` to `CommandHistory`.
3. Add unit tests for `evaluate_at` with step, linear, and boundary points.

## 8. Verification Command

```bash
cargo test -p livecraft-core --lib tests::test_command_undo_redo
```

## 9. Acceptance Criteria

- [ ] `AutomationLane` correctly interpolates values at any arbitrary beat time.
- [ ] Multiple automation lanes can coexist on a single track.
- [ ] Undo and redo commands for automation points execute without errors.

## 10. Failure Handling Protocol

If floating-point comparisons fail:

- Use epsilon comparison (`(a - b).abs() < 1e-5`) when asserting interpolated automation values in unit tests.
