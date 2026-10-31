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
- The path from today's code to a full Ableton Live 12 equivalent, and beyond it, is the implementation plan in [plans/livecraft-1.0/PLAN.md](plans/livecraft-1.0/PLAN.md); progress is tracked in [CHECKLIST.md](plans/livecraft-1.0/CHECKLIST.md).
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

| Command                  | What it does                            | Expected result                          | Common error and fix                                                                          |
| ------------------------ | --------------------------------------- | ---------------------------------------- | --------------------------------------------------------------------------------------------- |
| `cargo test --workspace` | Builds every crate and runs all tests   | 55 tests pass                            | Linux link errors for ALSA or X11: install the packages in [Prerequisites](#prerequisites)    |
| `livecraft new demo.als` | Writes a new gzip-compressed Live Set   | `demo.als` is created                    | Permission denied: use a writable folder                                                      |
| `livecraft inspect ...`  | Prints tracks, device chains, and clips | A track and clip summary in the terminal | `Decompression failed`: the file is not a gzip `.als`                                         |
| `livecraft render ...`   | Renders the set offline to a WAV file   | `demo.wav` exists                        | Silent output: the demo set may have no notes on unmuted tracks; check with `inspect --clips` |
| `livecraft gui`          | Opens the desktop window                | A Live-style window with Session View    | Window fails to open: update GPU drivers; on Windows set WGPU_BACKEND=gl to try OpenGL        |

`cargo run --bin livecraft -- <args>` and `target/debug/livecraft <args>` are equivalent. The first build takes several minutes.

### Prerequisites

- Rust stable with `rustfmt` and `clippy`. Baseline verified with Rust 1.99.1 on 2026-10-27.
- Linux only: `sudo apt-get install -y libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev libx11-dev` (the same list CI installs).
- Optional, for agents and contributors who read the reference manual: Python 3.11 and the packages in [plans/livecraft-1.0/tools/requirements.txt](plans/livecraft-1.0/tools/requirements.txt), installed into a repo-local environment (never globally). See [docs/live-manual.md](docs/live-manual.md).

  ```bash
  python -m venv .venv
  ./.venv/Scripts/python -m pip install -r plans/livecraft-1.0/tools/requirements.txt
  ```

  On macOS or Linux use `./.venv/bin/python`.

## Features Today

Verified in code on 2026-10-27. Fidelity to Live is partial everywhere; the plan's [parity matrix](plans/livecraft-1.0/reference/PARITY-MATRIX.md) tracks each feature.

- Document model (`livecraft-model`): project, tracks (audio, MIDI, return, master), clip slots, scenes, MIDI and audio clips, arrangement clips with split, duplicate, and trim, automation lanes with linear interpolation, 12 scale types, Scala `.scl` tuning, and a set of MIDI transformations.
- Live Set files (`livecraft-formats`): reads and writes gzip `.als` XML for the subset of elements the model knows. Unknown elements are dropped on save (see [Known Limitations](#known-limitations)).
- Undo and redo for a small set of commands (`CommandHistory`).
- Audio engine (`livecraft-engine`): `cpal` output stream, per-track device chains, sends and returns, In, Auto, and Off monitoring, offline WAV render and stem export through `hound`.
- Devices (`livecraft-devices`, built on filters in `livecraft-dsp`): Operator-style 4-operator FM synth, Simpler-style sampler, a 16-pad drum rack, and effects named after EQ Eight, EQ Three, Compressor, Limiter, Gate, Auto Filter, Auto Pan, Chorus, Delay, Reverb, Saturator, Redux, and Utility. These are simplified approximations, not matched to Live's sound or parameter sets.
- GUI (`livecraft-ui`): control bar, Session View with mixer strips, Arrangement View, device and clip detail area, piano roll, and browser sidebar, in dark and light themes, plus a headless `--screenshot` mode.

## Tech Stack

| Technology            | Version in `Cargo.toml` | Purpose and where used                                               |
| --------------------- | ----------------------- | -------------------------------------------------------------------- |
| Rust                  | 2024 edition            | Whole workspace                                                      |
| `cpal`                | 0.18                    | Audio output stream in `crates/livecraft-engine/src/engine.rs`       |
| `hound`               | 3.5                     | WAV read and write for render, stems, and samples                    |
| `egui`, `eframe`      | 0.37 (wgpu renderer)    | Immediate-mode desktop GUI in `crates/livecraft-ui`                  |
| `image`               | 0.25 (PNG only)         | Saves `--screenshot` captures                                        |
| `quick-xml`, `flate2` | 0.42, 1.0               | `.als` XML parsing and gzip in `crates/livecraft-formats/src/als.rs` |
| `crossbeam-channel`   | 0.5                     | Bounded parameter queue from GUI or CLI to the audio callback        |
| `clap`                | 4.5                     | CLI argument parsing in `apps/livecraft`                             |
| `serde`, `serde_json` | 1.0                     | JSON project format and `inspect --json`                             |
| `anyhow`, `thiserror` | 1.0, 2                  | Error handling                                                       |
| `log`, `env_logger`   | 0.4, 0.11               | Logging, configured through `RUST_LOG`                               |

## Repository Structure

```text
LiveCraft/
|-- .cargo/
|-- .github/
|-- apps/
|   `-- livecraft/
|-- crates/
|   |-- livecraft-devices/
|   |-- livecraft-dsp/
|   |-- livecraft-engine/
|   |-- livecraft-formats/
|   |-- livecraft-model/
|   |-- livecraft-testkit/
|   |-- livecraft-time/
|   `-- livecraft-ui/
|-- docs/
|-- plans/
|   `-- livecraft-1.0/
|-- xtask/
|-- Cargo.toml
|-- deny.toml
`-- rust-toolchain.toml
```

- `apps/livecraft/`: the `livecraft` binary (package `livecraft`), which opens the GUI or runs CLI subcommands.
- `crates/`: one Cargo package per layer; responsibilities and allowed dependencies are in [ARCHITECTURE.md](ARCHITECTURE.md#module-map).
- `docs/`: the Live 12 manual guide and shortcut tables. The manual PDF itself is Git-ignored and kept locally.
- `plans/livecraft-1.0/`: the roadmap to 1.0: phases (`PLAN.md`), progress (`CHECKLIST.md`), owner-only requests (`OWNER-QUEUE.md`), milestone specs, plan-wide reference decisions, pending task files, and the manual query tool.
- `xtask/`: `cargo xtask ci` (format, clippy, tests, release build, crate layering, Markdown links, cargo-deny).
- `.cargo/config.toml`: the `cargo xtask` and `cargo dev` aliases. `rust-toolchain.toml` pins Rust 1.99.1. `deny.toml` holds the dependency licence and advisory policy.

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

`cargo xtask ci` runs all of these plus a crate layering check, a Markdown link check, and `cargo-deny`; run it before every commit. `cargo dev` launches the GUI. CI ([.github/workflows/ci.yml](.github/workflows/ci.yml)) runs `cargo xtask ci --no-deny` on Windows, Ubuntu, and macOS, `cargo-deny` in a separate job, and uploads the Windows executable as an artifact. Dependabot proposes weekly dependency and action updates. The individual commands are:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release --workspace
```

Baseline on 2026-10-31: `cargo xtask ci` passes with 55 tests (livecraft 6, livecraft-model 9, livecraft-formats 6, livecraft-dsp 1, livecraft-devices 17, livecraft-engine 10, livecraft-ui 2, xtask 4). One test (`test_parse_real_ableton_template`) only runs its assertions when Ableton Live 12 Suite is installed at the default Windows path.

## Troubleshooting

| Problem                               | Likely cause                                    | Diagnostic                       | Resolution                                              |
| ------------------------------------- | ----------------------------------------------- | -------------------------------- | ------------------------------------------------------- |
| Build fails on Linux with ALSA errors | Missing system headers                          | `pkg-config --libs alsa`         | Install the packages in [Prerequisites](#prerequisites) |
| `play` or GUI has no sound            | No default output device, or engine failed      | `RUST_LOG=debug livecraft play`  | Select a default output device in the OS                |
| Audio crackles while using the GUI    | GUI and audio thread share a mutex              | Reproduce while dragging a fader | Known issue, fixed in the plan's Phase 1                |
| `.als` loses data after `convert`     | Writer only emits elements the model knows      | Compare XML of input and output  | Known issue, fixed in the plan's Phase 2                |
| GUI crashes at start on Windows       | GPU driver problem in the selected wgpu backend | WGPU_BACKEND=gl cargo dev        | Default is DX12; try gl, or update drivers              |

## Known Limitations

- The audio callback locks a mutex shared with the GUI and outputs silence when it cannot get it, and it clones clip data every block. It is not real-time safe yet.
- `.als` round trips are lossy, and only Live 12 sets were tested.
- Most of Live 12 is missing: warping, recording, routing, racks, automation editing, most devices, plug-in hosting, MIDI input, and preferences. The [parity matrix](plans/livecraft-1.0/reference/PARITY-MATRIX.md) has the full list.

## Contributing

Read [CONTRIBUTING.md](CONTRIBUTING.md). Agents read [AGENTS.md](AGENTS.md) first.

## Licence

Copyright 2026 Parth Mital. Licensed under the [GNU General Public License v3.0 or later](LICENSE).

## Documentation

| File                                                       | Answers                                                         |
| ---------------------------------------------------------- | --------------------------------------------------------------- |
| [AGENTS.md](AGENTS.md)                                     | What must an agent know in every task here?                     |
| [ARCHITECTURE.md](ARCHITECTURE.md)                         | How is the code organised today, and what is wrong with it?     |
| [DESIGN.md](DESIGN.md)                                     | How does the UI look and behave?                                |
| [SECURITY.md](SECURITY.md)                                 | How is the project protected, and what is at risk?              |
| [RESEARCH.md](RESEARCH.md)                                 | What was evaluated before choosing libraries and approaches?    |
| [CONTRIBUTING.md](CONTRIBUTING.md)                         | How do outsiders contribute?                                    |
| [docs/live-manual.md](docs/live-manual.md)                 | Where in the Live 12 manual is each topic, and how to query it? |
| [docs/keyboard-shortcuts.md](docs/keyboard-shortcuts.md)   | Which keyboard shortcuts does Live 12 define?                   |
| [plans/livecraft-1.0/PLAN.md](plans/livecraft-1.0/PLAN.md) | How does the project get from today to 1.0?                     |
