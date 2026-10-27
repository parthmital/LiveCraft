# UI Fidelity

Purpose: how LiveCraft's UI is made and proven indistinguishable from Live 12.4. Read when: expanding M12 to M24 or any milestone that adds UI, or changing theme tokens.
Last verified: 2026-10-27.

## Sources Of Truth

1. **Owner captures of the installed Live 12.4** (primary, pixel accurate). Stored only in `.agent-local/reference/live/` (Git-ignored), never committed, never uploaded.
2. **Manual page renders** (secondary, layout and labels only; screenshots in the manual are small and scaled). Rendered with `tools/manual.py render` into `.agent-local/reference/manual/`.
3. **Manual text** for behaviour, labels, and value ranges.

## Capture Protocol (HUMAN, defined in M12)

The owner captures Live with these fixed conditions so captures are comparable:

- Windows display scaling 100 percent; Live Settings > Display: Zoom 100 percent; window maximised on a 1920x1080 display (or the Live window resized to exactly 1920x1080 client area).
- Themes: capture each state in Live's default theme (the one a fresh 12.4 install selects) and in the light theme closest to the manual's screenshots. The owner writes the exact theme names from Live's Settings > Theme & Colors into each sidecar. Other themes are measured later from captures only; Live's theme files are never copied into the repository.
- File naming: `<area>__<state>__<theme>.png`, for example `control-bar__stopped__dark.png`.
- One LiveCraft-authored `.als` per state (written by M09 tooling or by hand by the owner in Live from the listed steps), so both apps show the same content.
- A JSON sidecar per capture: `{ "live_version": "12.4.x", "zoom": 100, "theme": "...", "window": [1920, 1080], "steps": "..." }`.

The full list of required states is created in M12 from manual chapters 3 to 31 (at least: empty set in Session and Arrangement, each Control Bar state, Browser per category, Clip View audio and MIDI, Device View with each built-in device at default settings, Mixer sections shown, every context menu in chapter 41, every Settings tab).

## Measurement

- Tokens are measured from captures with `tools/ui_measure.py` (created in M12): colour picking at listed coordinates, run-length measurement of borders and spacing, and text bounding boxes for font metrics.
- Every measured token is recorded in `crates/livecraft-ui/src/theme/<theme>.rs` with a comment naming the capture file and coordinates it came from.
- Sizes are stored in logical points at 100 percent zoom; all drawing multiplies by the zoom factor.

## Font

Live uses a proprietary typeface that cannot be redistributed. M13 selects the substitute:

1. Candidates (all SIL OFL 1.1): Jost, Futura-alike geometric sans; Inter; IBM Plex Sans; Source Sans 3; Roboto Flex.
2. Render the same labels Live shows in reference captures (track names, device labels, numbers in number boxes) at Live's sizes with each candidate.
3. Score = mean absolute pixel error of glyph masks plus advance-width error; lower wins. The task records all scores in `DESIGN.md`.
4. The owner confirms the winner (D-OWN-4). Until confirmed, the winner is used.
5. Digits must be tabular (fixed width) in number boxes; use the font's tabular figures feature or a separate monospaced digit font chosen by the same score.

## Comparison And Thresholds

`tools/ui_diff.py` (M12) compares a LiveCraft capture with the matching Live capture region by region:

| Check                                   | Pass threshold                                                           |
| --------------------------------------- | ------------------------------------------------------------------------ |
| Region bounding boxes (panels, widgets) | every edge within 1 px at 100 percent zoom                               |
| Solid colour regions                    | each channel within 2 of 255                                             |
| Structural similarity (SSIM, grey)      | at least 0.97 per region; at least 0.95 for text-heavy regions           |
| Text                                    | same strings, same alignment, baseline within 1 px                       |
| Interaction                             | same hover, pressed, selected, disabled visuals for every state captured |

A region failing any threshold keeps its parity row at In Progress.

## Behaviour

- Every mouse and keyboard interaction described in the manual for a widget is implemented: click, double click, drag with and without modifiers, fine drag (Shift in Live), reset (Delete or double click per the manual), typing values into number boxes, arrow-key nudges, scroll wheel, context menus.
- Number box drag sensitivity, acceleration, and value formatting are measured by the owner (HUMAN) by dragging a known distance in Live and recording the value change (M13).
- Animations Live uses (clip progress, blinking launch states, meter ballistics) are matched in timing; meter ballistics measured from a capture video by the owner (M13).
