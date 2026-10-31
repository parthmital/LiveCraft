# Architecture

Purpose: how the code is organised today and why, and where it falls short. Read when: adding a module, moving code, or changing dependencies between parts.
Last verified: 2026-10-31.

The target architecture that replaces this one is defined in `plans/livecraft-1.0/reference/TARGET-ARCHITECTURE.md`. Update this file as each plan task lands, so it always describes the code as it is.

## Overview

One process, three kinds of threads:

- UI thread: `eframe` runs `LiveCraftApp::update` once per frame. It owns the `Project` and mutates it directly.
- Audio thread: `cpal` calls `write_audio` (`crates/livecraft-engine/src/engine.rs`) for every hardware buffer. It drains `AudioParamMsg` messages and renders the `AudioGraph`.
- CLI: runs the same `AudioGraph` offline through `OfflineBouncer`, or opens the GUI.

```text
GUI or CLI --AudioParamMsg (crossbeam bounded 1024)--> audio callback
GUI --sync_project(&Project) via Arc<Mutex<AudioGraph>>::try_lock--> AudioGraph
audio callback --try_lock--> AudioGraph::process_buffer_interleaved --> cpal output
GUI --try_lock--> AudioGraph (reads playhead and meters)
```

## Module Map

| Crate or path                                         | Responsibility                                                                               | Main public API                                                            | May depend on (internal)             |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------ |
| `crates/livecraft-model/src/ids.rs`                  | Typed ids (`TrackId`, `ClipId`, `SceneId`, `DeviceId`, `LaneId`) and `IdAllocator`           | `TrackId`, `ClipId`, `SceneId`, `DeviceId`, `LaneId`, `IdAllocator`        | none                                 |
| `crates/livecraft-model/src/model.rs`                | Facade that re-exports the model modules, so imports from `model` keep working               | every model type                                                           | model modules                        |
| `crates/livecraft-model/src/set.rs`                  | Live Set (`Project`), transport, and the default Set                                         | `Project`, `Transport`                                                     | time, track, device, scene, scale, tuning |
| `crates/livecraft-model/src/track.rs`                | Tracks (audio, MIDI, group, return, and Main), parent groups, and monitoring                | `Track`, `TrackType`, `MonitoringMode`                                     | clip, device, automation, color      |
| `crates/livecraft-model/src/clip.rs`                 | Clips, clip slots, arrangement clips, audio payloads, and warp markers                       | `Clip`, `ClipSlot`, `ArrangementClip`, `ClipPayload`                       | note, automation                     |
| `crates/livecraft-model/src/note.rs`                 | MIDI notes and MIDI clip data                                                                | `MidiNote`, `MidiClipData`                                                 | none                                 |
| `crates/livecraft-model/src/midi_tools_legacy.rs`    | MIDI transformations moved from M01 (velocity and copies adapted in M03); M19 replaces them | `MidiClipData` methods (quantize, humanize, chop, and so on)               | note, scale                          |
| `crates/livecraft-model/src/device.rs`               | Device kinds, per-kind parameter tables, and devices whose values are stored by id              | `Device`, `DeviceType`                                                     | none                                 |
| `crates/livecraft-model/src/automation.rs`           | Automation targets, points, and lanes                                                        | `AutomationLane`, `AutomationPoint`, `AutomationTarget`                    | none                                 |
| `crates/livecraft-model/src/param.rs`              | Parameter ids and values per device, stored sorted by id, with the per-kind tables in `device.rs` | `ParamId`, `ParamValues`, `ParamSpec`, `param_id` | none |
| `crates/livecraft-model/src/routing.rs`              | Audio routing descriptions: inputs, outputs, and tap points (no audio until M20)             | `Routing`, `AudioInput`, `AudioOutput`, `TapPoint`                         | ids                                  |
| `crates/livecraft-model/src/edit.rs`                 | Edit operations that check their preconditions, so accepted edits keep the Set valid         | `Project::add_track`, `add_return_track`, `remove_track`, `set_parent`, `set_clip`, and others | model modules                        |
| `crates/livecraft-model/src/validate.rs`             | `Project::validate`: referential integrity and the structural rules of the Set              | `Project::validate`, `ValidationError`                                      | model modules                        |
| `crates/livecraft-model/src/scene.rs`                | Scenes with optional tempo and time signature                                                | `Scene`                                                                    | none                                 |
| `crates/livecraft-model/src/scale.rs`                | Scales and scale snapping                                                                    | `ScaleConfig`, `ScaleType`, `note_name`                                    | none                                 |
| `crates/livecraft-model/src/tuning.rs`               | Scala tuning systems and frequency lookup                                                    | `TuningSystem`                                                             | none                                 |
| `crates/livecraft-model/src/color.rs`                | Live's clip and track colour palette, as hex strings until the colour index                  | `ABLETON_PALETTE`                                                          | none                                 |
| `crates/livecraft-formats/src/als.rs`                 | `.als` gzip XML read and write                                                               | `AlsParser::load_project`, `save_project_to_als`                           | model                                |
| `crates/livecraft-formats/src/theme.rs`               | `.ask` theme parsing                                                                         | `AbletonTheme`                                                             | model                                |
| `crates/livecraft-dsp/src/biquad.rs`                  | DSP primitives (biquad filter)                                                               | `BiquadFilter`, `FilterType`                                               | none                                 |
| `crates/livecraft-devices/src/processor.rs`           | `DspProcessor` trait and factory from `Device`                                               | `DspProcessor`, `create_processor`                                         | model, dsp                           |
| `crates/livecraft-devices/src/{effects,instruments}/` | One effect or instrument per file                                                            | `<Name>Effect`, `OperatorSynth`, `SimplerInstrument`, `DrumRackInstrument` | model, dsp                           |
| `crates/livecraft-engine/src/engine.rs`               | `cpal` stream and parameter queue                                                            | `AudioEngine`, `AudioParamMsg`                                             | model, dsp, devices, formats         |
| `crates/livecraft-engine/src/graph.rs`                | Per-track state, clip playback, voices, mixing, meters                                       | `AudioGraph`, `TrackDspState`, `TrackMeter`                                | as above                             |
| `crates/livecraft-engine/src/audio_clip.rs`           | WAV sample cache with a 2^28-sample limit                                                    | `AudioSampleCache`                                                         | as above                             |
| `crates/livecraft-engine/src/bouncer.rs`              | Offline render and stems                                                                     | `OfflineBouncer`, `RenderOptions`                                          | as above                             |
| `crates/livecraft-ui/src/app.rs`                      | Frame loop, shortcuts, panel layout; applies each view's edits through the session           | `LiveCraftApp`                                                             | model, session, time                 |
| `crates/livecraft-ui/src/{view}.rs`                   | One view or panel per file (transport, session, arrange, mixer, devices, pianoroll, browser); reads `&Project` and returns `Edits` (commands) | `render_*` functions                                                       | model, session, time                 |
| `crates/livecraft-ui/src/widgets.rs`, `style.rs`      | Knobs, faders, meters, LCD fields; palettes                                                  | `live_knob`, `live_fader`, `LivePalette`                                   | as above                             |
| `apps/livecraft/src/main.rs`                          | `clap` subcommands (`command list`, `command run`, and the rest), GUI launcher               | `livecraft` binary                                                         | model, formats, engine, devices, session, ui |
| `crates/livecraft-testkit/src/lib.rs`                 | Test helpers shared across crates                                                            | `render_project_summary`, `RenderSummary`                                  | model, engine                        |
| `crates/livecraft-time/src/units.rs`                  | Musical time units: beats and samples, with arithmetic                                       | `Beats`, `Samples`                                                         | none                                 |
| `crates/livecraft-time/src/tempo.rs`                  | Tempo map (step and linear ramp changes) and beat, second, and sample conversion             | `TempoMap`, `MIN_BPM`, `MAX_BPM`, `seconds_per_beat`, `samples_per_beat`, `beats_per_sample`, `seconds_for_beats` | none                                 |
| `crates/livecraft-time/src/meter.rs`                  | Time signature map (changes at bar boundaries) and bar.beat.sixteenth conversion and display | `TimeSignatureMap`, `BarBeatSixteenth`, `TimeError`                        | none                                 |
| `crates/livecraft-time/src/grid.rs`                   | Grid and clip launch quantization (Live's Quantize menu) and the next grid line at or after a position | `Grid`, `LaunchQuantization`, `quantize_launch`                            | none                                 |
| `crates/livecraft-session/src/command.rs`             | `Operation` (a named, serialisable edit with undo), `Command` (its object-safe form), `CommandError` | `Operation`, `Command`, `CommandError`                          | model                                |
| `crates/livecraft-session/src/history.rs`             | `UndoHistory`: apply, undo, redo, gesture merge within 500 ms, a step limit (default 1000), transactions (one labelled step), and the Undo History entries with revert and reapply (`go_to`) | `UndoHistory`, `Transaction`, `HistoryEntry`, `MERGE_GAP`, `OPENING_LABEL` | model                                |
| `crates/livecraft-session/src/registry.rs`            | `CommandRegistry`: stable names to constructors that decode JSON arguments                   | `CommandRegistry`                                                          | model                                |
| `crates/livecraft-session/src/session.rs`             | `Session`: the open Set, its history, its id allocator, its engine, and the dirty flag; pushes changes to the engine; file open and save; `demo_set` | `Session`, `load_set`, `demo_set`, `EngineStatus`, `TrackMeter`            | time, model, formats, engine         |
| `crates/livecraft-session/src/commands/scale.rs`      | Scale settings (active, root note, type) as one edit                                         | one `Operation`                                                            | model                                |
| `crates/livecraft-session/src/commands/track.rs`      | Track create, delete (with exact undo), name, color, order, arm, solo, mute, monitor, volume, pan, and sends | one `Operation` per edit, `commands::registry()` | model                                |
| `crates/livecraft-session/src/commands/automation.rs` | Automation point add, remove, and move, kept sorted by beat                                  | one `Operation` per edit                                                   | model                                |
| `crates/livecraft-session/src/commands/transport.rs` | Tempo, time signature, loop, and metronome                                                   | one `Operation` per edit                                                   | model, time                          |
| `crates/livecraft-session/src/commands/scene.rs`     | Scene create, delete (back to its row on undo), and duplicate (after its source)              | one `Operation` per edit                                                   | model                                |
| `crates/livecraft-session/src/commands/clip.rs`      | Clip create (MIDI and audio), delete, duplicate (new clip and lane ids), move, rename, loop, and markers. A clip is named by `ClipId`; a slot by its track and row | one `Operation` per edit                                                   | model, time                          |
| `crates/livecraft-session/src/commands/note.rs`      | MIDI note add, remove, and edit, checked against Live's note ranges. A note is named by its index until M18 | one `Operation` per edit                                                   | model                                |
| `crates/livecraft-session/src/commands/device.rs`    | Device insert (new id), remove (with the automation that targets it, restored on undo), move, enable, and set parameter by name | one `Operation` per edit                                       | model                                |
| `xtask/src/main.rs`                                   | `cargo xtask ci`, layering check, docs link check                                            | `cargo xtask <ci,layering,docs>`                                           | none                                 |

## Dependency Rules

- `livecraft-dsp` and `livecraft-time` have no internal dependencies. `livecraft-model` depends only on time. None of them has audio, UI, or OS dependencies.
- `livecraft-formats` depends on model.
- `livecraft-devices` depends on model, dsp, and time.
- `livecraft-engine` depends on model, dsp, devices, formats, and time.
- `livecraft-session` depends on time, model, formats, and engine. It is the only crate that changes the Set: front ends read `Session::project` and send commands through `Session::apply`, `transaction`, `undo`, and `redo`.
- `livecraft-ui` depends on model, session, and time. It has no engine or file-format dependency: the session opens the engine and loads and saves files.
- `livecraft` (`apps/livecraft`) depends on model, formats, engine, devices, and ui.
- `livecraft-testkit` is a dev-only dependency and may use model and engine.
- Never add a reverse edge. A new crate needs a `LAYERS` entry, or `cargo xtask layering` fails.
- Only `apps/livecraft` (package `livecraft`) produces a binary.
- Enforced by `cargo xtask layering` (part of `cargo xtask ci`): the `LAYERS` table in `xtask/src/main.rs` is the source of truth.

## Where New Code Goes

- New document data or edit operation: the entity's module in `livecraft-model/src/` (for example `track.rs` or `clip.rs`), re-exported from `model.rs`, plus an `Operation` in `livecraft-session/src/commands/<area>.rs`, registered in that module's `register`.
- New DSP building block (filter, oscillator, envelope): `livecraft-dsp/src/`.
- New effect or instrument: a file in `livecraft-devices/src/effects/` or `instruments/`, a `DspProcessor` impl in `processor.rs`, a `DeviceType` variant in `livecraft-model`.
- New view or panel: a new file in `livecraft-ui/src/`, called from `app.rs`.
- New CLI subcommand: `apps/livecraft/src/main.rs` plus a test in `apps/livecraft/tests/cli_tests.rs`.

## Decisions

- Rust workspace with separate model, formats, DSP, devices, engine, GUI, and CLI crates: keeps the model and DSP testable without audio or UI.
- `egui` immediate-mode GUI: fully custom painting suits Live's flat, dense look; used by the Storytold Craft apps. See `RESEARCH.md#gui-toolkit-2026-10-27`.
- `.als` support through `quick-xml` and `flate2`: a Live Set is gzip-compressed XML.
- wgpu renderer with DX12 default on Windows: Vulkan crashed on the reference machine (DECISIONS.md D12).

## Known Violations

Each is scheduled in [plans/livecraft-1.0/PLAN.md](plans/livecraft-1.0/PLAN.md).

| Problem                                                                                                              | Where                                                   | Plan phase   |
| -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- | ------------ |
| Audio callback takes `Arc<Mutex<AudioGraph>>::try_lock`; contention outputs silence                                  | `engine.rs` `write_audio*`                              | 1            |
| Audio callback clones a whole `Clip` (with its note `Vec`) for every track every block                               | `graph.rs` `dispatch_clip_events`                       | 1            |
| `AudioParamMsg::SetDeviceParam` carries a `String` key, freed on the audio thread                                    | `engine.rs`                                             | 1            |
| Audio clips decode their WAV file from disk on the audio thread the first time they play                             | `audio_clip.rs` `render_clip_samples` via `get_or_load` | 1 (M05, M11) |
| `process_buffer_interleaved` grows its scratch buffers on the audio thread when the driver block exceeds 8192 frames | `graph.rs`                                              | 1            |
| `DspProcessor` processes one sample per virtual call and looks parameters up by string                               | `processor.rs`                                          | 1            |
| Clip-slot, note, and send positions are still used as identities in commands and messages (tracks, clips, scenes, devices, and automation lanes are typed since M03) | `engine.rs`, `livecraft-session/src/commands/automation.rs` (points by index), note commands (M04 item 5 and M18) | 1 (M04, M18) |
| `.als` writer regenerates XML from the model; every unknown element is lost                                          | `als.rs`                                                | 2            |
| Session clip slots without an active clip are played back as a sequential timeline                                   | `graph.rs` `dispatch_clip_events`                       | 1            |
| Large files mixing concerns: `graph.rs` (1080), `als.rs` (1008), `devices.rs` (995)         | as listed                                               | 1            |
| GUI layout uses estimated sizes and colours, not measurements of Live                                                | `crates/livecraft-ui/src/*.rs`                          | 3            |

## Growth Signals

- Any source file over 800 lines, or a module with more than one reason to change: split it.
- A second front end (for example a control server) needs state that lives in `LiveCraftApp`: move that state into a UI-independent crate (planned as `livecraft-session`, see `plans/livecraft-1.0/reference/TARGET-ARCHITECTURE.md`).
