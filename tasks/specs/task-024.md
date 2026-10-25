# Task Specification: task-024

## 1. Metadata

- **ID:** `task-024`
- **Phase:** Phase 4: Ableton Live 12 Desktop Interface Parity
- **Title:** Complete Left Browser Taxonomy & Asset Hot-Swapping System
- **Dependencies:** `task-023`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 4 (Working with the Browser, pp. 61-120)

## 2. Context & Problem Statement

In `crates/livecraft-gui/src/browser.rs`, clicking on almost all categories (Sounds, Drums, MIDI Effects, Plug-Ins, Clips, Samples, Places, Collections) displays a static label "No items in folder". Furthermore, under Audio Effects, only 5 devices are listed, and device IDs are hardcoded to `1001` and `1002`, risking ID collisions in the project model.

## 3. Scope of Work

1. Populate all Browser categories with authentic Ableton Live assets:
   - **Instruments:** Operator, Simpler, Drum Rack, Wavetable, Analog, Drift.
   - **Audio Effects:** EQ Eight, Compressor, Delay, Reverb, Saturator, Auto Filter, Chorus-Ensemble, Auto Pan-Tremolo, Gate, EQ Three, Redux, Limiter, Utility.
   - **MIDI Effects:** Arpeggiator, Chord, Pitch, Random, Scale, Velocity, Note Length.
   - **Drums:** 808 Kit, 909 Kit, Acoustic Kit, Electronic Kit presets for Drum Rack.
   - **Places:**
     - `User Library`: lists local user presets.
     - `Current Project`: lists current set and sample assets.
2. Implement live asset insertion:
   - Clicking or dragging an instrument or effect generates a unique incremented ID and inserts it into the selected track's device chain.
   - Real-time search filter matching device names and tags across all categories.

## 4. Files to Inspect

- `crates/livecraft-gui/src/browser.rs`
- `crates/livecraft-core/src/model.rs` (Device, DeviceType)

## 5. Files to Modify

- `crates/livecraft-gui/src/browser.rs`
- `crates/livecraft-gui/src/app.rs`

## 6. What NOT to Touch

- Do NOT touch `session.rs` or `arrange.rs`.
- Do NOT touch `livecraft-dsp/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/browser.rs`:
   - Expand `render_content_pane` to populate:
     - All 13 Audio Effects.
     - All 7 MIDI Effects.
     - Built-in Instruments and Drum Kits.
   - For `Places -> Current Project`: read project directory and list project files.
   - When an item is selected: return `BrowserAction::AddDevice` with a dynamically generated unique ID:
     `let next_id = selected_track.device_chain.iter().map(|d| d.id).max().unwrap_or(0) + 1;`
2. In `crates/livecraft-gui/src/app.rs`:
   - Handle `action.add_device`: push into `selected_track.device_chain` and dispatch parameter message to audio engine.
3. Run `cargo test -p livecraft-gui`.

## 8. Verification Command

```bash
cargo test -p livecraft-gui
```

## 9. Acceptance Criteria

- [ ] All Audio Effects, MIDI Effects, and Instruments are cataloged and searchable in the Browser.
- [ ] Inserting a device from the browser assigns a unique ID and appends it to the track chain.
- [ ] Search query filters items across all categories instantly.

## 10. Failure Handling Protocol

If inserting a device on an Audio track causes an invalid instrument error:

- Enforce track type validation: instruments and MIDI effects can only be inserted on MIDI tracks; audio effects can be inserted on Audio, MIDI, or Return tracks.
