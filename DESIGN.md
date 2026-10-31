# Design

Purpose: how the UI looks and behaves, and how fidelity to Live 12 is achieved and checked. Read when: changing any user-facing UI.
Last verified: 2026-10-27.

## Target

- The UI must be visually and behaviourally indistinguishable from Ableton Live 12.4 at the same window size, zoom level, and theme, except for branding.
- Fidelity is measured, never estimated: every size, colour, and font metric comes from a reference capture of Live and is recorded with its source.
- The full fidelity process and the reference capture list are in `plans/livecraft-1.0/reference/UI-FIDELITY.md`.

## Tokens

- Single home for values: `crates/livecraft-ui/src/style.rs` (`LivePalette::dark`, `LivePalette::light`). Do not copy values into docs.
- Status on 2026-10-27: current values are estimates, not measured against Live. The plan replaces them with measured tokens per Live theme (Dark, Light, and the other built-in themes) and a zoom factor (Live's Zoom setting, 50 to 200 percent).
- Typography: currently egui's default proportional and monospace fonts, which do not match Live. The plan selects an openly licensed font by measured metric error against reference captures (see `UI-FIDELITY.md#font`).

## Components

Built on `egui` painting primitives in `crates/livecraft-ui/src/widgets.rs`.

| Component                      | Function                   | State today                                                                            |
| ------------------------------ | -------------------------- | -------------------------------------------------------------------------------------- |
| Rotary knob                    | `live_knob`                | 270 degree sweep, unipolar and bipolar; sizes not measured                             |
| Volume fader and meter         | `live_fader`, stereo meter | Piecewise dB mapping in `db_to_norm` and `norm_to_db`; curve not verified against Live |
| LCD number field               | `live_lcd_box`             | Drag to change value; Live's number box drag and typing rules not complete             |
| Toggle button                  | `live_toggle_button`       | Coloured on state                                                                      |
| Session and Arrangement switch | `live_view_mode_toggle`    | Vector icons                                                                           |

## Patterns

- Icons: drawn as vector paths in code. Never bitmaps copied from Live.
- Copy: use Live's exact UI terms and capitalisation from the manual.
- Layout regions match manual section 3.1 to 3.14: Control Bar, Browser, Session or Arrangement, Clip and Device View, Info View, Status Bar.
- Light and dark themes switch at runtime with `F8` (a LiveCraft debug binding; Live itself sets the theme in Settings).

## Decisions

- Custom-painted widgets on `egui` rather than native or themed controls: Live draws its own flat widgets, so only custom painting can match it.
- Measured tokens with recorded sources: removes guesswork and makes visual regressions testable.
- No Ableton fonts, icons, or theme files in the repository: they are proprietary (see `AGENTS.md#clean-room-rules`).
