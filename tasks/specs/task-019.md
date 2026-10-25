# Task Specification: task-019

## 1. Metadata

- **ID:** `task-019`
- **Phase:** Phase 4: Ableton Live 12 Desktop Interface Parity
- **Title:** Mixer Channel Strips & Calibrated 24-Segment Peak/RMS VU Meters
- **Dependencies:** `task-018`
- **Manual Grounding:** Ableton Live 12 Manual Chapter 18 (Mixing, pp. 394-400)

## 2. Context & Problem Statement

In `crates/livecraft-gui/src/mixer.rs`, the channel strips suffer from several shortcomings:

1. In / Auto / Off monitoring buttons are dummy stubs.
2. Volume fader lacks standard Ableton Live 12 cap graphics and double-click 0dB reset.
3. VU meters only render simple green rects rather than authentic Ableton 24-segment dual peak and RMS meters with calibrated tick marks (+6, +3, 0, -6, -12, -24, -inf) and red clip indicators.
4. Pan knob lacks Split Stereo Pan mode toggle.

## 3. Scope of Work

1. Wire up In / Auto / Off monitoring toggle buttons directly to `trk.monitoring`.
2. Connect Track Activator (Amber number), Solo ('S' blue), and Arm ('●' red) with keyboard shortcuts (`0`, `S`, `C`).
3. Rebuild `live_stereo_meter` in `widgets.rs`:
   - Dual-channel (L and R) vertical meters.
   - 24 segmented bars: green below -12 dB, yellow/amber between -12 dB and 0 dB, bright red above 0 dBFS.
   - Dual-readout: solid bright inner bar for RMS, thin peak needle on top with 1.5-second peak hold.
   - Red square clip indicator at the top of the meter that stays lit upon clipping until clicked.
4. Vertical volume fader with authentic Ableton cap (flat rectangular tab with center score line) and dB scale markings.

## 4. Files to Inspect

- `crates/livecraft-gui/src/mixer.rs`
- `crates/livecraft-gui/src/widgets.rs` (lines 110-180)

## 5. Files to Modify

- `crates/livecraft-gui/src/mixer.rs`
- `crates/livecraft-gui/src/widgets.rs`

## 6. What NOT to Touch

- Do NOT touch `session.rs` or `arrange.rs`.
- Do NOT touch `livecraft-dsp/`.

## 7. Step-by-Step Implementation Instructions

1. In `crates/livecraft-gui/src/widgets.rs`:
   - Implement `live_segmented_meter(ui, peak_l, peak_r, rms_l, rms_r, size)`:
     - Render 24 discrete segments separated by 1px gaps.
     - Color mapping:
       - Segments 0..15 (below -12 dB): Green (`#38d855`)
       - Segments 16..21 (-12 to 0 dB): Amber/Orange (`#ffa844`)
       - Segments 22..23 (above 0 dB): Red (`#ff4848`)
     - Draw peak hold needle (1px white line) persisting for 100 frames.
     - Render clipping indicator box at top of meter.
2. In `crates/livecraft-gui/src/mixer.rs`:
   - Wire In / Auto / Off buttons to `trk.monitoring`. When clicked, update `trk.monitoring` and dispatch parameter message to audio engine.
   - Implement numeric volume box with click-to-edit manual value entry.
   - Double-clicking volume fader resets volume to `0.0 dB`.
3. Verify GUI builds and unit tests pass.

## 8. Verification Command

```bash
cargo test -p livecraft-gui
```

## 9. Acceptance Criteria

- [ ] In/Auto/Off buttons toggle track monitoring state accurately.
- [ ] 24-segment stereo peak and RMS meters render with peak hold and clip indicators.
- [ ] Double-click on fader resets to 0.0 dB.
- [ ] Track Activator, Solo, and Arm illuminate with authentic colors.

## 10. Failure Handling Protocol

If clipping indicator stays lit indefinitely:

- Implement a click listener on the clip box that resets the peak hold state to zero.
