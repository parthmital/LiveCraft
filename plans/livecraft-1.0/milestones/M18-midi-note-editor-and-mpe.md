# M18: MIDI Note Editor and MPE

Purpose: scope, decisions, and acceptance criteria for MIDI editing and MPE. Read when: expanding or reviewing M18.
Phase: 4. Depends on: M17. Manual: 10 Editing MIDI (252 to 292: 10.1 layout, 10.2 zoom and navigation, 10.3 creating a clip, 10.4 adding notes, 10.5 editing notes with non-destructive editing, selecting, find and select, moving, length, note stretch, deactivating, note operations, pitch and time utilities, MIDI tools access, quantizing, velocities, probabilities, 10.6 folding and scales, 10.7 editing clips, 10.8 multi-clip editing), 12 Editing MPE (330 to 339). Parity rows: 10.1 to 10.8, 12.1 to 12.6.

## Goal

The MIDI Note Editor, velocity and probability lanes, fold and scale display, multi-clip editing, and MPE viewing and editing match Live; the engine plays note probability, velocity ranges, and MPE data.

## Decisions

- Replaces today's `pianoroll.rs` with `crates/livecraft-ui/src/views/note_editor/` (grid, keyboard, lanes, selection, tools).
- Note probability and velocity deviation are evaluated in the engine at note start with the set's seeded PRNG (deterministic renders).
- MPE: per-note pitch bend, slide (CC 74), and pressure curves stored on notes (`MpeData` breakpoint lists); engine sends per-note expression to MPE-capable devices and plug-ins (M37), and to MIDI outputs as MPE (12.6 settings).
- Find and Select (10.5.3) is a query language implemented exactly as the manual documents; tests quote each example.

## Task Outline

1. Editor grid, keyboard, zoom and navigation (10.1, 10.2).
2. Create clip, add and delete notes with draw mode and grid (10.3, 10.4).
3. Selection, time selection, find and select (10.5.2, 10.5.3).
4. Move, length, stretch, deactivate (10.5.4 to 10.5.7).
5. Note operations and pitch and time utilities (10.5.8, 10.5.9).
6. Quantize dialog and behaviour (10.5.11).
7. Velocity lane and velocity ranges (10.5.12); probability lane (10.5.13); engine evaluation.
8. Fold and scale display (10.6); clip editing commands (10.7).
9. Multi-clip editing (10.8).
10. MPE viewing and editing (12.1 to 12.3); MPE in devices and MIDI out settings (12.4 to 12.6).
11. Milestone verification.

## Acceptance Criteria

- Note Editor states pass `ui_diff.py`; manual-quote tests for 10.x and 12.x behaviours.
- Deterministic probability test: same seed, same render; statistical test over 10,000 notes within 1 percent of the set probability.
- Parity rows 10.x and 12.x Done with evidence.
