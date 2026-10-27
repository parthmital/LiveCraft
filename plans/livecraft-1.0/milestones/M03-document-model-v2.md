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
