# LiveCraft UI/UX Design System Specification

## 1. Design Philosophy

LiveCraft implements the user interface and interaction design of Ableton Live 12 in immediate-mode Rust (egui/eframe). The design centers on high information density, structural alignment, minimal visual ornamentation, and clear operational hierarchy. Every interactive widget, layout container, color token, and typography style directly maps to the official Ableton Live 12 Reference Manual.

## 2. Spatial Layout and Window Structure

The primary window is structured into five distinct functional panels:

1. Control Bar (Top, 36 px fixed height): Transport controls, BPM, time signature, metronome, quantization, Live 12 Scale Mode selector, position readout, loop locators, MIDI/Key mapping triggers, CPU load meter, and View Switcher.
2. Browser Sidebar (Left, 200 px fixed width): Search box, Collections with 7 Ableton color-coded tags, Categories, Places, and expandable device and instrument catalogs.
3. Main Work Area (Central Panel, fills remaining width and height):
   - Session View: Vertical track columns combining clip slot launch matrix (8 scene rows), clip stop row, monitoring toggles, aux sends, stereo panning, 100 px volume faders, peak VU meters, track activators, solo, arm, and track color stripes.
   - Arrangement View: Linear horizontal timeline on the left with bar/beat grid, clip cards with note and waveform visualization, playhead marker, and docked Track Header control strips on the right.
4. Detail View (Bottom, 180 px fixed height): Multi-tab container switching between Device View (device rack chains with modular controls) and Clip View (piano roll and Live 12 MIDI transformation tools).
5. Status Bar (Bottom, 22 px fixed height): Contextual help tips, engine sample rate, bit depth, and active project state.

## 3. Color Tokens and Themes

LiveCraft features both Dark Neutral and Light themes with authentic Ableton Live 12 color values:

| Token Name           | Dark Theme Value | Light Theme Value | Usage                                          |
| -------------------- | ---------------- | ----------------- | ---------------------------------------------- |
| `desktop`            | `#1a1a1a`        | `#9e9e9c`         | Deep window canvas background                  |
| `surface_area`       | `#282828`        | `#bfbfbd`         | Main workspace panels and lane backgrounds     |
| `surface_highlight`  | `#383838`        | `#d4d4d2`         | Selected tracks, hover states, card headers    |
| `control_background` | `#1a1a1a`        | `#a8a8a6`         | Device bodies, knob backgrounds, trough wells  |
| `control_foreground` | `#c8c8c8`        | `#181818`         | Primary labels, button text, values            |
| `control_disabled`   | `#5a5a5a`        | `#6e6e6c`         | Inactive controls, placeholder text            |
| `amber`              | `#ffad56`        | `#ff9800`         | Ableton Signature accent, playhead, activators |
| `play_green`         | `#2ce562`        | `#1ed450`         | Play transport button, scene launch triangles  |
| `record_red`         | `#ff4f4f`        | `#ff3b3b`         | Record arm buttons, recording indicators       |
| `solo_blue`          | `#2f77ee`        | `#2668de`         | Track solo buttons                             |
| `cyan`               | `#00cade`        | `#00b0c8`         | Audio track accents, filter cutoff parameters  |
| `meter_green`        | `#28e058`        | `#20d050`         | VU meter normal range (-inf to -6 dB)          |
| `meter_yellow`       | `#ffd036`        | `#f0c020`         | VU meter warning range (-6 to 0 dB)            |
| `meter_red`          | `#ff4545`        | `#ff3838`         | VU meter clip overload (> 0 dB)                |
| `panel_border`       | `#353535`        | `#8e8e8c`         | Dividing outlines and panel separators         |
| `lcd_bg`             | `#141414`        | `#222222`         | Recessed LCD numerical readout boxes           |
| `lcd_text`           | `#dddddd`        | `#eeeeee`         | High-contrast monospace digits in LCDs         |
| `clip_slot_bg`       | `#1c1c1c`        | `#c2c2c0`         | Session matrix slot backgrounds                |
| `clip_slot_border`   | `#2c2c2c`        | `#a4a4a2`         | Clip slot boundary outlines                    |
| `fader_trough`       | `#141414`        | `#9a9a98`         | Volume fader travel wells                      |
| `fader_cap`          | `#5a5a5a`        | `#d8d8d6`         | Physical metallic fader caps                   |
| `ruler_bg`           | `#202020`        | `#b4b4b2`         | Timeline and time ruler backgrounds            |

Theme switching can be toggled live at runtime using `F8`, switching all panels and custom vector-painted widgets synchronously.

## 4. Component and Widget Standards

### Rotary Knobs (`live_knob`)

- Outer diameter: 24 to 28 px
- Value range: 270 degree sweep from 7:30 o'clock to 4:30 o'clock
- Modes: Unipolar (0..max starting from 7:30 o'clock) and Bipolar (-max..+max starting from 12 o'clock center for Pan and EQ gain)
- Indicators: Recessed circular well, active 36-step antialiased colored arc matching parameter family, needle pointer line, and theme-adaptive bottom text label.

### Volume Faders (`live_fader`) and Peak Meters (`live_stereo_meter`)

- Travel height: 100 px
- Scale: Piecewise tapered mathematical mapping matching Ableton Live faders (`db_to_norm` / `norm_to_db`):
  - +6.0 dB to 0.0 dB: Top 20% of fader travel (norm 0.80..=1.00)
  - 0.0 dB to -12.0 dB: Middle 30% of fader travel (norm 0.50..=0.80)
  - -12.0 dB to -24.0 dB: Lower 20% of fader travel (norm 0.30..=0.50)
  - -24.0 dB to -60.0 dB: Bottom 30% of fader travel (norm 0.00..=0.30)
- Ticks: Calibrated tick marks at exactly +6, 0, -6, -12, -24, and -inf dB sharing identical vertical alignment between fader ticks and stereo meter ticks
- Cap: Metallic cap with 1 px highlight and center tracking groove
- Double-click action: Reset immediately to unity gain (0.0 dB)

### View Mode Segmented Capsule (`live_view_mode_toggle`)

- Top right transport pill switching between Session View (`|||`) and Arrangement View (`===`)
- Custom vector-drawn icons avoiding system font and unicode glyph dependencies

### Monitoring Mode Selector (`render_monitoring_section`)

- 3-state radio toggle buttons: `In`, `Auto`, `Off`
- Active state highlighted in signature Amber (`#ffad56`) with dark label

### Arrangement Timeline & Track Headers

- Left timeline: Responsive beat-grid scaling adapting to viewport width, playhead line spanning all tracks, loop brace bracket
- Right track headers: Pinned and visible at right margin with track number, activator toggle, solo, record arm, volume/pan readout, and track color badge
- Top locator toolbar: "Set" marker button with cue pin icon docked right above track headers

## 5. Accessibility and Typography

- Monospace digits: Used in all numerical readouts (BPM, position, volume dB, frequency, time signature) to prevent layout jitter during value adjustment.
- Proportional text: Used for track labels, browser item descriptions, and device parameter titles.
- Minimum tap target: All buttons maintain at least 18x18 px interactive hitboxes.
