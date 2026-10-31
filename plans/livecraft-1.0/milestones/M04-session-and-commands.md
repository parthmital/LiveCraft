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

## Review (2026-10-31)

Every Outline item has its own commit, except items 9 and 10, which share one (see deviations). Each commit passed `cargo xtask ci` before it was pushed to `main`.

| Item | Commit | Summary |
| --- | --- | --- |
| 1 | `23e7232` | `livecraft-session` with `Operation`, `Command`, `CommandError`, and `UndoHistory` (7 tests) |
| 2 | `33af3b4` | `CommandRegistry` with JSON arguments and a round trip test |
| 3 | `d4269f4` | M01 actions ported (automation points, tempo, track create and volume). `model/command.rs` deleted |
| 4 | `41c6559` | Track commands: create return and group, delete with exact undo, rename, colour, reorder, arm, solo, mute, monitor, pan, send |
| 5 | `c9c4571` | Clip, slot, and note commands |
| 6 | `4f36477` | Device and parameter commands (`ParamValues::remove` added to the model) |
| 7 | `ec5d4d3` | Scene and transport commands, the scene row insert, and the random sequence property test |
| 8 | `f78fe0a` | Transactions, the Undo History entries, and `go_to` (revert and reapply) |
| 9 and 10 | `0418a9b` | Every view routed through the session; `livecraft command list` and `livecraft command run` |
| 11 | this commit | Verification, docs, owner row Q08, checklist `[~]` |

### Manual quotes

From `docs/ableton-live-12-reference-manual.pdf`, read with `plans/livecraft-1.0/tools/manual.py`. Renders stayed in `.agent-local/`.

- 5.4.2, page 135: "The Undo History lists all the actions taken since opening a Set and lets you revert or reapply them up to a specific point." Used by `UndoHistory::entries` and `go_to`.
- 5.4.2, page 136: "Once opened, you will see all the actions in the order they occurred, with the newest at the top and the oldest at the bottom." Used by the entry order.
- 5.4.2, page 136: "The actions listed above the selected action are then greyed out to indicate that they have been undone." Used by `HistoryEntry::undone`.
- 5.4.2, page 136: "The actions listed below the selected action remain active, as their changes still apply." Used by `go_to`, which undoes only the steps newer than the selected entry.
- 5.4.2, page 136: "You can select a greyed out action to reapply it along with any other undone actions below it." Used by `go_to` for undone entries.
- 5.4.2, page 136: "Creating or opening a Set is treated as the first action in the Undo History and therefore cannot be undone." Used by `OPENING_LABEL`, the last entry, which no undo reaches.
- 5.4.2, page 136: "Note that the Undo History is not saved with a Set once it is closed and is refreshed each time the Set is opened." The history lives in memory and is not written to `.als`.
- 42.5, page 1017: the Editing table lists Cut, Copy, and Paste (Ctrl+X, Ctrl+C, Ctrl+V). Clipboard commands are not in M04 (see deviations), so nothing from 42.5 is quoted as implemented.

### Decisions and deviations

1. `Operation::perform` and `Command::apply` take the session's `IdAllocator` as well as the Set. The outline's `apply(&mut self, set)` cannot keep ids unique after a deletion, because `Project::id_allocator()` is recomputed from the Set each time.
2. The milestone's "LiveSet" is `Project`, as M03 deviation 2 records.
3. No clipboard. Copy and paste have no GUI path yet, so `Session` has no clipboard field. M15 and M17 add it with their views.
4. `Session` holds `Option<AudioEngine>` directly, not a separate `EngineHandle` type.
5. Items 9 and 10 are one commit. The CLI's `Session` use and the app shell change in the same files, and the lockfile records both the UI and app dependency edges, so the two could not be split cleanly.
6. Views return `Edits` (a `Vec` of boxed commands), not the `Vec<Intent>` the target architecture names. An intent that changes the Set is a command, so one type serves both.
7. One frame of edits is one undo step (`Session::apply`), and a single command can merge into the step before it. This keeps a note drag, a fader drag, and a multi-note transform one step each.
8. Playhead, play, stop, record, and track selection are session or view state, not commands. They are not undo steps, as Live's Undo History does not hold them.
9. `scale.set` was added to the registry. The transport bar and the piano roll change the scale, and the outline has no command for it.
10. `track.set_parent` is not registered. No view changes group membership until M20, so the registry does not need it yet.
11. Notes and automation points are addressed by index. They have no ids until M18 and M22. Clips are addressed by `ClipId`. A slot appears only as the row where a clip is created or moved to, which is a grid position. A send is addressed by its return track's id.
12. `parse_hex_color` moved to `livecraft-model`, and `livecraft-formats` re-exports it. The UI needed it without the formats crate. Non-ASCII input now returns white instead of panicking on a byte slice.
13. Layering: `livecraft-session` depends on time, model, formats, and engine. `livecraft-ui` depends on model, session, and time. The app depends on model, formats, engine, devices, session, and ui. The app keeps the first four for the CLI's offline bounce and inspection, which the milestone does not forbid.
14. Owner-only input: Q08, the Undo History labels and the name of the opening entry in Live 12.4. The labels in code are provisional until Q08 arrives.

