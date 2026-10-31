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
| `crates/livecraft-model/src/model.rs`                 | Document model, MIDI transformations, scales, Scala tuning                                   | `Project`, `Track`, `Clip`, `ClipSlot`, `Device`, `TuningSystem`           | none                                 |
| `crates/livecraft-model/src/command.rs`               | `Command` enum and `CommandHistory` undo and redo                                            | `Command`, `CommandHistory`                                                | none                                 |
| `crates/livecraft-formats/src/als.rs`                 | `.als` gzip XML read and write                                                               | `AlsParser::load_project`, `save_project_to_als`                           | model                                |
| `crates/livecraft-formats/src/theme.rs`               | `.ask` theme parsing                                                                         | `AbletonTheme`                                                             | model                                |
| `crates/livecraft-dsp/src/biquad.rs`                  | DSP primitives (biquad filter)                                                               | `BiquadFilter`, `FilterType`                                               | none                                 |
| `crates/livecraft-devices/src/processor.rs`           | `DspProcessor` trait and factory from `Device`                                               | `DspProcessor`, `create_processor`                                         | model, dsp                           |
| `crates/livecraft-devices/src/{effects,instruments}/` | One effect or instrument per file                                                            | `<Name>Effect`, `OperatorSynth`, `SimplerInstrument`, `DrumRackInstrument` | model, dsp                           |
| `crates/livecraft-engine/src/engine.rs`               | `cpal` stream and parameter queue                                                            | `AudioEngine`, `AudioParamMsg`                                             | model, dsp, devices, formats         |
| `crates/livecraft-engine/src/graph.rs`                | Per-track state, clip playback, voices, mixing, meters                                       | `AudioGraph`, `TrackDspState`, `TrackMeter`                                | as above                             |
| `crates/livecraft-engine/src/audio_clip.rs`           | WAV sample cache with a 2^28-sample limit                                                    | `AudioSampleCache`                                                         | as above                             |
| `crates/livecraft-engine/src/bouncer.rs`              | Offline render and stems                                                                     | `OfflineBouncer`, `RenderOptions`                                          | as above                             |
| `crates/livecraft-ui/src/app.rs`                      | Frame loop, shortcuts, panel layout, engine wiring                                           | `LiveCraftApp`                                                             | model, formats, engine               |
| `crates/livecraft-ui/src/{view}.rs`                   | One view or panel per file (transport, session, arrange, mixer, devices, pianoroll, browser) | `render_*` functions                                                       | as above                             |
| `crates/livecraft-ui/src/widgets.rs`, `style.rs`      | Knobs, faders, meters, LCD fields; palettes                                                  | `live_knob`, `live_fader`, `LivePalette`                                   | as above                             |
| `apps/livecraft/src/main.rs`                          | `clap` subcommands, GUI launcher                                                             | `livecraft` binary                                                         | model, formats, engine, devices, ui  |
| `crates/livecraft-testkit/src/lib.rs`                 | Test helpers shared across crates                                                            | `render_project_summary`, `RenderSummary`                                  | model, engine                        |
| `crates/livecraft-time/src/units.rs`                  | Musical time units: beats and samples, with arithmetic                                       | `Beats`, `Samples`                                                         | none                                 |
| `crates/livecraft-time/src/tempo.rs`                  | Tempo map (step and linear ramp changes) and beat, second, and sample conversion             | `TempoMap`, `MIN_BPM`, `MAX_BPM`                                           | none                                 |
| `crates/livecraft-time/src/meter.rs`                  | Time signature map (changes at bar boundaries) and bar.beat.sixteenth conversion and display | `TimeSignatureMap`, `BarBeatSixteenth`, `TimeError`                        | none                                 |
| `crates/livecraft-time/src/grid.rs`                   | Grid and clip launch quantization (Live's Quantize menu) and the next grid line at or after a position | `Grid`, `LaunchQuantization`, `quantize_launch`                            | none                                 |
| `xtask/src/main.rs`                                   | `cargo xtask ci`, layering check, docs link check                                            | `cargo xtask <ci,layering,docs>`                                           | none                                 |

## Dependency Rules

- `livecraft-model`, `livecraft-dsp`, and `livecraft-time` have no internal dependencies and no audio, UI, or OS dependencies.
- `livecraft-formats` depends on model.
- `livecraft-devices` depends on model and dsp.
- `livecraft-engine` depends on model, dsp, devices, and formats.
- `livecraft-ui` depends on model, formats, and engine. The engine edge is temporary and is removed when M04 adds `livecraft-session`.
- `livecraft` (`apps/livecraft`) depends on model, formats, engine, devices, and ui.
- `livecraft-testkit` is a dev-only dependency and may use model and engine.
- Never add a reverse edge. A new crate needs a `LAYERS` entry, or `cargo xtask layering` fails.
- Only `apps/livecraft` (package `livecraft`) produces a binary.
- Enforced by `cargo xtask layering` (part of `cargo xtask ci`): the `LAYERS` table in `xtask/src/main.rs` is the source of truth.

## Where New Code Goes

- New document data or edit operation: `livecraft-model/src/model.rs`, with a `Command` variant in `command.rs`.
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
| GUI mutates `Project` directly; most edits bypass `CommandHistory`, so undo covers few actions                       | `crates/livecraft-ui/src/*.rs`                          | 1            |
| Entity identity is `usize` indexes in commands and messages, which break on insert or delete                         | `command.rs`, `engine.rs`                               | 1            |
| `.als` writer regenerates XML from the model; every unknown element is lost                                          | `als.rs`                                                | 2            |
| Session clip slots without an active clip are played back as a sequential timeline                                   | `graph.rs` `dispatch_clip_events`                       | 1            |
| Large files mixing concerns: `model.rs` (1214 lines), `graph.rs` (1080), `als.rs` (1008), `devices.rs` (995)         | as listed                                               | 1            |
| GUI layout uses estimated sizes and colours, not measurements of Live                                                | `crates/livecraft-ui/src/*.rs`                          | 3            |

## Growth Signals

- Any source file over 800 lines, or a module with more than one reason to change: split it.
- A second front end (for example a control server) needs state that lives in `LiveCraftApp`: move that state into a UI-independent crate (planned as `livecraft-session`, see `plans/livecraft-1.0/reference/TARGET-ARCHITECTURE.md`).
