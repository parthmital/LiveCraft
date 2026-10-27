# Target Architecture

Purpose: the architecture LiveCraft 1.0 must have, decided once so no task re-decides it. Read when: expanding a milestone, creating a crate, or adding a dependency between crates.
Last verified: 2026-10-27.

Milestone M01 creates this layout; later milestones fill it. Root `ARCHITECTURE.md` describes the code as it is at any moment and must converge on this file.

## Principles

1. One document model, one command registry, one engine. The GUI, CLI, and control server are thin front ends that dispatch the same named commands.
2. The audio thread never allocates, frees, locks, logs, or does I/O. Everything it needs is prepared off-thread and handed over through wait-free queues.
3. Offline render runs the exact same engine code as playback, with a virtual clock, so renders are deterministic and bit-identical across runs on one machine.
4. Every crate below the front ends is free of UI toolkit and OS window dependencies, and is testable headlessly.
5. Lossless file compatibility: anything read from a Live file is written back unchanged unless the user changed it.

## Crate Map

| Crate (path)                                                     | Responsibility                                                                                                                                                                     | May depend on (internal)                       |
| ---------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------- |
| `livecraft-time` (`crates/livecraft-time`)                       | `Beats`, `Samples`, `TempoMap`, `TimeSignatureMap`, grid and quantization values, bar.beat.sixteenth formatting                                                                    | none                                           |
| `livecraft-model` (`crates/livecraft-model`)                     | The Live Set document: ids, tracks, clips, scenes, devices and parameter values, routing, automation, grooves, take lanes, validation                                              | time                                           |
| `livecraft-dsp` (`crates/livecraft-dsp`)                         | Pure DSP primitives: filters, band-limited oscillators, envelopes, LFOs, delay lines, smoothing, oversampling, FFT helpers, dither, meters (peak, RMS, LUFS, true peak)            | none                                           |
| `livecraft-warp` (`crates/livecraft-warp`)                       | Transient and tempo detection, warp marker maths, the six warp modes                                                                                                               | dsp                                            |
| `livecraft-devices` (`crates/livecraft-devices`)                 | `DeviceProcessor` trait, parameter specs, every native audio effect, MIDI effect, instrument, rack, and modulator                                                                  | time, model, dsp, warp                         |
| `livecraft-formats` (`crates/livecraft-formats`)                 | Lossless `.als`, `.alc`, `.adg`, `.adv`, JSON export, MIDI files, Scala `.scl` and `.ascl`, `.ask` themes (read only), audio decode (symphonia) and encode (hound), analysis cache | time, model                                    |
| `livecraft-engine` (`crates/livecraft-engine`)                   | Graph compiler, real-time executor, transport and launch scheduler, clip players, recording, PDC, metering, audio and MIDI I/O backends, offline renderer, plug-in host client     | time, model, dsp, warp, devices, formats       |
| `livecraft-plugin-protocol` (`crates/livecraft-plugin-protocol`) | Shared-memory and message protocol between engine and plug-in host process                                                                                                         | none                                           |
| `livecraft-library` (`crates/livecraft-library`)                 | Browser index, tags, collections, search, similarity embeddings, preview decoding                                                                                                  | model, formats, dsp                            |
| `livecraft-control` (`crates/livecraft-control`)                 | MIDI and Key mapping, computer MIDI keyboard, control surface scripts, MIDI clock, Tempo Follower, Link (feature)                                                                  | time, model                                    |
| `livecraft-session` (`crates/livecraft-session`)                 | App core without UI: `Session`, command registry, undo history, selection, clipboard, preferences, jobs, autosave, engine and library handles                                      | time, model, formats, engine, library, control |
| `livecraft-server` (`crates/livecraft-server`)                   | JSON-RPC 2.0 control server (stdio and localhost TCP) and MCP adapter over the command registry                                                                                    | session                                        |
| `livecraft-ui` (`crates/livecraft-ui`)                           | egui front end: theme tokens, widget kit, every view and dialog                                                                                                                    | time, model, session                           |
| `livecraft-testkit` (`crates/livecraft-testkit`)                 | Test fixtures, golden audio and image comparison, oracle helpers (dev-dependency only)                                                                                             | any (dev only)                                 |
| `livecraft` (`apps/livecraft`)                                   | The `livecraft` binary: GUI by default, CLI subcommands                                                                                                                            | session, server, ui                            |
| `livecraft-plugin-host` (`apps/livecraft-plugin-host`)           | Out-of-process plug-in host binary (CLAP, VST3, AU)                                                                                                                                | plugin-protocol, dsp                           |
| `xtask` (`xtask`)                                                | `cargo xtask ci`, layering check, doc link check, release packaging                                                                                                                | none                                           |

Enforcement: `cargo xtask ci` reads `cargo metadata` and fails when a normal dependency edge is not in this table (Task P0-07 creates the check; M01 updates its rule table).

## Threads

