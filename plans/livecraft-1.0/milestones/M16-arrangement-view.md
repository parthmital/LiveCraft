# M16: Arrangement View

Purpose: scope, decisions, and acceptance criteria for Arrangement View parity. Read when: expanding or reviewing M16.
Phase: 4. Depends on: M15. Manual: 6 Arrangement View (165 to 186: 6.1 layout, 6.2 navigation and zooming, 6.3 transport, 6.4 locators, 6.5 time signature changes, 6.6 loop, 6.7 moving and resizing clips, 6.8 fades and crossfades, 6.9 selecting clips and time, 6.10 editing grid, 6.11 time commands, 6.12 splitting, 6.13 consolidating, 6.14 linked-track editing, 6.15 the mixer in Arrangement View). Parity rows: 6.1 to 6.15.

## Goal

Arrangement View looks and behaves like Live's: timeline, ruler, Overview, scrubbing, zoom, locators, time signature markers, loop and punch braces, clip editing with grid, fades and crossfades, time selection commands, consolidation, linked-track editing, and the in-view mixer.

## Decisions

- Timeline coordinate system in `Beats` with a zoom of pixels per beat, matching Live's zoom limits measured from captures.
- Grid follows manual 6.10: adaptive and fixed grid, triplet, snap toggle, and keyboard shortcuts from the M14 table.
- Audio clip fades and crossfades (manual 6.8) are model data rendered by the engine with Live's fade curve shape; shape measured with an oracle render (HUMAN).
- Consolidate (6.13) renders audio through the M08 offline path for audio tracks and merges notes for MIDI tracks.
- Linked-track editing (6.14): link groups in the model; edits on one track apply to linked tracks via transactions.

## Task Outline

1. Ruler, Overview, and scroll and zoom behaviour.
2. Track lanes and headers (with the in-view mixer section of 6.15).
3. Clip rendering (MIDI note preview, audio waveform from M11 overviews).
4. Locators and time signature markers.
5. Loop and punch braces.
6. Clip move and resize with grid snapping; slip editing.
7. Fades and crossfades (model, engine, UI); HUMAN oracle for curve shape.
8. Clip and time selection model.
9. Time commands (insert, delete, duplicate, cut time) and splitting.
10. Consolidate.
11. Linked-track editing.
12. Milestone verification: Arrangement states pass `ui_diff.py`; manual-quote tests for every 6.x behaviour.

## Acceptance Criteria

- Every Arrangement state in `capture-list.md` passes thresholds.
- Fade oracle render meets the nonlinear thresholds in `QUALITY-GATES.md#audio`.
- Parity rows 6.1 to 6.15 Done with evidence.
