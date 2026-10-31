# M03: Document model v2

Purpose: scope, decisions, and acceptance criteria for the Live Set document model. Read when: expanding or reviewing M03.
Phase: 1. Depends on: M02. Manual: 3.5 Live Sets (40), 3.7 Tracks (42 to 44), 3.8 Audio and MIDI (45), 3.9 and 3.10 Clips (46 to 47), 3.11 Devices (48 to 49), 7.2 Tracks and Scenes (189 to 193), 10.5 Editing MIDI Notes (note properties: velocity, release velocity, probability, velocity range; 259 to 283), 18.3 Group Tracks (400). Parity rows: 3.7, 3.8.

## Goal

`livecraft-model` represents everything a Live Set holds that LiveCraft implements, with typed ids, validation, and no index-based identity, split into focused modules.

## Decisions

- Modules: `ids.rs`, `set.rs` (`LiveSet`), `track.rs` (Audio, MIDI, Group, Return, Main), `clip.rs` (`Clip`, `ClipSlot`, `ArrangementClip`, audio and MIDI payloads), `note.rs`, `device.rs` (`DeviceInstance { id, kind: DeviceKind, enabled, params: ParamValues, chains: Vec<Chain> }`), `param.rs` (`ParamId(u16)`, `ParamValues` as a sorted small vector), `automation.rs`, `scene.rs`, `scale.rs`, `tuning.rs`, `routing.rs` (input and output selections as typed enums, sends), `validate.rs`.
- Track order is a `Vec<TrackId>` in the set; tracks are stored in an id-keyed map. Group membership is `parent: Option<TrackId>`, nesting allowed (Live 12 supports nested groups; confirm on page 400).
- Scenes hold `name`, `color`, optional tempo and time signature (manual 7.2).
- MIDI note fields as in `TARGET-ARCHITECTURE.md#data-model-decisions`; velocity 1 to 127 stored as `f32` for Live's fractional velocities; probability 0.0 to 1.0.
- Colours: Live's fixed clip and track colour palette is represented by index (`ColorIndex(u8)`), matching the `Color` value Live writes in `.als`; the palette size is counted from the colour chooser in a reference capture (M12) and the RGB values come from M13 measurement.
- MIDI transformation code in today's `model.rs` (quantize, humanize, and so on) moves unchanged to `livecraft-model/src/midi_tools_legacy.rs` until M19 replaces it.
- `serde` derives on every type for JSON export; JSON is not a save format (`.als` is).
- Validation: `LiveSet::validate() -> Vec<ValidationError>` checks referential integrity (every id referenced exists, group parents are group tracks, no cycles, sends count equals return count).

## Task Outline

1. Typed ids and `IdAllocator` (monotonic, seeded from `.als` `NextPointeeId` later).
2. Split `model.rs` into the modules above with no behaviour change (move only).
3. Replace `usize` and `u64` identity with typed ids in tracks, clips, scenes, devices; update engine and UI call sites.
4. Note model upgrade (release velocity, probability, velocity deviation, mute) with defaults matching Live.
5. Group tracks and nested groups; Main track naming (Live 12 says Main, not Master).
6. Routing description types (no audio behaviour yet).
7. `ParamValues` and `ParamId`; convert `Device` string-keyed params to ids with a per-device mapping table; engine adapter until M05.
8. `validate()` with property tests generating random sets.
9. Milestone verification and docs.

## Acceptance Criteria

- `grep -rn "track_index: usize\|track_idx: usize" crates` returns nothing.
- Property test: random edit sequences keep `validate()` empty (10,000 cases).
- M01 characterisation tests pass.

## Review (2026-10-31)

Evidence for every Outline item and Acceptance Criterion. Commits: `070a103` (item 1), `273b64e` (item 2), `3234aa3` (item 3), `163ed17` (item 4, with the M02 carry-over for transport), `7bfee79` (item 5), `69b8dc7` (item 6), `6f487a2` (item 7), `dbce138` (item 8), and the commit that adds this review (item 9).

