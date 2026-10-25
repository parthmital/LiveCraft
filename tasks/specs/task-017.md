# Task Specification: task-017

## 1. Metadata

- **ID:** `task-017`
- **Phase:** Phase 4: Ableton Live 12 Desktop Interface Parity
- **Title:** Ableton Live 12 Control Bar 9-Section Micro-Architectural Rebuild
- **Dependencies:** `task-016`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 3.1 (The Control Bar, pp. 34-36)

## 2. Context & Problem Statement

In `crates/livecraft-gui/src/transport.rs`, the top Control Bar is currently an approximation where some sections are missing, buttons are non-functional, and the layout doesn't follow Ableton Live 12's official 9-section structure. The Ableton Live 12 Reference Manual (Chapter 3.1) groups the Control Bar into exactly nine sections with distinct visual treatments and responsive LCD wells.

## 3. Scope of Work

Re-architect `render_transport_bar` in `crates/livecraft-gui/src/transport.rs` into the official 9 sections:

1. **Browser Options:** Toggle button (`◀`/`▶`) and config icon.
2. **Tempo & Metronome:** Link toggle, Tap tempo, BPM LCD well with up/down drag nudge, Time Signature LCD well (`4/4`), Metronome toggle with authentic dual-circle icon (`○ ●`).
3. **Scale Settings:** Live 12 Scale Mode button with Root note and Scale Type selector dropdowns reflecting selected clip.
4. **Follow & Arrangement Position:** Follow toggle, Arrangement Position LCD well (`Bar . Beat . 16th`) with draggable scrubbing.
5. **Transport Controls:** Green polygon Play triangle (`▶`), square Stop (`■`), circular Record (`●`).
6. **Arrangement Loop Controls:** Loop toggle, Punch-In marker toggle, Punch-Out toggle, Loop Start LCD well, Loop Length LCD well.
7. **Overdub, Automation, MIDI Capture:** MIDI Overdub (`+`), Automation Arm, Re-enable Automation, Capture MIDI button.
8. **Tools:** Draw mode (`✎`), Computer MIDI Keyboard (`⌨`), KEY map, MIDI map, CPU load meter with horizontal bar and percentage readout.
9. **View Selectors:** Session View (`|||`) / Arrangement View (`☰`) selector, Stacked View toggles (`▲` Clip View, `▲` Device View, `▲` Mixer View).

## 4. Files to Inspect

- `crates/livecraft-gui/src/transport.rs`
- `crates/livecraft-gui/src/widgets.rs`
- `docs/ableton-live-12-reference-manual.pdf` (pages 34-36)

## 5. Files to Modify

- `crates/livecraft-gui/src/transport.rs`
- `crates/livecraft-gui/src/widgets.rs`

## 6. What NOT to Touch

- Do NOT touch `session.rs` or `mixer.rs`.
- Do NOT touch `livecraft-dsp/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/widgets.rs`:
   - Refine `live_lcd_box`: ensure dark recessed well (`#141414`), 1px subtle border (`#333333`), amber active text (`#ffad56`), monospace font, and vertical drag increment/decrement.
   - Refine `live_toggle_button`: flat compact button with 2px rounding, subtle hover brightening, and illuminated amber/blue/red state.
   - Implement authentic Play triangle polygon and dual-circle metronome glyphs.
2. In `crates/livecraft-gui/src/transport.rs`:
   - Group layout into 9 distinct horizontal subsections separated by 1px vertical dividers (`live_divider`).
   - Implement `TransportBarResponse` returning flags for every interactive toggle.
   - Wire CPU load calculation to real CPU timer or synthetic load average.
3. Verify GUI builds and unit tests pass.

## 8. Verification Command

```bash
cargo test -p livecraft-gui
```

## 9. Acceptance Criteria

- [ ] Control Bar contains all 9 official Ableton Live 12 sections in correct horizontal order.
- [ ] BPM LCD, Time Sig LCD, and Arrangement Position LCD support drag-scrubbing.
- [ ] View switchers toggle Session vs Arrangement and Stacked Views cleanly.
- [ ] Desktop GUI compiles without errors.

## 10. Failure Handling Protocol

If Control Bar overflows on smaller window widths:

- Wrap non-essential tool toggles (KEY/MIDI map) or enable responsive scrollable row.