| Thread                | Owns                                                       | Talks to                                                    |
| --------------------- | ---------------------------------------------------------- | ----------------------------------------------------------- |
| UI (main)             | egui context, view state                                   | session through method calls; reads engine snapshots        |
| Session worker        | `Session`, the model, undo history, graph compiler         | UI through a command channel; engine through `rtrb` queues  |
| Audio (cpal callback) | Compiled graph, device processors, voices, transport       | session through `rtrb`; publishes `triple_buffer` snapshots |
| Audio workers (M41)   | Nothing persistent; process graph nodes in parallel        | audio thread through atomics                                |
| Disk streamer         | Open sample files, read-ahead buffers                      | audio thread through `rtrb` buffer handoff                  |
| Collector             | `basedrop` collector; frees what the audio thread released | none                                                        |
| Jobs pool             | Analysis, rendering, stem separation, library indexing     | session through job results                                 |

The first versions may run the session on the UI thread (M04); the split into a session worker happens in M41 when profiling shows it is needed. Interfaces are designed so the move changes no call sites outside `livecraft-session`.

## Data Model Decisions

- Identity: every entity has a typed id (`TrackId(u64)`, `ClipId(u64)`, `SceneId(u64)`, `DeviceId(u64)`, `LaneId(u64)`) allocated by the set's `IdAllocator`. Indexes are never used as identity in commands, messages, or files.
- Parameters: each device kind has a static table of `ParamSpec { id: ParamId(u16), name, short_name, min, max, default, unit, taper, steps }`. The model stores the real (unnormalised) value per `(DeviceId, ParamId)`. Display strings use Live's units and rounding, taken from the manual and reference captures.
- Time: model positions are `Beats(f64)` (quarter notes), matching `.als`. The engine converts to sample positions per block with `TempoMap`; no accumulated per-sample floating drift (positions are recomputed from block start each block).
- MIDI notes: `{ id, pitch: u8, start: Beats, duration: Beats, velocity: f32, release_velocity: f32, probability: f32, velocity_deviation: f32, mute: bool, mpe: Option<MpeData> }`, matching Live 12's note properties.
- Note names: MIDI 60 is C3 everywhere.
- Undo: commands record inverse data. History is bounded only by memory (configurable, default 1000 steps).

## Engine Decisions

- Graph compile: the session thread compiles the model into a `CompiledGraph` (nodes, edges in topological order, preallocated buffers, PDC delay lines, a transfer list mapping old node slots to new ones). The audio thread swaps graphs at the start of a block in O(nodes) without allocating; the old graph returns through a queue and is freed off-thread.
- Processing: block based, block size from the driver (32 to 2048) with sub-block splitting for sample-accurate events inside devices (CLAP-style event lists with sample offsets).
- `DeviceProcessor` trait: `fn process(&mut self, ctx: &ProcessContext, io: &mut AudioIo, events: &EventList, out_events: &mut EventSink)`, `fn set_param(&mut self, id: ParamId, value: f32, offset: u32)`, `fn latency_samples(&self) -> u32`, `fn reset(&mut self)`, `fn tail_samples(&self) -> Option<u64>`. No string lookups, no per-sample virtual calls.
- Parameter changes: the session sends `ParamEvent { node, param, value, offset }` through `rtrb`; automation is evaluated by the engine per block with sample-accurate breakpoints and smoothing per `ParamSpec`.
- Feedback: transport position, clip states, meters, CPU load are published through `triple_buffer`; discrete events (clip launched, recording finished, overload) through an `rtrb` queue.
- Safety: debug and test builds wrap the callback in `assert_no_alloc`. Release builds replace non-finite output samples with 0.0.
- Offline render: a `VirtualDriver` drives the same `Engine` with a fixed block size and no wall clock.

## UI Decisions

- `egui` 0.37 with the wgpu renderer; every Live widget is custom painted from theme tokens; no egui default widgets visible to users.
- Views are functions of `(&SessionSnapshot, &mut ViewState) -> Vec<Intent>`; intents become commands. No view mutates the model.
- Theme tokens per Live theme and a global zoom factor (Live's Zoom setting, 50 to 200 percent) live in `livecraft-ui/src/theme/`; values come from measurement (see `UI-FIDELITY.md`).
- Keyboard shortcuts come from one data table generated from manual chapter 42 (M14) and map to command names.

## File Format Decisions

- `.als` is the primary save format. Reading builds an `XmlTree` that keeps every element, attribute, attribute order, and text; the model maps onto tree nodes by `Id` and path; saving patches the tree. New sets start from a LiveCraft-authored template tree with `MajorVersion="5" MinorVersion="12.0_12117" SchemaChangeCount="10"` and `Creator="LiveCraft <version>"`.
- Unknown device types (plug-ins not installed, Max for Live devices) stay in the tree, show as placeholder devices, and are written back untouched.
- Optional uncompressed, stably formatted XML export for version control (`livecraft convert --pretty-xml`).

## Where New Code Goes

- New edit operation: a command in `livecraft-session/src/commands/<area>.rs`, registered in the registry, with an undo test.
- New device: `livecraft-devices/src/<category>/<device>/` with `spec.rs`, `processor.rs`, `tests.rs`; its UI in `livecraft-ui/src/devices/<device>.rs`; its `.als` mapping in `livecraft-formats/src/als/devices/<device>.rs`.
- New view or dialog: `livecraft-ui/src/views/<view>/`.
- New file format: `livecraft-formats/src/<format>/`.
- New CLI subcommand: `apps/livecraft/src/cli/<command>.rs`, dispatching registry commands.