### Manual quotes

Read with `plans/livecraft-1.0/tools/manual.py` from `docs/ableton-live-12-reference-manual.pdf`. Page renders stayed in `.agent-local/` and are not committed.

- Nested groups, page 400 (section 18.3): "You also use this command to nest an existing Group Track(s) within a new Group Track." Used by the group nesting and the parent field.
- Group tracks hold no clips, page 400: "Group Tracks themselves cannot contain clips, but they are similar to audio tracks in that they have mixer controls and can host audio effects." Used by `GroupHasClips` and the group clip-slot rule.
- Nested group solo, page 402: the figure caption "Solo Button of a Group Track Containing a Soloed Nested Group Track." Second source for nesting.
- Main track and return tracks, page 402 (section 18.4): "In addition to Group Tracks and tracks that play clips, a Live Set has a Main track and return tracks; these cannot play clips, but allow for more flexible signal processing and routing." Used by `TrackType::Main` and the return and Main rules.
- Group routing, page 400: "When tracks are placed into a group, their Audio To routing choosers are automatically assigned to their Group Track unless they already had a custom routing, i.e. to a destination other than Main." Not implemented yet, planned for M20.
- External input, page 375 (section 17.2): "An audio interface's inputs are selected by choosing 'Ext. In' from the Input Type chooser of an audio track. The Input Channel chooser then offers the individual input channels." Used by `AudioInput::ExternalIn`.
- Resampling, page 380 (section 17.4): "The 'Resampling' option in any audio track's Input Type chooser will route the Main output to that track." Used by `AudioInput::Resampling`.
- Tapped track routing, page 381 (section 17.5.1): "...the signal received can be tapped from one of three different points chosen from the Input Channel chooser: Pre FX, Post FX or Post Mixer." Used by `TapPoint`.
- Note velocity deviation, page 280 (section 10.5): "The Velocity Deviation slider can be used to set a range for each note's velocity. Velocity values are then chosen randomly from within the specified range each time a note is played." Used by `velocity_deviation`.
- Release velocity, page 281: "Note Off (or 'release') velocity is a somewhat esoteric parameter." Used by `release_velocity`.
- Note probability, page 282 (section 10.5.13): "Note probability determines the likelihood of a MIDI note being triggered during clip playback." Used by `probability`.
- Scenes, page 190 (section 7.2): "The horizontal rows are called scenes." Used by `Scene` and the scene-to-row map.
- Scene tempo and time signature, page 191 (section 7.2.1): scene tempo and time signature are assigned from the Main track's title header. Used by `Scene.tempo` and `Scene.time_signature`, which stay optional.
- Time signature changes, page 172 (section 6.5): "Live's time signature can be changed at any point in the Arrangement using time signature markers."
- Time signature markers, page 173: "Time signature markers are not quantized; they may be placed anywhere in the timeline, and their positioning is only constrained by the editing grid. This means that it is possible to place meter changes in 'impossible' places, such as before the end of the previous measure. Doing so creates a fragmentary bar, which is represented in the scrub area by a crosshatched region."

### Decisions

