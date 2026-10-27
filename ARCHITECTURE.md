# Architecture

Purpose: how the code is organised today and why, and where it falls short. Read when: adding a module, moving code, or changing dependencies between parts.
Last verified: 2026-10-27.

The target architecture that replaces this one is defined in `plans/livecraft-1.0/reference/TARGET-ARCHITECTURE.md`. Update this file as each plan task lands, so it always describes the code as it is.

## Overview

One process, three kinds of threads:

- UI thread: `eframe` runs `LiveCraftApp::update` once per frame. It owns the `Project` and mutates it directly.
- Audio thread: `cpal` calls `write_audio` (`crates/livecraft-dsp/src/engine.rs`) for every hardware buffer. It drains `AudioParamMsg` messages and renders the `AudioGraph`.
- CLI: runs the same `AudioGraph` offline through `OfflineBouncer`, or opens the GUI.

```text
GUI or CLI --AudioParamMsg (crossbeam bounded 1024)--> audio callback
GUI --sync_project(&Project) via Arc<Mutex<AudioGraph>>::try_lock--> AudioGraph
audio callback --try_lock--> AudioGraph::process_buffer_interleaved --> cpal output
GUI --try_lock--> AudioGraph (reads playhead and meters)
```

## Module Map

| Crate or path                                     | Responsibility                                                                               | Main public API                                                            | May depend on          |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | ---------------------- |
| `crates/livecraft-core/src/model.rs`              | Document model, MIDI transformations, scales, Scala tuning                                   | `Project`, `Track`, `Clip`, `ClipSlot`, `Device`, `TuningSystem`           | serde                  |
| `crates/livecraft-core/src/command.rs`            | `Command` enum and `CommandHistory` undo and redo                                            | `Command`, `CommandHistory`                                                | model                  |
| `crates/livecraft-core/src/als.rs`                | `.als` gzip XML read and write                                                               | `AlsParser::load_project`, `save_project_to_als`                           | quick-xml, flate2      |
| `crates/livecraft-core/src/theme.rs`              | `.ask` theme parsing                                                                         | `AbletonTheme`                                                             | quick-xml              |
| `crates/livecraft-dsp/src/engine.rs`              | `cpal` stream and parameter queue                                                            | `AudioEngine`, `AudioParamMsg`                                             | graph, cpal, crossbeam |
| `crates/livecraft-dsp/src/graph.rs`               | Per-track state, clip playback, voices, mixing, meters                                       | `AudioGraph`, `TrackDspState`                                              | processor, instruments |
| `crates/livecraft-dsp/src/processor.rs`           | `DspProcessor` trait and factory from `Device`                                               | `DspProcessor`, `create_processor`                                         | effect modules         |
| `crates/livecraft-dsp/src/{effect}.rs`            | One effect or instrument per file                                                            | `<Name>Effect`, `OperatorSynth`, `SimplerInstrument`, `DrumRackInstrument` | biquad                 |
| `crates/livecraft-dsp/src/bouncer.rs`             | Offline render and stems                                                                     | `OfflineBouncer`                                                           | graph, hound           |
| `crates/livecraft-gui/src/app.rs`                 | Frame loop, shortcuts, panel layout, engine wiring                                           | `LiveCraftApp`                                                             | all GUI modules        |
| `crates/livecraft-gui/src/{view}.rs`              | One view or panel per file (transport, session, arrange, mixer, devices, pianoroll, browser) | `render_*` functions                                                       | widgets, style         |
| `crates/livecraft-gui/src/widgets.rs`, `style.rs` | Knobs, faders, meters, LCD fields; palettes                                                  | `live_knob`, `live_fader`, `LivePalette`                                   | egui                   |
| `crates/livecraft-cli/src/main.rs`                | `clap` subcommands, GUI launcher                                                             | `livecraft` binary                                                         | every crate            |

## Dependency Rules

- `livecraft-core` has no audio, UI, or OS dependencies.
- `livecraft-dsp` never depends on `livecraft-gui`.
- Only `livecraft-cli` produces a binary.
- Not enforced by tooling yet. The plan adds a layering check to `cargo xtask ci`.

## Where New Code Goes

- New document data or edit operation: `livecraft-core/src/model.rs`, with a `Command` variant in `command.rs`.
- New effect: a new file in `livecraft-dsp/src/`, a `DspProcessor` impl in `processor.rs`, a `DeviceType` variant in `model.rs`.
- New view or panel: a new file in `livecraft-gui/src/`, called from `app.rs`.
- New CLI subcommand: `livecraft-cli/src/main.rs` plus a test in `crates/livecraft-cli/tests/cli_tests.rs`.

## Decisions

- Rust workspace with separate core, DSP, GUI, and CLI crates: keeps the model testable without audio or UI.
- `egui` immediate-mode GUI: fully custom painting suits Live's flat, dense look; used by the Storytold Craft apps. See `RESEARCH.md#gui-toolkit-2026-10-27`.
- `.als` support through `quick-xml` and `flate2`: a Live Set is gzip-compressed XML.

## Known Violations

Each is scheduled in `plans/livecraft-1.0/`.

| Problem                                                                                                                            | Where                                                   | Plan phase   |
| ---------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- | ------------ |
| Audio callback takes `Arc<Mutex<AudioGraph>>::try_lock`; contention outputs silence                                                | `engine.rs` `write_audio*`                              | 1            |
| Audio callback clones a whole `Clip` (with its note `Vec`) for every track every block                                             | `graph.rs` `dispatch_clip_events`                       | 1            |
| `AudioParamMsg::SetDeviceParam` carries a `String` key, freed on the audio thread                                                  | `engine.rs`                                             | 1            |
| Audio clips decode their WAV file from disk on the audio thread the first time they play                                           | `audio_clip.rs` `render_clip_samples` via `get_or_load` | 1 (M05, M11) |
| `process_buffer_interleaved` grows its scratch buffers on the audio thread when the driver block exceeds 8192 frames               | `graph.rs`                                              | 1            |
| `DspProcessor` processes one sample per virtual call and looks parameters up by string                                             | `processor.rs`                                          | 1            |
| GUI mutates `Project` directly; most edits bypass `CommandHistory`, so undo covers few actions                                     | `crates/livecraft-gui/src/*.rs`                         | 1            |
| Entity identity is `usize` indexes in commands and messages, which break on insert or delete                                       | `command.rs`, `engine.rs`                               | 1            |
| `.als` writer regenerates XML from the model; every unknown element is lost                                                        | `als.rs`                                                | 2            |
| `.als` decompression has no size limit                                                                                             | `als.rs` `decompress_to_xml`                            | 0            |
| Session clip slots without an active clip are played back as a sequential timeline                                                 | `graph.rs` `dispatch_clip_events`                       | 1            |
| Large files mixing concerns: `model.rs` (1208 lines), `graph.rs` (1072), `devices.rs` (990)                                        | as listed                                               | 1            |
| GUI layout uses estimated sizes and colours, not measurements of Live                                                              | `crates/livecraft-gui/src/*.rs`                         | 3            |
| Window title and status bar use the Ableton trademark as branding                                                                  | `livecraft-gui/src/lib.rs`, `app.rs`                    | 0            |

## Growth Signals

- Any source file over 800 lines, or a module with more than one reason to change: split it.
- A second front end (for example a control server) needs state that lives in `LiveCraftApp`: move that state into a UI-independent crate (planned as `livecraft-app`).
- Device count past about 20: move devices out of `livecraft-dsp` into their own crate (planned as `livecraft-devices`).
