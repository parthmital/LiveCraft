# Task Specification: task-023

## 1. Metadata

- **ID:** `task-023`
- **Phase:** Phase 4: Ableton Live 12 Desktop Interface Parity
- **Title:** Modular Device Rack Cards & Live 12 Stacked View Architecture
- **Dependencies:** `task-022`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 3.12 (Clip and Device View, pp. 49-51) & Chapter 8

## 2. Context & Problem Statement

Ableton Live 12 introduced a signature architectural feature: **Stacked Views** (Manual p. 51). In Live 12, users can view Clip View AND Device View at the exact same time (stacked vertically or split), as well as viewing the Arrangement Mixer simultaneously! In `crates/livecraft-gui/src/app.rs`, the bottom panel is currently an exclusive radio tab (0 = Devices, 1 = Piano Roll), completely unable to display both simultaneously. Furthermore, device rack cards in `devices.rs` lack authentic Ableton header styling, preset selectors, and fold triangles.

## 3. Scope of Work

1. Implement Ableton Live 12 Stacked Views in `crates/livecraft-gui/src/app.rs`:
   - Bottom view state machine supporting:
     - Clip View Only
     - Device View Only
     - Stacked: Clip View and Device View visible simultaneously (split vertically).
   - Bottom-right view selector toggles (`▲ Clip View` and `▲ Device View`) matching Ableton Live 12.
2. Polish Modular Device Rack Cards in `crates/livecraft-gui/src/devices.rs`:
   - Title bar: Power toggle (`●`), Device Name, Preset browser hot-swap button, Fold triangle (`▼`), and Delete button (`×`).
   - Drag-to-reorder: Ability to drag device cards horizontally to reorder the signal processing chain.
   - Interactive displays:
     - `EQ Eight`: Draw actual interactive frequency curve using `magnitude_response` from `task-014`.
     - `Saturator`: Draw transfer function curve.
     - `Compressor`: Vertical gain-reduction meter bar.

## 4. Files to Inspect

- `crates/livecraft-gui/src/app.rs` (lines 400-470)
- `crates/livecraft-gui/src/devices.rs`

## 5. Files to Modify

- `crates/livecraft-gui/src/app.rs`
- `crates/livecraft-gui/src/devices.rs`

## 6. What NOT to Touch

- Do NOT touch `session.rs` or `arrange.rs`.
- Do NOT touch `livecraft-dsp/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/app.rs`:
   - Replace `pub bottom_tab: usize` with:
     ```rust
     pub clip_view_visible: bool,
     pub device_view_visible: bool,
     ```
   - If both are `true`, allocate bottom detail area with two vertically stacked panels or split pane.
   - Connect `Shift + Tab` and `F12` to toggle focus between Clip and Device views; `Ctrl + Alt + 3` toggles Clip View; `Ctrl + Alt + 4` toggles Device View.
2. In `crates/livecraft-gui/src/devices.rs`:
   - Render device rack cards with authentic headers and rounded corners.
   - For `EQ Eight`: plot biquad magnitude curve across 20 Hz – 20 kHz on a logarithmic frequency axis.
   - For `Saturator`: plot waveshaping transfer curve across $[-1.0, 1.0]$.
   - For `Compressor`: draw active yellow/red gain-reduction meter bar.
3. Run `cargo test -p livecraft-gui`.

## 8. Verification Command

```bash
cargo test -p livecraft-gui
```

## 9. Acceptance Criteria

- [ ] Clip View and Device View can be displayed simultaneously in Live 12 Stacked View layout.
- [ ] Device cards display authentic Ableton headers with power toggles and fold controls.
- [ ] EQ Eight and Saturator render interactive DSP response curves.

## 10. Failure Handling Protocol

If rendering two stacked panels cramps screen space on low resolutions:

- Set minimum panel heights (140px each) and provide an egui scroll area.