- Time signature storage: changes stay at bar boundaries, as M02 built them. Reason: page 173 allows a marker between bar lines, which creates a fragmentary bar. Storing that needs a beat position for each marker and a bar numbering rule for the fragment, and the manual states neither. The Transport holds the signature as a `TimeSignatureMap`, so the bar-index storage stays inside `livecraft-time`. Owner row Q06 measures the numbering on an installed Live 12.4. M07 moves markers to beat positions once the rule is known.
- Transport: `Transport.tempo` is a `TempoMap` and `Transport.time_signature` is a `TimeSignatureMap`, with `bpm()`, `set_bpm`, `time_signature()`, and `set_time_signature()` as the accessors. The JSON export keeps `bpm` and `time_signature` at the song start, so the characterisation snapshot holds. Tempo ramps and later signature changes are not exported to JSON. No code path creates them yet, and `.als` keeps them (M09).
- Main naming: `TrackType::Main`, `Project.main_track`, and `Track::new` use Live's name. The JSON keys stay `Master` and `master_track` through serde renames, and the default track name stays `Master`. Changing the snapshot needs the owner, so it is Q04.
- Track storage: tracks stay in `Vec<Track>` in set order, and lookup is by `TrackId` through `Project::track` and `Project::track_mut`. The characterisation test indexes `tracks[0]`, so the Vec stays. Ids are the identity.
- Typed ids: each id type implements `From<u64>` for file loading and integer literals. New ids come only from `IdAllocator`, which `Project::id_allocator` seeds above every id in the Set.
- Constructors: `Clip::new_midi` and `Clip::new_audio` take `impl Into<ClipId>`, because the protected characterisation test passes integer literals. The other constructors take typed ids.
- Parameters: each device kind has a table in `device.rs`. A `ParamId` is the position in that table, so tables only grow at the end. Parameters with no default (Operator `algorithm` and `feedback`, Simpler `start_pct`, `length_pct`, `crossfade_pct`, and `filter_resonance`, DrumRack `gain_db`) are absent until set, so the default JSON does not change.
- Name adapter: `get_param`, `set_param`, and `param` take the M01 names. A name outside the kind's table is ignored. Reads of an unset parameter return 0.0 from `get_param` and `None` from `param`, as the M01 `HashMap` did.
- Engine messages keep String parameter names until M05. The adapter is in `graph.rs` and `device.rs`.
- Routing: `Track.routing` holds the description and is left out of the JSON while it is default. Sends stay a `Vec<f32>` by position (Send A and B), so the JSON is unchanged. A typed send list is not introduced in M03.
- Edit operations: each checks its preconditions before it changes the Set. Removing a group moves its children to the group's parent. Removing a return track removes its send slot from every track. Removing a device removes the automation that targets it. These choices are assumptions. The manual pages read do not state what Live does to children on removal, so they are not verified against Live.

### Evidence

- `cargo xtask ci` passes on the final tree, with log `.agent-local/logs/2026-10-31_m03-09-ci.log`. Earlier per-item logs are in `.agent-local/logs/2026-10-31_m03-0N-ci.log`.
- `cargo test --workspace` (final tree): 122 passed, 0 failed.

| Crate                                           | Tests |
| ----------------------------------------------- | ----- |
| livecraft (characterisation 2, cli 4)           | 6     |
| livecraft-model (unit 25, edit 8, validation 8) | 41    |
| livecraft-formats                               | 6     |
| livecraft-dsp                                   | 1     |
| livecraft-devices (unit 17, tempo_sync 1)       | 18    |
| livecraft-engine (unit 8, wav_limits 2)         | 10    |
| livecraft-time                                  | 34    |
| livecraft-ui                                    | 2     |
| xtask                                           | 4     |
| livecraft-testkit                               | 0     |

- Property test: `random_edit_sequences_keep_the_set_valid` in `crates/livecraft-model/tests/edit.rs` runs `ProptestConfig::with_cases(10_000)`, and each case checks `validate()` after every step. It passes in about 2.9 s. The acceptance rate of edits is not measured, so the test does not show how many edits were accepted.
- `cargo test -p livecraft --test characterisation`: 2 passed. `git diff 39115f4 HEAD -- apps/livecraft/tests/characterisation.rs apps/livecraft/tests/snapshots/` prints nothing.
- `grep -rn "track_index: usize\|track_idx: usize" crates apps`: no matches.

### Criteria

| Criterion                                                                   | Met | Evidence                                              |
| --------------------------------------------------------------------------- | --- | ----------------------------------------------------- |
| `grep -rn "track_index: usize\|track_idx: usize" crates` returns nothing    | Yes | No matches in `crates` or `apps`                      |
| Property test: random edit sequences keep `validate()` empty (10,000 cases) | Yes | `tests/edit.rs`, `with_cases(10_000)`, passes         |
| M01 characterisation tests pass                                             | Yes | 2 passed. Test and snapshot unchanged since `39115f4` |

