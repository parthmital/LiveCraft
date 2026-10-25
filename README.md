# LiveCraft

A clean-room, native Rust digital audio workstation (DAW) engineered for deterministic audio rendering, low latency, and structural parity with Ableton Live 12. Inspired by storytold architectural patterns seen in PhotoCraft and FilmCraft.

LiveCraft combines a real-time audio engine, faster-than-realtime offline bouncing, authentic Ableton Live set (`.als`) compatibility, headless CLI workflows, and a native desktop interface styled after Ableton Live 12.

## Table of Contents

- [Overview](#overview)
- [Authoritative Ableton Live 12 Reference Suite](#authoritative-ableton-live-12-reference-suite)
- [Quick Start](#quick-start)
- [Key Capabilities](#key-capabilities)
- [Workspace Architecture](#workspace-architecture)
- [Tech Stack](#tech-stack)
- [Environment Variables](#environment-variables)
- [Installation and Build](#installation-and-build)
- [CLI Usage](#cli-usage)
- [Graphical User Interface](#graphical-user-interface)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Testing and Verification](#testing-and-verification)
- [Metrics](#metrics)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [Licence](#licence)

## Overview

LiveCraft provides an open-source, memory-safe alternative to proprietary DAWs. It is built entirely in pure Rust with zero web view or Electron overhead. The system uses a decoupled pure data model in [livecraft-core](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/lib.rs) and a lock-free, allocation-free audio pipeline in [livecraft-dsp](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/lib.rs). All graphical operations in [livecraft-gui](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/lib.rs) are also exposed through the headless [livecraft-cli](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-cli/src/main.rs).

## Authoritative Ableton Live 12 Reference Suite

The specification and mathematical formulas of LiveCraft are grounded in the official Ableton Live 12 Reference Manual (`docs/ableton-live-12-reference-manual.pdf`, 108 MB, 1039 pages, 43 chapters):

- [Ableton Live 12 Complete System Specification](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/ABLETON_12_SPECIFICATION.md): Maps all 43 chapters of the reference manual to LiveCraft modules.
- [Complete Devices Reference Guide](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/DEVICES_REFERENCE.md): Technical catalog of 42 Audio Effects, 8 MIDI Effects, and 13 Instruments.
- [MIDI Tools, Scales and Microtonal Tuning](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/MIDI_TOOLS_AND_TUNING.md): Details transformation algorithms, generative Euclidean rhythms, scale engines, and Scala `.scl` tuning.
- [Complete Keyboard Shortcuts Matrix](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/KEYBOARD_SHORTCUTS.md): Comprehensive shortcut mapping across Windows and macOS from Chapter 42.
- [LiveCraft Technical Architecture](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/ARCHITECTURE.md): Engineering manual covering crate boundaries, lock-free queues, dynamic topological DSP chains, and offline bouncers.
- [LiveCraft UI/UX Design System](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/DESIGN.md): Ableton Live 12 design tokens, layout proportions, typography, and widget specifications.

## Quick Start

Follow these verified steps from the repository root to verify and run LiveCraft:

```bash
# Step 1: Run the full test suite (verifies 44 unit and integration tests)
cargo test --workspace

# Step 2: Build the optimized release binary
cargo build --release

# Step 3: Create a fresh Ableton Live set template
cargo run --release --bin livecraft -- new DemoTrack.als --bpm 128

# Step 4: Inspect the generated project structure
cargo run --release --bin livecraft -- inspect DemoTrack.als --devices --clips

# Step 5: Render 4 musical bars to a 24-bit 44.1 kHz WAV file
cargo run --release --bin livecraft -- render DemoTrack.als -o demo_output.wav --duration-bars 4

# Step 6: Launch the Ableton Live 12 desktop interface
cargo run --release --bin livecraft -- gui
```

Quick Start command breakdown:

| Command                                     | Action                     | Directory       | Expected Result                         | Common Errors and Fixes                                                                |
| :------------------------------------------ | :------------------------- | :-------------- | :-------------------------------------- | :------------------------------------------------------------------------------------- |
| `cargo test --workspace`                    | Runs all 44 tests          | Repository root | All tests pass with 0 failures          | Missing sound dependencies on Linux: run `sudo apt install libasound2-dev libudev-dev` |
| `cargo build --release`                     | Compiles release binary    | Repository root | Executable created in `target/release/` | Rust toolchain outdated: run `rustup update stable`                                    |
| `livecraft new DemoTrack.als --bpm 128`     | Initializes project set    | Any directory   | Creates valid gzip `.als` XML file      | File permission error: choose a writable directory path                                |
| `livecraft render DemoTrack.als -o out.wav` | Offline audio bounce       | Any directory   | Generates 24-bit stereo WAV             | Invalid duration: ensure `--duration-bars` is greater than 0                           |
| `livecraft gui`                             | Launches desktop interface | Any directory   | Ableton Live 12 desktop window appears  | Missing OpenGL drivers: ensure GPU drivers support OpenGL 3.3 or higher                |

## Key Capabilities

- Pure Native Rust Stack: Audio engine, DSP graph, format parser, CLI, and GUI written in 100% native Rust without web view or Electron dependencies.
- Authentic Ableton Live Set Compatibility: Direct decompression and streaming XML parsing of gzip `.als` archives. Supports Live 10, 11, and 12 sets, extracting tracks, scenes, clips, notes, and devices, and exporting projects back to valid `.als` files.
- Dynamic Modular DSP Processor Chain: Standardized [`DspProcessor`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/processor.rs) trait allowing dynamic insertion, removal, and reordering of rack devices in topological order.
- Lock-Free Audio Concurrency: GUI and CLI interact with the real-time audio thread using lock-free parameter message queues ([`AudioParamMsg`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/engine.rs)) without heap allocations or mutex contention.
- Sample-Accurate Voice Allocation: MIDI note events are scheduled with sub-block sample offsets, polyphonic voice stealing, and note instance lifecycle tracking.
- Continuous Audio Streaming: Non-destructive timeline audio streaming with cubic Hermite interpolation in [`AudioSampleCache`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/audio_clip.rs).
- In/Auto/Off Monitoring and Return Summing: Channel monitoring modes matching Live Manual Chapter 15, alongside multi-channel return track summing with 32-bit floating point headroom.
- Built-In Studio DSP Suite: 16 studio-grade instruments and audio processors:
  - [`OperatorSynth`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/operator.rs): 4-operator polyphonic FM synthesizer with 11 routing algorithms, per-op waveforms, ADSR envelopes, feedback, and resonant filter.
  - [`SimplerInstrument`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/simpler.rs): Multi-mode polyphonic sampler supporting Classic, 1-Shot, and Slicing modes.
  - [`DrumRackInstrument`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/drumrack.rs): 16-pad drum machine with choke groups (hi-hats) and per-pad solo, mute, and pan routing.
  - [`EqEight`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/eq8.rs): 8-band parametric biquad equalizer with 7 filter topologies and analytical response curve calculation.
  - Dynamics Suite: [`CompressorEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/compressor.rs), [`LimiterEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/limiter.rs), and [`GateEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/gate.rs).
  - Time and Modulation: [`DelayEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/delay.rs), [`ReverbEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/reverb.rs), [`ChorusEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/chorus.rs), and [`AutoPanEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/autopan.rs).
  - Colour and Filter: [`AutoFilterEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/autofilter.rs), [`SaturatorEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/saturator.rs), [`EqThreeEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/eq3.rs), [`ReduxEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/redux.rs), and [`UtilityEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/limiter.rs).
- Scale Systems and Microtonal Tuning: Diatonic scale awareness across 12 roots and 12 modes, Fold to Scale piano roll, and native Scala `.scl` file format support.
- Non-Destructive Timeline Editing: Arrangement clip splitting, duplicating, and trimming with breakpoint automation curves.
- High-Speed Offline Bouncer: Offline project rendering to 16, 24, or 32-bit float WAV files approximately 133 times faster than real-time, plus multi-track stem exports.

## Workspace Architecture

```text
LiveCraft/
|-- Cargo.toml                  # Workspace root configuration
|-- docs/                       # Authoritative Ableton Live 12 reference suite
|   |-- ableton-live-12-reference-manual.pdf  # Definitive 1039-page manual
|   |-- ABLETON_12_SPECIFICATION.md          # 43-chapter system mapping
|   |-- DEVICES_REFERENCE.md                 # 42 audio FX, 8 MIDI FX, 13 synths catalog
|   |-- MIDI_TOOLS_AND_TUNING.md             # Transformations, Generators, Scala engine
|   |-- KEYBOARD_SHORTCUTS.md                # Comprehensive shortcut mapping
|   |-- ARCHITECTURE.md                      # Engineering manual
|   `-- README.md                            # Documentation overview
|-- crates/
|   |-- livecraft-core/         # Pure data document model, ALS/ASK parser, command history
|   |   `-- src/
|   |       |-- model.rs        # Project, Track, Clip, Scale, Tuning, Automation
|   |       |-- als.rs          # Ableton .als XML deserializer and serializer
|   |       |-- theme.rs        # .ask Theme parser and palette mapping
|   |       |-- command.rs      # Undo/Redo command history stack
|   |       `-- lib.rs
|   |-- livecraft-dsp/          # Audio graph, DSP units, synth, audio engine
|   |   `-- src/
|   |       |-- engine.rs       # Real-time CPAL stream and lock-free parameter dispatch
|   |       |-- graph.rs        # Multi-track audio graph and voice scheduling
|   |       |-- processor.rs    # DspProcessor trait and dynamic modular chain
|   |       |-- audio_clip.rs   # AudioSampleCache and Hermite interpolation
|   |       |-- bouncer.rs      # Offline faster-than-realtime WAV and stem exporter
|   |       |-- operator.rs     # 4-operator FM synthesizer with 11 routing algorithms
|   |       |-- simpler.rs      # Polyphonic sampler with Classic, 1-Shot, Slicing modes
|   |       |-- drumrack.rs     # 16-pad drum machine with choke groups
|   |       |-- eq8.rs          # 8-band parametric biquad equalizer and response curve
|   |       |-- compressor.rs   # Dynamics compressor with knee and metering
|   |       |-- limiter.rs      # Lookahead brickwall limiter and utility effect
|   |       |-- gate.rs         # Noise gate with hysteresis return
|   |       |-- autofilter.rs   # Resonant multi-mode filter
|   |       |-- autopan.rs      # Auto Pan-Tremolo LFO stereo panner
|   |       |-- chorus.rs       # Multi-delay chorus ensemble
|   |       |-- delay.rs        # Stereo ping-pong and tempo-synced delay
|   |       |-- eq3.rs          # 3-band DJ style isolator equalizer
|   |       |-- redux.rs        # Sample rate and bitcrusher effect
|   |       |-- reverb.rs       # Algorithmic room and hall reverb
|   |       |-- saturator.rs    # Waveshaping harmonic distortion
|   |       `-- lib.rs
|   |-- livecraft-gui/          # Ableton Live 12 desktop GUI with egui and glow
|   |   `-- src/
|   |       |-- app.rs          # Main application loop and shortcuts
|   |       |-- transport.rs    # 9-section Control Bar
|   |       |-- session.rs      # Session View clip matrix and scene launches
|   |       |-- arrange.rs      # Arrangement View timeline tracks and ruler
|   |       |-- mixer.rs        # Track strips, volume faders, VU meters
|   |       |-- devices.rs      # Bottom modular device rack cards
|   |       |-- pianoroll.rs    # Piano roll note editor and scale fold
|   |       |-- browser.rs      # Left asset taxonomy browser
|   |       |-- style.rs        # Ableton Live 12 color theme and widgets
|   |       `-- widgets.rs      # Custom knobs, faders, meters
|   `-- livecraft-cli/          # Headless CLI entry point and subcommands
|       |-- tests/              # CLI integration test harness
|       `-- src/
|           `-- main.rs         # CLI command definitions
|-- tasks/                      # Autonomous 28-task execution system
|   |-- manifest.json           # Machine-readable task registry
|   |-- state.json              # Living progress tracker
|   |-- AUTONOMOUS_EXECUTION.md # Autonomous execution protocol
|   `-- specs/                  # Task specification files (task-001 to task-028)
|-- scripts/                    # Automation and task runner tools
|-- ROADMAP.md                  # Master roadmap and parity audit
|-- RESEARCH.md                 # Research dossier and technology selection
|-- CONTRIBUTING.md             # Contributor guidelines
`-- Cargo.lock
```

## Tech Stack

| Technology             | Version              | Purpose                                      | Where Used                     | Why Needed                                                                 | Interaction                                                                    |
| :--------------------- | :------------------- | :------------------------------------------- | :----------------------------- | :------------------------------------------------------------------------- | :----------------------------------------------------------------------------- |
| Rust                   | 2021 edition (1.80+) | Programming language                         | Entire workspace               | Memory safety, zero-cost abstractions, predictable low-latency performance | Compiles all crates into native binaries                                       |
| `cpal`                 | 0.15                 | Low-latency audio driver abstraction         | `livecraft-dsp/src/engine.rs`  | Cross-platform hardware audio stream management                            | Supplies audio callback executing hardware buffers via WASAPI, CoreAudio, ALSA |
| `hound`                | 3.5                  | WAV audio encoder and decoder                | `livecraft-dsp/src/bouncer.rs` | Sample loading and faster-than-realtime offline export                     | Reads and writes 16, 24, and 32-bit float WAV files                            |
| `egui` / `eframe`      | 0.28                 | Immediate-mode desktop UI framework          | `livecraft-gui/`               | High-performance reactive UI rendering                                     | Renders Ableton Live 12 layout with zero state synchronization lag             |
| `glow`                 | 0.13                 | Hardware-accelerated OpenGL bindings         | `livecraft-gui/`               | Cross-platform GPU rendering                                               | Backs `eframe` for desktop rendering                                           |
| `flate2`               | 1.0                  | Pure Rust gzip decompression and compression | `livecraft-core/src/als.rs`    | Ableton Live set archive decompression and generation                      | Decompresses and compresses `.als` files                                       |
| `quick-xml`            | 0.36                 | High-throughput streaming XML parser         | `livecraft-core/src/als.rs`    | Parsing and serializing Ableton Live project XML                           | Reads and writes authentic Ableton XML schemas                                 |
| `crossbeam-channel`    | 0.5                  | Lock-free bounded communication queues       | `livecraft-dsp/`               | Decoupling GUI/CLI from audio thread                                       | Dispatches parameter changes without mutex blocking                            |
| `clap`                 | 4.5                  | Command line argument parsing                | `livecraft-cli/`               | Headless CLI interface and subcommands                                     | Maps terminal arguments to DAW execution commands                              |
| `serde` / `serde_json` | 1.0                  | Serialization framework                      | `livecraft-core/`              | JSON conversion, state tracking, inspection                                | Serializes document models to human-readable JSON                              |

## Environment Variables

No required environment variables are needed to build, test, or run LiveCraft. The application is entirely self-contained. Optional standard variables include:

| Variable         | Required | Purpose                       | Format                            | Safe Example       | Default | Security Notes                          |
| :--------------- | :------- | :---------------------------- | :-------------------------------- | :----------------- | :------ | :-------------------------------------- |
| `RUST_LOG`       | Optional | Controls logging verbosity    | String (`info`, `debug`, `trace`) | `RUST_LOG=info`    | None    | Local diagnostics only                  |
| `RUST_BACKTRACE` | Optional | Displays stack trace on panic | Integer (`1`)                     | `RUST_BACKTRACE=1` | 0       | Do not expose in public production logs |

## Installation and Build

### Prerequisites

- Rust 1.80 or later (Rust 2021 edition)
- Windows 10/11, macOS, or Linux
- On Linux, install development audio and windowing packages:
  `sudo apt install libasound2-dev libudev-dev libxkbcommon-dev libwayland-dev libx11-dev`

### Building from Source

```bash
# Clone the repository
git clone https://github.com/storytold/livecraft.git
cd livecraft

# Run full test suite
cargo test --workspace

# Build optimized release binary
cargo build --release

# The compiled executable is available at:
# target/release/livecraft.exe (Windows) or target/release/livecraft (macOS/Linux)
```

## CLI Usage

LiveCraft is fully operable from the terminal for automation, testing, and audio processing:

### 1. Project Information

Prints project metadata, Ableton creator, version, tempo, track count, and scene count:

```bash
livecraft info path/to/project.als
```

### 2. Deep Project Inspection

Inspects track volumes, pan positions, device chains, clip slots, and MIDI note counts:

```bash
livecraft inspect path/to/project.als --devices --clips

# Output as JSON for scripting and automation
livecraft inspect path/to/project.als --json
```

### 3. Create a New Project

Initializes a new Ableton Live set or JSON project:

```bash
livecraft new MyTrack.als --bpm 128
livecraft new MyTrack.json --bpm 140
```

### 4. Offline Audio Bounce

Renders an Ableton set to a WAV audio file faster than real time:

```bash
livecraft render MyTrack.als -o output.wav --duration-bars 8 --sample-rate 44100 --bits 24
```

### 5. Multi-Track Stem Export

Bounces every track in the project to an isolated stem WAV file in an output directory:

```bash
livecraft stems MyTrack.als -o ./stems_dir --duration-bars 16
```

### 6. Terminal Audio Playback

Plays the project through your default audio output with live ASCII stereo VU meters and position tracking:

```bash
livecraft play path/to/project.als

# Play default demo session
livecraft play --duration 10
```

### 7. Synthesizer Tone Generation

Renders synthetic waveforms directly from the command line:

```bash
livecraft synth --note 60 --waveform saw --duration-sec 2.0 -o synth_c4.wav
```

### 8. Bidirectional Format Conversion

Converts between Ableton `.als` (gzipped XML) and human-readable JSON:

```bash
livecraft convert project.als project.json
livecraft convert project.json project.als
```

### 9. Launch Desktop GUI

```bash
livecraft gui

# Open directly with a project file
livecraft gui path/to/project.als

# Launch with light or dark theme matching Ableton Live 12
livecraft gui --theme light
livecraft gui --theme dark

# Headless screenshot capture for visual CI and testing
livecraft gui --screenshot capture.png --view session --theme dark
livecraft gui --screenshot capture_arr.png --view arrangement --theme light
```

## Graphical User Interface

The desktop application recreates Ableton Live 12's signature interface layout:

- Top Control Bar (Manual Ch. 3.1): 9-section Control Bar with Browser toggle, Link, Tap tempo, BPM LCD well with up/down nudge, Time Signature LCD, Metronome, Quantization menu, Live 12 Scale toggle and Root/Mode selector, Follow toggle, Arrangement Position LCD (Bar . Beat . 16th), Play triangle, Stop square, Record circle, Automation and MIDI overdub/arm/capture controls, Arrangement Loop controls (punch in/out, start, length, loop toggle), Draw mode, Computer MIDI Keyboard, KEY/MIDI map modes, CPU load meter with real-time bar, and View Selector (Session / Arrangement).
- Session View (`Tab`): Track headers with color tags, vertical clip slot matrix with circular stop placeholders and solid left color badges, launch arrows, real-time animated loop progress fills, track stop buttons, Master Scene launch column with green launch triangles, and Stop All Clips.
- Mixer View (Manual Ch. 18): In/Auto/Off monitoring buttons, Send A and Send B rotary knobs, Pan knob with center detent readout, vertical volume faders with graduated dB ticks (+6, 0, -6, -12, -24, -inf), 24-segment stereo peak VU meters with clipping indicators, numeric dB LCD display boxes, illuminated Track Activators (amber numbers), Solo (blue), and Arm (red) buttons.
- Arrangement View (`Tab`): Horizontal timeline with bar/beat time ruler, draggable loop brace with start/end boundary flags, scrubbing playhead marker, alternating lane backgrounds with grid lines, track controls headers, and clip representations with MIDI note previews and audio waveform contours.
- Bottom Detail Panel (Manual Ch. 3.12, 23.1):
  - Device Chain (Device View): Horizontal modular rack showing selected track's devices (EQ Eight, Auto Filter, Auto Pan-Tremolo, Gate, EQ Three, Redux, Saturator, Compressor, Delay, Reverb, Operator, Utility) with parameter controls, ADSR curves, and filter response displays.
  - Clip Detail / Piano Roll (Clip View): Clip properties LCD displays, loop toggle, diatonic scale awareness engine, piano keyboard with pitch labels (C3, C4), in-scale key highlighting, note insertion/removal grid, moving playhead indicator, and MIDI toolbars (Chop, Ornament, Span, Euclid, Stacks, Quantize, Transpose, Strum, Humanize, Invert, Reverse, Velocity Ramp).
- Left Browser Sidebar (Manual Ch. 4): Search bar, 7-color collection tags (Favorites, Synths, Drums, Bass, FX, Templates, Other), Core Library categories (Sounds, Drums, Instruments, Audio Effects, MIDI Effects, Plug-Ins, Clips, Samples), and Places (Packs, User Library, Current Project).
- Bottom Status Bar (Manual Ch. 3.2): Info View toggle button, contextual status readout, and audio engine rate/bit-depth badge.

## Keyboard Shortcuts

Standard Ableton Live 12 shortcuts from Chapter 42:

- `Space`: Play / Stop playback
- `Tab`: Switch between Session View and Arrangement View
- `Shift + Tab` or `F12`: Toggle between Device View and Clip View
- `F8`: Toggle between Dark Neutral and Light Themes
- `Ctrl + T`: Insert Audio Track
- `Ctrl + Shift + T`: Insert MIDI Track
- `0`: Toggle Mute / Track Activator on selected track
- `S`: Toggle Solo on selected track
- `C`: Toggle Arm on selected track
- `Ctrl + Z`: Undo
- `Ctrl + Y`: Redo
- `Ctrl + S`: Save project

See the complete matrix in [KEYBOARD_SHORTCUTS.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/KEYBOARD_SHORTCUTS.md).

## Testing and Verification

LiveCraft is continuously tested using automated unit, integration, and command-line verification:

```bash
# Run the complete test suite across all crates
cargo test --workspace

# Check formatting compliance
cargo fmt --all -- --check

# Check linter compliance with all warnings treated as errors
cargo clippy --workspace --all-targets -- -D warnings
```

Test Results Breakdown (44 tests passing):

- `livecraft-cli` integration tests: 4 passed
- `livecraft-core` unit tests: 13 passed
- `livecraft-dsp` unit tests: 25 passed
- `livecraft-gui` unit tests: 2 passed (db_to_norm roundtrip & boundary precision)
- Total: 44 passed; 0 failed; 0 warnings
  - `test_cli_midi_transformations_and_tuning`: ok
  - `test_cli_new_and_roundtrip_project`: ok
  - `test_cli_render_and_stem_pipeline`: ok
  - `test_cli_synth_to_wav`: ok
- `livecraft-core` unit tests: 13 passed
  - `test_project_creation`: ok
  - `test_theme_colors`: ok
  - `test_theme_hex_variants`: ok
  - `test_device_defaults`: ok
  - `test_clip_slot_and_payload_manipulation`: ok
  - `test_als_xml_export_and_import`: ok
  - `test_als_full_roundtrip_with_devices_and_clips`: ok
  - `test_command_undo_redo`: ok
  - `test_command_history_capacity`: ok
  - `test_scale_system_and_snapping`: ok
  - `test_midi_transformations`: ok
  - `test_tuning_system`: ok
  - `test_parse_real_ableton_template`: ok
- `livecraft-dsp` unit tests: 25 passed
  - `test_biquad_all_filter_types`: ok
  - `test_eq8_processing`: ok
  - `test_saturator_processing`: ok
  - `test_delay_processing`: ok
  - `test_reverb_processing`: ok
  - `test_compressor_gain_reduction`: ok
  - `test_utility_effect`: ok
  - `test_operator_synth_note`: ok
  - `test_operator_all_waveforms`: ok
  - `test_limiter_brickwall`: ok
  - `test_audio_graph_summing`: ok
  - `test_offline_bounce`: ok
  - `test_stem_exporter`: ok
  - `test_device_parameter_synchronization`: ok
  - `test_arrangement_timeline_playback`: ok
  - `test_graph_dynamic_track_sync`: ok
  - `test_drum_rack_synthesis_and_choke`: ok
  - `test_autofilter_modulation`: ok
  - `test_chorus_processing`: ok
  - `test_autopan_modulation`: ok
  - `test_gate_dynamics`: ok
  - `test_eq3_processing`: ok
  - `test_redux_quantization`: ok
  - `test_lock_free_param_queue_dispatch`: ok
  - `test_simpler_playback_modes`: ok

## Metrics

Verifiable counts and limits from the current codebase:

| Metric                               | Value                       | Source File or Verification Command     | Notes                                                                                              |
| :----------------------------------- | :-------------------------- | :-------------------------------------- | :------------------------------------------------------------------------------------------------- |
| Automated workspace tests            | 42                          | `cargo test --workspace`                | 0 failures, 0 ignored                                                                              |
| Implemented studio DSP devices       | 16                          | `crates/livecraft-dsp/src/processor.rs` | Operator, Simpler, Drum Rack, EQ Eight, Dynamics, Space, Modulation                                |
| Autonomous implementation progress   | 14 / 28 (50.0%)             | `tasks/state.json`                      | Phases 1 and 2 completed, Phase 3 underway                                                         |
| Offline render speed                 | ~133x faster than real-time | `crates/livecraft-dsp/src/bouncer.rs`   | 2-bar project renders in approximately 15 ms                                                       |
| Supported audio sample rates         | 44.1 kHz to 192 kHz         | `crates/livecraft-dsp/src/engine.rs`    | Native CPAL stream configuration                                                                   |
| Internal summing headroom            | 32-bit floating point       | `crates/livecraft-dsp/src/graph.rs`     | Bit-transparent neutral operations conforming to Chapter 39                                        |
| Audio callback scratch buffer size   | 4096 samples                | `crates/livecraft-dsp/src/engine.rs`    | Fixed array pre-allocated at initialization                                                        |
| Diatonic scale modes supported       | 12 modes                    | `crates/livecraft-core/src/model.rs`    | Major, Minor, Dorian, Phrygian, Lydian, Mixolydian, Locrian, Harmonic, Melodic, Pentatonics, Blues |
| Drum Rack pad matrix                 | 16 pads                     | `crates/livecraft-dsp/src/drumrack.rs`  | MIDI notes 36 to 51 with choke groups                                                              |
| Operator FM algorithms               | 11 routing algorithms       | `crates/livecraft-dsp/src/operator.rs`  | Conforms to Ableton Operator routing topologies                                                    |
| Memory allocations in audio callback | 0 heap allocations          | `crates/livecraft-dsp/src/engine.rs`    | Conforms to Chapter 38 real-time safety                                                            |

## Troubleshooting

| Problem                                    | Likely Cause                             | Diagnostic Command                                      | Resolution                                                            |
| :----------------------------------------- | :--------------------------------------- | :------------------------------------------------------ | :-------------------------------------------------------------------- |
| Audio device initialization fails on Linux | Missing ALSA development headers         | `cargo test -p livecraft-dsp`                           | Install `libasound2-dev` and `libudev-dev` via your package manager   |
| GUI fails to open or panics on launch      | GPU driver lacks OpenGL 3.3 support      | `livecraft gui --screenshot test.png`                   | Update your graphics drivers or run with software rendering           |
| `livecraft info` fails on a `.als` file    | File is not a valid gzip Ableton archive | `file path/to/set.als`                                  | Ensure the file is an authentic Ableton Live set compressed with gzip |
| Offline bounce produces a silent WAV file  | Track is muted or clip slot has no notes | `livecraft inspect path/to/set.als --clips`             | Add MIDI notes to the clip or ensure the track activator is turned on |
| Clippy reports warnings on custom code     | Missing strict typing or unused imports  | `cargo clippy --workspace --all-targets -- -D warnings` | Fix identified lint issues without using `#![allow(warnings)]`        |

## Contributing

We welcome community contributions. Please review [CONTRIBUTING.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/CONTRIBUTING.md) for architectural guidelines, real-time safety rules, and pull request procedures. All pull requests must pass the local CI test suite:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo build --release`

## Licence

LiveCraft is dual-licensed under either:

- Apache License, Version 2.0 ([LICENSE-APACHE](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/LICENSE-APACHE))
- MIT License ([LICENSE-MIT](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/LICENSE-MIT))

You may choose either licence at your option.
