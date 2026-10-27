# Plan Overview

Purpose: the phases, milestones, order, and exit criteria from today's code to LiveCraft 1.0. Read when: starting the plan, expanding a milestone, or reviewing progress.
Last verified: 2026-10-27.

## Starting Point (2026-10-27, commit `1735fa8`)

- Four crates, about 20,000 lines, 44 tests. A working prototype: `.als` subset read and write, a few simplified devices, offline WAV render, an egui GUI that resembles Live's layout.
- Not production quality: the audio thread locks and allocates, `.als` saving drops data, the GUI edits the model directly, sizes and colours are estimates, and most of Live is missing. `ARCHITECTURE.md#known-violations` lists the evidence.
- The previous 28-task plan was removed: its tasks were broad, its verification was a single crate test run, and it marked work as done that does not meet its own criteria.

## Strategy

1. **Foundation first** (Phase 0): tooling, CI, security, and dependency upgrades, fully specified as tasks now.
2. **Rolling-wave detail**: Phases 1 to 8 are milestones. Each milestone file fixes scope, decisions, interfaces, and acceptance criteria; an Expand line turns it into tasks just before execution, against the code as it then exists, and the executor continues straight into those tasks in the same run. This keeps every task exact without writing thousands of tasks against code that will have changed.
3. **Engine and model before UI**: the real-time engine, document model, commands, and lossless file I/O are the parts every feature depends on and the hardest to change later.
4. **Measure, don't guess**: parity is proven against the manual and the installed Live (`reference/QUALITY-GATES.md`, `reference/UI-FIDELITY.md`).
5. **One run, no waiting**: one prompt runs every line to the release. Owner-only work (captures, oracle renders, measurements, legal advice, certificates) is queued in `OWNER-QUEUE.md` and finished by Reference pass lines; owner decisions use their defaults.
6. **Parity, then beyond**: improvements in `reference/BEYOND-LIVE.md` are scheduled after the parity work they extend, except where cheaper to build in from the start (plug-in sandboxing, autosave, the control server).

## Phases And Milestones

| Phase | Milestone | Title                                                             | Live manual chapters |
| ----- | --------- | ----------------------------------------------------------------- | -------------------- |
| 0     | P0        | Production foundation (18 tasks, written)                         | none                 |
| 1     | M01       | Workspace restructure into target crates                          | none                 |
| 1     | M02       | Musical time foundation                                           | 6.5, 9.1             |
| 1     | M03       | Document model v2                                                 | 3, 5.4               |
| 1     | M04       | Session, command registry, undo                                   | 42.5                 |
| 1     | M05       | Real-time engine core                                             | 38, 39, 40           |
| 1     | M06       | Audio and MIDI I/O and device settings                            | 2.3, 17.2, 17.3      |
| 1     | M07       | Transport, tempo, metronome, launch scheduler                     | 3.1, 16, 6.3 to 6.6  |
| 1     | M08       | Offline render and export                                         | 3.21, 5.4            |
| 2     | M09       | Lossless Live Set I/O                                             | 5.4                  |
| 2     | M10       | Live Projects and file management                                 | 5                    |
| 2     | M11       | Audio files, sample cache, disk streaming                         | 5.1, 38.2            |
| 3     | M12       | UI reference capture and fidelity tooling (queues owner captures) | all UI chapters      |
| 3     | M13       | Design system: tokens, zoom, font, icons, widget kit              | 3, 41                |
| 3     | M14       | Main window shell, menus, Control Bar, Info View, shortcuts       | 3.1, 3.2, 41, 42     |
| 4     | M15       | Session View                                                      | 7, 16                |
| 4     | M16       | Arrangement View                                                  | 6                    |
| 4     | M17       | Clip View and audio clip editing                                  | 8                    |
| 4     | M18       | MIDI Note Editor and MPE                                          | 10, 12               |
| 4     | M19       | MIDI Tools: transformations and generators                        | 11                   |
| 4     | M20       | Mixer, groups, returns, routing, crossfader, cue                  | 3.14, 3.16, 17, 18   |
| 4     | M21       | Recording, capture, comping                                       | 19, 21               |
| 4     | M22       | Automation and clip envelopes                                     | 26, 27               |
| 4     | M23       | Warping and tempo                                                 | 9                    |
| 4     | M24       | Browser, library, Sound Similarity, preview, hot-swap             | 3.3, 3.4, 4          |
| 5     | M25       | Device framework, Device View, racks, macros                      | 23, 25               |
| 5     | M26       | Audio effects: EQ, filter, analysis, utility                      | 29                   |
| 5     | M27       | Audio effects: dynamics                                           | 29                   |
| 5     | M28       | Audio effects: delay, reverb, looping, spectral time              | 29                   |
| 5     | M29       | Audio effects: distortion and character                           | 29                   |
| 5     | M30       | Audio effects: modulation, pitch, resonance, vocoder              | 29                   |
| 5     | M31       | MIDI effects and modulators                                       | 30, 33               |
| 5     | M32       | Instruments: sample based                                         | 31                   |
| 5     | M33       | Instruments: synthesizers                                         | 31                   |
| 5     | M34       | Instruments: physical modelling and external                      | 31                   |
| 6     | M35       | Grooves, tuning systems, scale awareness                          | 3.13, 14, 15         |
| 6     | M36       | Audio to MIDI, stem separation, bounce, freeze, consolidate       | 13, 20, 22, 38.1     |
| 6     | M37       | Plug-in hosting (CLAP, VST3, AU) with sandboxing                  | 24, 23.3             |
| 6     | M38       | Remote control, control surfaces, synchronisation                 | 34, 35, 36, 37       |
| 6     | M39       | Video                                                             | 28                   |
| 6     | M40       | Settings, Learn View, accessibility, keyboard navigation          | 2, 41, 42            |
| 6     | M41       | Performance: multicore, CPU and disk load, overload               | 38, 18.9             |
| 7     | M42       | Control server, MCP, and scripting                                | 32 (replacement)     |
| 7     | M43       | Reliability: autosave, version history, crash recovery            | none                 |
| 7     | M44       | Mixing and mastering extras                                       | none                 |
| 7     | M45       | Platforms, packaging, updates                                     | 2.1                  |
| 8     | M46       | Release 1.0: parity audit, blind tests, release                   | all                  |

