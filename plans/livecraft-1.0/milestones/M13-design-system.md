# M13: Design system: tokens, zoom, font, icons, widget kit

Purpose: scope, decisions, and acceptance criteria for LiveCraft's Live-identical widget layer. Read when: expanding or reviewing M13.
Phase: 3. Depends on: M12. Manual: 2.3 Live's Settings (Display and Input, Theme and Colors), 41 Accessibility and Keyboard Navigation (focus visuals), 42.6 Adjusting Values (1018), every device and view screenshot. Parity rows: supports all UI rows.

## Goal

A theme token set measured from Live for each built-in theme, a global zoom factor, the selected open font, a vector icon set, and a widget kit whose every state matches the reference captures; egui default widgets are no longer visible anywhere.

## Decisions

- Module `crates/livecraft-ui/src/theme/`: `tokens.rs` (`ThemeTokens` struct: colours by role, sizes, stroke widths, corner radii, font sizes), one file per theme with measured values and source comments, `zoom.rs` (all sizes multiplied by `Zoom(f32)` from 0.5 to 2.0, Live's 50 to 200 percent).
- Font selection per `UI-FIDELITY.md#font`: candidates Jost, Inter, IBM Plex Sans, Source Sans 3, Roboto Flex (all SIL OFL 1.1, fonts committed under `crates/livecraft-ui/assets/fonts/` with their licence files); task records scores in `DESIGN.md`; owner confirms (D-OWN-4). `default_fonts` feature removed from eframe; the `epaint_default_fonts` exception removed from `deny.toml`.
- Icons: vector paths in `crates/livecraft-ui/src/icons.rs`, drawn to match captures at 100 and 200 percent zoom; no bitmaps.
- Widget kit (`crates/livecraft-ui/src/widgets/`), each with states normal, hover, pressed, focused, disabled, selected, automated (Live shows automated parameters distinctly), mapped (MIDI and key map modes): toggle button, momentary button, radio group, number box (drag with Live's sensitivity and Shift for fine, typing, arrows, Delete resets to default per manual 42.6), knob, slider (horizontal and vertical), fader with meter, chooser (dropdown), tab strip, text field, scroll bars, splitter, tooltip, context menu, track header, clip slot cell, colour chooser.
- Interaction measurements (number box drag ratio, knob drag ratio, meter ballistics) come from HUMAN measurements recorded in `DESIGN.md`.

## Task Outline

1. `ThemeTokens` and zoom plumbing; current estimated values moved into it; every view reads tokens.
2. HUMAN: owner records interaction measurements (number box and knob drag per 100 px, double-click reset, meter decay times from a short screen recording) into a form in the task.
3. Measure dark theme tokens with `ui_measure.py` and replace values (one task per area: global surfaces, Control Bar, Session, Mixer, Device View, Browser, Arrangement).
4. Same for the light theme.
5. Font candidates, scoring tool, results in `DESIGN.md`; adopt the winner.
6. Icon set (Control Bar, track headers, Browser, Clip and Device View), compared at 100 and 200 percent.
7. Widgets: one or two per task with `egui_kittest` interaction tests and snapshot tests.
8. Remove every remaining egui default widget usage (grep test in CI: no `ui.button(`, `ui.checkbox(`, `ComboBox::` in views).
9. Milestone verification: widget gallery state captures match references within thresholds.

## Acceptance Criteria

- `ui_diff.py` on the widget gallery states passes every threshold in `UI-FIDELITY.md#comparison-and-thresholds` for both themes at 100 percent zoom; 200 percent zoom passes bounding boxes and SSIM 0.95.
- `DESIGN.md` lists every token with its source capture and every font score.
- No egui default widgets in views (grep check in xtask).
