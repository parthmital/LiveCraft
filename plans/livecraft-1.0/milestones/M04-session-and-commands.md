# M04: Session, command registry, undo

Purpose: scope, decisions, and acceptance criteria for the UI-independent app core. Read when: expanding or reviewing M04.
Phase: 1. Depends on: M03. Manual: 5.4.2 Accessing a Set's Undo History (135 to 136), 42.5 Editing (1017). Parity rows: 5.4 (undo part).

## Goal

Every model change goes through a named, serialisable command with undo; the GUI and CLI dispatch commands instead of mutating the model; the Undo History view data exists.

## Decisions

- New crate `livecraft-session` (dependencies: time, model, formats, engine). `LAYERS`: ui now depends on session, model, time only (not engine); livecraft app on session, ui.
- `trait Command: Send { fn name(&self) -> &'static str; fn apply(&mut self, set: &mut LiveSet) -> Result<(), CommandError>; fn undo(&mut self, set: &mut LiveSet); fn merge(&mut self, next: &dyn Command) -> bool { false } }`. Commands store what they need to undo inside themselves after `apply`.
- Registry: `CommandRegistry` maps stable dotted names (`track.create_midi`, `clip.set_loop`, `device.set_param`) to constructors from `serde_json::Value` arguments, so the CLI and the control server (M42) can call any command. Each command module registers its commands.
- Transactions: `session.transaction("Duplicate Clips", |tx| { ... })` groups commands into one undo step, labelled like Live's Undo History entries.
- Continuous gestures (fader drag) use `merge` keyed by target and a 500 ms gap, so one drag is one undo step.
- `Session` owns `LiveSet`, `UndoHistory` (default limit 1000 steps, configurable), `Selection`, `Clipboard`, `EngineHandle`, and a dirty flag. After each applied command it notifies the engine sync layer (structural change or parameter change) which M05 implements; until M05 it calls the existing `sync_project`.
- Undo History: list of labels, newest first, with "revert to here" and "reapply to here" (manual 5.4.2).

## Task Outline

1. Create `livecraft-session` with `Command`, `CommandError`, `UndoHistory`, and tests (apply, undo, redo, limit, merge).
2. `CommandRegistry` with JSON argument decoding and a test that every registered command round-trips its arguments.
3. Port today's `livecraft_model::command` actions into the new trait and delete the old module.
4. Track commands (create audio, MIDI, return, group; delete; rename; colour; reorder; arm, solo, mute, monitor).
5. Clip and slot commands (create, delete, duplicate, move, rename, loop and markers, notes add, remove, edit).
6. Device and parameter commands (insert, remove, move, enable, set param with merge).
7. Scene and transport commands (create, delete, duplicate scene; tempo; time signature; loop; metronome).
8. Transactions and Undo History data model (revert and reapply to a point).
9. Route every GUI mutation through the session (one view per task: control bar, session view, mixer, arrangement, device rack, piano roll, browser).
10. CLI subcommands `livecraft command list` and `livecraft command run <set.als> <name> <json-args> -o <out.als>`.
11. Milestone verification and docs.

## Acceptance Criteria

- `grep -rn "self.project\.[a-z_.\[\]]* = \|\.push(" crates/livecraft-ui/src` finds no direct model mutation (only command dispatch).
- Property test: random command sequences, then undo all, give a set equal to the start (10,000 cases).
- `livecraft command list` prints every registered command name; the count is recorded in the review.