### Outline items

1. Typed ids and `IdAllocator`: done in `070a103`.
2. Split `model.rs`: done in `273b64e`. Move only. A line comparison of the old file against the new modules matches the new modules line for line, apart from `use` headers and one doc line.
3. Typed ids in tracks, clips, scenes, devices, and lanes: done in `3234aa3`. Engine and UI call sites updated.
4. Note upgrade: done in `163ed17`, with the transport carry-over. Note positions stay `f64` beats (see deviations).
5. Group tracks, nested groups, Main naming: done in `7bfee79`, with the naming held by Q04.
6. Routing description types: done in `69b8dc7`.
7. `ParamValues` and `ParamId`: done in `6f487a2`.
8. `validate()` with property tests: done in `dbce138`.
9. Verification and docs: this commit.

### Deviations from the milestone

1. Main track default name and export keys stay `Master` and `master_track` (Q04). The characterisation snapshot pins them.
2. `Project` keeps its name, because the characterisation test imports `livecraft_model::model::Project`. `LiveSet` is not introduced.
3. Note positions stay `f64` beats, not `Beats`. Converting 105 sites in 10 files is large, and the note editor (M18) rewrites them.
4. No `NoteId`. Notes are addressed by index until M18, because the milestone lists typed ids for tracks, clips, scenes, devices, and lanes only.
5. No `MpeData` on notes. MPE is M18.
6. No `ColorIndex`. The palette size and RGB values need owner input (Q05), so colours stay hex strings. Group and Main tracks use `#888888`, which is not confirmed against Live.
7. `ParamSpec` has a name and a default. Range, unit, taper, and steps come with each device's spec (M25 to M34).
8. Velocity is `f32`, but the instruments still take a rounded integer velocity. Fractional velocity reaches them in M32 and M33.
9. Release velocity, probability, velocity deviation, and mute are stored, but the engine does not use them yet (M18 and M19).
10. Device parameter names stay strings in the engine messages until M05.
11. Sends stay positional, and no typed send list is added.
12. Note defaults: release velocity 64, probability 1.0, velocity deviation 0.0, mute false. The manual does not state the release velocity default, so it needs an installed-Live check (Q07).
13. `.als` has no reader or writer changes beyond the group tag. The `GroupTrack` tag is unverified, since no Live Set with a group track is in the repository.
14. The transport JSON keeps only the song-start tempo and signature (see Decisions).

### Behaviour changes (intentional)

- Track and clip lookups and undo actions find any track kind by id, including return and Main tracks. Before, they searched only the audio and MIDI list.
- Chop copies the source note's properties. It used to reset `mute`.
- Ornament grace notes keep fractional velocity. Before, they were truncated to integers.
- Unknown parameter names passed to `set_param` are ignored. Before, they were stored. No code path in the repository set one.
- New tracks, clips, and devices take ids from `IdAllocator`. Before, a new track's id was `len + 1`, which collided with the return and Main ids (5 to 7) in the default Set, and the browser gave every new device id 1001 to 1004.
- The CLI lists the Main track as `Main`, and a group track as `Group`.
- The `.als` writer writes a group track as `GroupTrack`.

### Owner rows

- Q04: default Main track name and export keys, and the Live name check.
- Q05: palette size and RGB values for `ColorIndex`.
- Q06: time signature fragment numbering on installed Live 12.4.
- Q07: note property defaults on installed Live 12.4.

### Not verified

- No run of the GUI, so the Session and Arrangement views with groups and routing are untested.
- No `.als` file with group tracks, routing, or new note properties was round-tripped through Live.
- The property test does not report how many edits were accepted.
- The 10,000-case property test runs under the debug profile only, as `cargo test` does.
