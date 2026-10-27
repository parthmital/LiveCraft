# LiveCraft

LiveCraft is an early-stage, clean-room digital audio workstation (DAW) written in Rust. Its goal is to match the interface, workflows, and behaviour of Ableton Live 12 (currently 12.4) and then go beyond it. It has a Cargo workspace with a document model, a DSP and audio engine, a desktop GUI built with egui, and a headless command-line interface (CLI).

LiveCraft is not affiliated with or endorsed by Ableton AG. "Ableton" and "Live" are trademarks of Ableton AG and appear here only to describe compatibility.

## Table of Contents

- [Project Status](#project-status)
- [Quick Start](#quick-start)
- [Features Today](#features-today)
- [Tech Stack](#tech-stack)
- [Repository Structure](#repository-structure)
- [CLI Usage](#cli-usage)
- [Configuration](#configuration)
- [Testing and Code Quality](#testing-and-code-quality)
- [Troubleshooting](#troubleshooting)
- [Known Limitations](#known-limitations)
- [Contributing](#contributing)
- [Licence](#licence)
- [Documentation](#documentation)

## Project Status

- Version `0.1.0`, pre-alpha. Not usable for real music production yet.
- The path from today's code to a full Ableton Live 12 equivalent, and beyond it, is the implementation plan in [plans/livecraft-1.0/](plans/livecraft-1.0/START-HERE.md).
- The plan's first phase replaces the parts of the current code that are not production quality. [ARCHITECTURE.md](ARCHITECTURE.md#known-violations) lists them.

## Quick Start

Run these from the repository root on Windows, macOS, or Linux.

```bash
cargo test --workspace
cargo run --bin livecraft -- new demo.als --bpm 128
cargo run --bin livecraft -- inspect demo.als --devices --clips
cargo run --bin livecraft -- render demo.als -o demo.wav --duration-bars 4
cargo run --bin livecraft -- gui
```

| Command                  | What it does                            | Expected result                               | Common error and fix                                                                          |
| ------------------------ | --------------------------------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `cargo test --workspace` | Builds every crate and runs all tests   | 44 tests pass (4 cli, 13 core, 25 dsp, 2 gui) | Linux link errors for ALSA or X11: install the packages in [Prerequisites](#prerequisites)    |
| `livecraft new demo.als` | Writes a new gzip-compressed Live Set   | `demo.als` is created                         | Permission denied: use a writable folder                                                      |
| `livecraft inspect ...`  | Prints tracks, device chains, and clips | A track and clip summary in the terminal      | `Decompression failed`: the file is not a gzip `.als`                                         |
| `livecraft render ...`   | Renders the set offline to a WAV file   | `demo.wav` exists                             | Silent output: the demo set may have no notes on unmuted tracks; check with `inspect --clips` |
| `livecraft gui`          | Opens the desktop window                | A Live-style window with Session View         | Window fails to open: update GPU drivers (the current renderer needs OpenGL 3.3)              |

`cargo run --bin livecraft -- <args>` and `target/debug/livecraft <args>` are equivalent. The first build takes several minutes.

### Prerequisites

- Rust stable with `rustfmt` and `clippy`. Baseline verified with Rust 1.99.1 on 2026-10-27.
- Linux only: `sudo apt-get install -y libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev libx11-dev` (the same list CI installs).
- Optional, for agents and contributors who read the reference manual: Python 3.11 and the packages in [plans/livecraft-1.0/tools/requirements.txt](plans/livecraft-1.0/tools/requirements.txt). See [docs/live-manual.md](docs/live-manual.md).

## Features Today

Verified in code on 2026-10-27. Fidelity to Live is partial everywhere; the plan's [parity matrix](plans/livecraft-1.0/reference/PARITY-MATRIX.md) tracks each feature.

- Document model (`livecraft-core`): project, tracks (audio, MIDI, return, master), clip slots, scenes, MIDI and audio clips, arrangement clips with split, duplicate, and trim, automation lanes with linear interpolation, 12 scale types, Scala `.scl` tuning, and a set of MIDI transformations.
- Live Set files: reads and writes gzip `.als` XML for the subset of elements the model knows. Unknown elements are dropped on save (see [Known Limitations](#known-limitations)).
- Undo and redo for a small set of commands (`CommandHistory`).
- Audio engine (`livecraft-dsp`): `cpal` output stream, per-track device chains, sends and returns, In, Auto, and Off monitoring, offline WAV render and stem export through `hound`.
- Devices: Operator-style 4-operator FM synth, Simpler-style sampler, a 16-pad drum rack, and effects named after EQ Eight, EQ Three, Compressor, Limiter, Gate, Auto Filter, Auto Pan, Chorus, Delay, Reverb, Saturator, Redux, and Utility. These are simplified approximations, not matched to Live's sound or parameter sets.
- GUI (`livecraft-gui`): control bar, Session View with mixer strips, Arrangement View, device and clip detail area, piano roll, and browser sidebar, in dark and light themes, plus a headless `--screenshot` mode.

## Tech Stack

| Technology            | Version in `Cargo.toml` | Purpose and where used                                            |
| --------------------- | ----------------------- | ----------------------------------------------------------------- |
| Rust                  | 2021 edition            | Whole workspace                                                   |
| `cpal`                | 0.15                    | Audio output stream in `crates/livecraft-dsp/src/engine.rs`       |
| `hound`               | 3.5                     | WAV read and write for render, stems, and samples                 |
| `egui`, `eframe`      | 0.29 (glow renderer)    | Immediate-mode desktop GUI in `crates/livecraft-gui`              |
| `image`               | 0.25 (PNG only)         | Saves `--screenshot` captures                                     |
| `quick-xml`, `flate2` | 0.36, 1.0               | `.als` XML parsing and gzip in `crates/livecraft-core/src/als.rs` |
| `crossbeam-channel`   | 0.5                     | Bounded parameter queue from GUI or CLI to the audio callback     |
| `clap`                | 4.5                     | CLI argument parsing in `crates/livecraft-cli`                    |
| `serde`, `serde_json` | 1.0                     | JSON project format and `inspect --json`                          |
| `anyhow`, `thiserror` | 1.0                     | Error handling                                                    |
| `log`, `env_logger`   | 0.4, 0.11               | Logging, configured through `RUST_LOG`                            |

## Repository Structure

```text
LiveCraft/
|-- Cargo.toml                 Workspace manifest and shared dependency versions
|-- crates/
|   |-- livecraft-core/        Document model, .als and .ask parsing, undo history (no audio, no UI)
|   |-- livecraft-dsp/         Audio graph, engine, device DSP, offline bouncer
|   |-- livecraft-gui/         egui desktop interface
|   `-- livecraft-cli/         `livecraft` binary: CLI subcommands and GUI launcher, integration tests
|-- docs/
|   |-- live-manual.md         How to use the Live 12 reference manual, with a chapter page map
|   |-- keyboard-shortcuts.md  Live 12 shortcut tables transcribed from manual chapter 42
|   `-- ableton-live-12-reference-manual.pdf   Reference manual (Git-ignored, 108 MB, kept locally)
|-- plans/livecraft-1.0/       End-to-end implementation plan, prompts, and reference material
|-- .github/                   CI workflow, issue and pull request templates
`-- LICENSE-MIT, LICENSE-APACHE
```

## CLI Usage

The binary is `livecraft`. Run `livecraft <command> --help` for every option. With no command it opens the GUI (`-p <file>` opens a project).

| Command                                                                                                              | Purpose                                                    |
| -------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| `livecraft new <file.als or file.json> [--bpm 120]`                                                                  | Create a new project                                       |
| `livecraft info <file>`                                                                                              | Print tempo, track, and scene counts                       |
| `livecraft inspect <file> [--devices] [--clips] [--json]`                                                            | Print tracks, device chains, and clips                     |
| `livecraft render <file> [-o output.wav] [-d 8] [-s 44100] [-b 24]`                                                  | Offline render to WAV; `-d` is bars, `-b` is 16, 24, or 32 |
| `livecraft stems <file> [-o ./stems] [-d 8]`                                                                         | Render one WAV per track                                   |
| `livecraft play [file] [-d seconds]`                                                                                 | Play through the default output with ASCII meters          |
| `livecraft synth [-n 60] [-w saw] [-o out.wav] [-d 2]`                                                               | Render or play a single test tone                          |
| `livecraft convert <input> <output>`                                                                                 | Convert between `.als` and `.json`                         |
| `livecraft gui [file] [--screenshot out.png] [--view session or arrangement] [--tab 0 or 1] [--theme dark or light]` | Open the GUI, or save one frame as PNG and exit            |

## Configuration

| Variable         | Required | Purpose                                             | Example            | Default             |
| ---------------- | -------- | --------------------------------------------------- | ------------------ | ------------------- |
| `RUST_LOG`       | Optional | Log level for `env_logger`; `-v` sets it to `debug` | `RUST_LOG=info`    | Unset (errors only) |
| `RUST_BACKTRACE` | Optional | Print a backtrace on panic                          | `RUST_BACKTRACE=1` | Unset               |

No secrets, accounts, or network services are used.

## Testing and Code Quality

`cargo xtask ci` runs all of these plus a crate layering check and a Markdown link check; run it before every commit. `cargo dev` launches the GUI. The CI workflow ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs the same checks. The individual commands are:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

Baseline on 2026-10-27: formatting clean, clippy clean, 44 tests pass. One test (`test_parse_real_ableton_template`) only runs its assertions when Ableton Live 12 Suite is installed at the default Windows path.

## Troubleshooting

| Problem                               | Likely cause                               | Diagnostic                       | Resolution                                              |
| ------------------------------------- | ------------------------------------------ | -------------------------------- | ------------------------------------------------------- |
| Build fails on Linux with ALSA errors | Missing system headers                     | `pkg-config --libs alsa`         | Install the packages in [Prerequisites](#prerequisites) |
| `play` or GUI has no sound            | No default output device, or engine failed | `RUST_LOG=debug livecraft play`  | Select a default output device in the OS                |
| Audio crackles while using the GUI    | GUI and audio thread share a mutex         | Reproduce while dragging a fader | Known issue, fixed in the plan's Phase 1                |
| `.als` loses data after `convert`     | Writer only emits elements the model knows | Compare XML of input and output  | Known issue, fixed in the plan's Phase 2                |

## Known Limitations

- The audio callback locks a mutex shared with the GUI and outputs silence when it cannot get it, and it clones clip data every block. It is not real-time safe yet.
- `.als` round trips are lossy, and only Live 12 sets were tested.
- Most of Live 12 is missing: warping, recording, routing, racks, automation editing, most devices, plug-in hosting, MIDI input, and preferences. The [parity matrix](plans/livecraft-1.0/reference/PARITY-MATRIX.md) has the full list.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). Agents read [AGENTS.md](AGENTS.md) first.

## Licence

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

## Documentation

| File                                                                   | Answers                                                         |
| ---------------------------------------------------------------------- | --------------------------------------------------------------- |
| [AGENTS.md](AGENTS.md)                                                 | What must an agent know in every task here?                     |
| [ARCHITECTURE.md](ARCHITECTURE.md)                                     | How is the code organised today, and what is wrong with it?     |
| [DESIGN.md](DESIGN.md)                                                 | How does the UI look and behave?                                |
| [SECURITY.md](SECURITY.md)                                             | How is the project protected, and what is at risk?              |
| [RESEARCH.md](RESEARCH.md)                                             | What was evaluated before choosing libraries and approaches?    |
| [CONTRIBUTING.md](CONTRIBUTING.md)                                     | How do outsiders contribute?                                    |
| [docs/live-manual.md](docs/live-manual.md)                             | Where in the Live 12 manual is each topic, and how to query it? |
| [docs/keyboard-shortcuts.md](docs/keyboard-shortcuts.md)               | Which keyboard shortcuts does Live 12 define?                   |
| [plans/livecraft-1.0/START-HERE.md](plans/livecraft-1.0/START-HERE.md) | How does the project get from today to 1.0?                     |