### Behaviour changes (intentional)

- Undo and redo go through the session, and Ctrl+Z and Ctrl+Y report a refused redo in the status bar.
- Saving reports errors in the status bar. Before, the save result was ignored.
- Commands refuse values the Set cannot hold: a tempo outside 20 to 999 BPM, a non-finite volume, pan outside -1 to 1, a send outside 0 to 1, a colour that is not `#RRGGBB`, a note pitch above 127, a note velocity outside 1 to 127, and a note probability outside 0 to 1.
- An automation command on a missing track, lane, or point is an error. Before, it was ignored.
- Moving an automation point re-sorts the lane by beat. Before, the lane could stay unsorted, which broke `evaluate_at`.
- The engine receives every change after each command. Before, the app sent mixer values by diff and sent the Set to the engine only when the track count changed, so a device added from the browser was not sent until the next track change.
- `track.delete` restores the other tracks' group parents, routing, and sends on undo.
- A new return track gets a send on every track (`Project::add_return_track`).

### Evidence

- `cargo xtask ci` on the final tree passed, log `.agent-local/logs/2026-10-31_m04-11-ci.log`. Earlier logs are in `.agent-local/logs/2026-10-31_m04-NN-ci.log`.
- Tests: 203 passed, 0 failed (baseline 122). Per crate: livecraft 9 (characterisation 2, cli 4, command_cli 3), livecraft-model 44 (unit 25, edit 11, validation 8), livecraft-formats 6, livecraft-dsp 1, livecraft-devices 18, livecraft-engine 10, livecraft-session 74 (unit 12, clips 12, commands 9, devices 9, random_sequences 1, scenes_and_transport 9, session 6, tracks 11, transactions 5), livecraft-time 34, livecraft-ui 2, xtask 5, livecraft-testkit 0.
- `livecraft command list` prints 42 names, stored in `.agent-local/output/m04/command-list.txt`. The `command_cli` test compares the printed list with the registry.
- GUI launch: `livecraft gui --screenshot .agent-local/output/m04/gui-session-dark.png --view session --theme dark` exited 0, and the frame shows the demo chord clip, the mixer strips, and the Operator device view.

### Criteria

| Criterion | Met | Evidence |
| --- | --- | --- |
| `grep -rn "self.project\.[a-z_.\[\]]* = \|\.push(" crates/livecraft-ui/src` finds no direct model mutation | Yes | No assignment to the Set remains, and views take `&Project`. The `.push(` hits are all `edits.push(...)` or `action.edits.push(...)`, which add a command to the list the app applies, with one exception: `points.push(Pos2::new(px, py))` in `devices.rs` builds the screen points of the EQ curve. |
| Property test: random command sequences, then undo all, equal the start (10,000 cases) | Yes | `crates/livecraft-session/tests/random_sequences.rs` runs 10,000 cases of 1 to 29 generated commands from 26 kinds, including invalid ones. After every step the Set must validate, and after undoing every step the Debug form of the Set must equal the start. The test passed in about 4.5 s. A mutation check that broke `SetTrackVolume::revert` made it fail with a minimal failing input, and the operation was restored. |
| `livecraft command list` prints every registered command name; count recorded | Yes | 42 names (see Evidence). The count is also in the README. |

### Owner rows

- Q08: the Undo History labels for the commands, and the name of the opening entry, in Live 12.4. The line is `[~]` until Q08 arrives.

### Not verified

- GUI interactions were not exercised. The screenshot check shows one rendered frame only. Clicks, drags, the piano roll tools, the browser insert, and the transport controls compile and pass clippy, but no test or run drives them.
- No sound was heard, and no audio output was checked against the old engine path.
- The Undo History labels and the opening label are not checked against Live (Q08).
- The 10,000-case property test runs under the debug profile, as `cargo test` does by default.
- Known issue, not fixed: the two default return tracks have 8 clip slots each, from M01. The manual (page 402) says return tracks cannot play clips. Changing it would change the characterisation snapshot, so it is left for a decision.
- Known issue, not fixed: `track.set_parent` has no command (deviation 10).
- Clipboard commands are not implemented (deviation 3).
- The engine sync is still a whole-Set `sync_project` plus per-value messages, until M05 replaces it.