Order rule: milestones run in table order. A milestone may start only when no earlier line in `CHECKLIST.md` is `[ ]` (`[~]` lines wait only on owner input and do not hold it back).

## Phase Exit Criteria

| Phase | Exit criteria (all measured by commands or recorded owner checks; owner-gated criteria close in the Reference pass after the phase)                                                                                                           |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0     | `cargo xtask ci` passes locally and in CI on Windows, Ubuntu, macOS; `cargo-deny` clean; edition 2024; egui 0.37 with wgpu; SECURITY.md findings LC-001 to LC-004 fixed with tests.                                                           |
| 1     | Target crate layout enforced by the layering check; audio callback runs under `assert_no_alloc` in tests with zero violations; every model edit is a registered command with an undo test; offline renders are bit-identical across two runs. |
| 2     | Every `.als` in the oracle corpus round-trips with zero XML differences when unmodified; the owner opens LiveCraft-saved sets in Live 12.4 without warnings (owner check).                                                                    |
| 3     | Main window shell matches reference captures within the thresholds in `UI-FIDELITY.md` for both default themes at 100 percent zoom.                                                                                                           |
| 4     | Every Live workflow in chapters 3 to 22 and 26 to 27 has a parity matrix row marked Done with evidence.                                                                                                                                       |
| 5     | Every device in chapters 29 to 31 is Done: parameters match the manual, UI matches captures, and oracle null tests meet `QUALITY-GATES.md#audio`.                                                                                             |
| 6     | Chapters 2, 13 to 15, 20, 22 to 24, 28, 34 to 42 Done; performance budgets in `QUALITY-GATES.md#performance` met.                                                                                                                             |
| 7     | Every item in `BEYOND-LIVE.md` marked Done with tests.                                                                                                                                                                                        |
| 8     | Release checklist in M46 complete; v1.0.0 tagged.                                                                                                                                                                                             |

## Scale And Honesty

- This is a multi-year scope. Live is the product of a large team over more than 20 years. The plan is ordered so every phase leaves a usable, better product, and so the most valuable parity (engine, files, core views) arrives first.
- "Indistinguishable" is judged by the blind tests in M46 and the measured thresholds, not by opinion.
- Owner decisions that block release are listed in `reference/DECISIONS.md#owner-decisions`.
