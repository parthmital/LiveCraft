# M15: Session View

Purpose: scope, decisions, and acceptance criteria for Session View parity. Read when: expanding or reviewing M15.
Phase: 4. Depends on: M14. Manual: 3.6 Arrangement and Session (40 to 41), 7 Session View (187 to 199: 7.1 clips, 7.2 tracks and scenes, 7.3 track status fields, 7.4 setting up the grid, 7.5 recording sessions into the Arrangement), 16 Launching Clips (360 to 371, UI side). Parity rows: 3.6, 7.1 to 7.5, 16.1 to 16.7.

## Goal

Session View looks and behaves like Live's: clip grid, clip slot states and progress, launch and stop buttons, scenes and scene properties, track headers and status fields, selection and editing, drag and drop, and recording Session play into the Arrangement.

## Decisions

- The view is a pure function of `SessionSnapshot` plus `SessionViewState` (scroll, selection, hover, drag), emitting intents; all edits are M04 commands.
- Clip slot states (empty, has clip, playing, queued, recording, stop button removed, armed) and blinking or progress animations are timed from the engine snapshot, not the UI frame clock, so they stay in sync with audio.
- Track status fields (manual 7.3) show clip play position, loop count, and follow action indicators.
- Scene tempo and time signature in scene names follow manual 7.2 naming rules.
- Recording Session into Arrangement (manual 7.5) uses the M07 transport and the M21 recorder when M21 exists; until then the task records launches as arrangement clips (MIDI and audio clip references, no new audio).

## Task Outline

1. Grid layout (tracks, slots, scenes column, Main track) matching captures; scrolling and zoom.
2. Clip slot rendering for every state, with snapshot tests.
3. Launch, stop, scene launch, Stop All Clips interactions with launch quantization display.
4. Track headers, activator, solo, arm, monitoring, colour, rename, fold; track status fields.
5. Selection model (cells, rows, columns, rectangles) and editing commands (copy, cut, paste, duplicate, delete, insert scene, capture and insert scene).
6. Drag and drop of clips between slots and tracks; drag from the Browser (stub until M24).
7. Scene properties (name, colour, tempo, time signature) and the scene launch behaviour.
8. Follow Action UI in clip properties (engine from M07).
9. Record Session into Arrangement (manual 7.5) and Back to Arrangement button.
10. Milestone verification: Session states from the capture list pass `ui_diff.py`; manual-quote interaction tests.

## Acceptance Criteria

- Every Session state in `capture-list.md` passes thresholds in both themes.
- Every interaction in manual 7.1 to 7.5 and 16 has an `egui_kittest` test quoting the manual.
- Parity rows 7.x and 16.x Done with evidence.
