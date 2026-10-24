# LiveCraft Master Roadmap: Production-Grade Ableton Live Recreation

## 1. Executive Overview and Strategic Vision

LiveCraft is an open-source, native Rust digital audio workstation (DAW) engineered for high performance, deterministic audio rendering, and deep interoperability with Ableton Live sets (`.als`).

The strategic target is divided into two continuous tiers:
1. Baseline: 1:1 functional, structural, and visual parity with Ableton Live 12.1+ (Session View, Arrangement View, audio engine, DSP rack, MIDI sequencing, and file format round-trip).
2. Superiority: Exceeding Ableton Live in areas where conventional DAWs struggle:
   - Full headless operation and terminal-driven automation (batch stem exports, headless set patching, CI audio testing).
   - Zero-telemetry, ultra-low resource footprint with immediate startup (< 150 ms).
   - Lock-free, allocation-free pure-Rust audio engine with native memory safety.
   - Built-in collaborative version control friendly formats (bidirectional clean JSON / Git-friendly `.als` diffing).
   - Built-in modern scale and microtonal tuning systems with zero third-party plugin requirements.

## 2. Audit of Existing Codebase and Gap Analysis

### 2.1 Current State Audit

| Subsystem | Implemented Components | Audit Verdict |
| :--- | :--- | :--- |
| `livecraft-core` | Project, Track, Clip, ClipSlot, Scene, Transport, Device models; `.als` gzip decompression and streaming XML parser; `.ask` theme parser; Command history undo/redo stack. | Solid architectural foundation. Gaps: Arrangement clips need explicit timeline placement (`start_beat`, `length_beats`) decoupled from Session slots; scales and keys are unmodelled; automation envelopes not yet stored. |
| `livecraft-dsp` | CPAL audio engine; AudioGraph; EQ Eight, Delay, Reverb, Compressor, Limiter, Utility; Operator synth (4 waveforms, ADSR, resonant filter); Simpler playback; Offline faster-than-realtime WAV bounce and stem export. | Functional audio pipeline. Critical gap: heap allocation occurs in the real-time audio callback (`vec![]` in `write_audio`), and per-sample clip iteration causes unneeded overhead. Needs pre-allocated scratch buffers and block-rate event dispatch. |
| `livecraft-gui` | Ableton Live 12 styled immediate-mode UI (`egui`/`eframe`); Top transport bar; Session View clip matrix with launch triggers; Mixer strips with volume faders and VU meters; Piano Roll note editor; Bottom modular device chain; Left library browser. | Visually faithful Ableton Live 12 layout. Gaps: Simultaneous stacked views (e.g. Mixer in Arrangement View); Scale and Key highlighting; MIDI transformations (strum, humanize, quantize); Arrangement timeline clip dragging and resizing. |
| `livecraft-cli` | Subcommands: `new`, `info`, `inspect`, `render`, `stems`, `play`, `synth`, `convert`, `gui`. | Rich headless command-line interface. Fully operational for rendering, inspect, and terminal playback. |
| Infrastructure | GitHub CI workflow (fmt, clippy, test, release build for Windows and Ubuntu). | Passing clean. Missing: open-source community standards (LICENSE, CONTRIBUTING, issue templates), audio regression benchmarks, and automated visual comparison tests. |

## 3. Comprehensive Feature and Workflow Roadmap

### Phase 0: Audio Engine Real-Time Hardening and Core Data Integrity (P0)

Focus: Lock-free concurrency, zero allocations on the audio thread, and robust document modeling.

- [x] Fix audio callback allocations: Replace all heap allocations in CPAL `write_audio` with pre-allocated ring buffers and static scratch arrays.
- [x] Decouple Arrangement View timeline clips from Session View clip slots in `livecraft-core`.
- [x] Integrate global Key and Scale systems into `Project` and `Clip` (Major, Minor, Dorian, Phrygian, Lydian, Mixolydian, Locrian, Harmonic Minor, Pentatonic, Blues).
- [x] Implement sample-accurate event scheduling: Dispatch MIDI note-on/note-off events using buffer-block offsets rather than iterating over note vectors per sample.
- [x] Add parameter automation storage in `livecraft-core` (track volume, pan, send levels, and device parameters over musical time).
- [ ] Implement multi-threaded track processing pool with work-stealing for multi-core DSP execution.
- [ ] Add dynamic buffer size switching and sample rate conversion (32000, 44100, 48000, 88200, 96000, 192000 Hz).
- [ ] Add true ASIO driver support on Windows alongside WASAPI.

### Phase 1: Ableton Live 12 Interface Parity and Workflow Polish (P1)

Focus: Visual elegance, fluid interactive controls, and Ableton 12 design paradigms.

- [x] Implement Piano Roll Scale Mode: Key and root selector, in-scale note highlighting, and "Fold to Scale" toggle.
- [x] Implement essential MIDI Transformations in the Piano Roll:
  - Quantize: Snap note start times and lengths to selected grid divisions (1/4, 1/8, 1/16, 1/32).
  - Transpose: Semitone and octave shift up/down.
  - Humanize: Velocity and micro-timing randomization with adjustable depth.
  - Strum: Successive micro-delays across chord notes with direction control.
  - Invert and Reverse: Pitch inversion and time reversal across selected notes.
- [x] Add Saturation and Distortion effect (`Saturator` / `Drive` device) matching Ableton's drive modes (Soft Sine, Hard Clip, Analog Warmth).
- [ ] Implement Stacked View toggle: Enable simultaneous viewing of Clip Detail and Device Chain, as well as Arrangement Mixer.
- [ ] Interactive Timeline Clip Editing in Arrangement View: Clip dragging, edge resizing, duplicating (`Ctrl+D`), split at playhead (`Ctrl+E`), and clip consolidation (`Ctrl+J`).
- [ ] Follow Actions in Session View: Action triggers (Next, Previous, First, Last, Random) on clip completion with loop counts.
- [ ] Drag-and-Drop sample loading from file system and library browser directly onto Audio Tracks, Simpler, and Drum Rack pads.

### Phase 2: Expanded DSP Device Suite and Instruments (P2)

Focus: Studio-grade sound generation and processing matching Ableton Live Suite.

- [ ] Drum Rack: 16-pad sample grid with independent Simpler instances, choke groups, send levels, and pad solo/mute.
- [ ] Wavetable Synth: Dual wavetable oscillators, sub-oscillator, filter morphing, and modulation matrix.
- [ ] Auto Filter: Resonant multi-mode filter (Lowpass, Highpass, Bandpass, Notch) with LFO modulation and envelope follower.
- [ ] Chorus-Ensemble: Multi-voice delay line modulation with vibrato and ensemble warmth.
- [ ] Glue Compressor: Analog-modeled VCA bus compressor with sidechain high-pass filter.
- [ ] Pitch-shifting and Time-stretching (Warp Engine): Granular time-stretch algorithm for Beats, Tones, and Texture modes.
- [ ] Melodic Stem Separation CLI and GUI tool: Built-in local offline stem separation for drums, bass, vocals, and instruments.

### Phase 3: Plugin Hosting and Interoperability (P3)

Focus: Ecosystem expansion and open standards.

- [ ] CLAP (Clever Audio Plugin) host implementation: Pure Rust hosting of CLAP instruments and audio effects.
- [ ] VST3 plugin host implementation on Windows, macOS, and Linux.
- [ ] MIDI Hardware Control Surface integration (Push 2/3, Novation Launchpad, generic MIDI Learn).
- [ ] Complete `.als` round-trip fidelity validation against Ableton Live 10, 11, and 12 sets without data loss.
- [ ] Sound library and preset system with tag filtering, category search, and fast audio preview.

## 4. Acceptance Criteria and Verification Regimes

Every milestone and pull request must meet the following automated criteria:

1. Zero Audio Callback Allocations: Audio processing callbacks must execute without calling heap allocators (`malloc`, `free`, `Vec::new()`, `String::new()`).
2. Continuous Formatting and Linting: `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` must pass with zero warnings.
3. Test Coverage: All unit tests in `livecraft-core`, `livecraft-dsp`, and `livecraft-cli` must pass without failures.
4. Deterministic Offline Bouncing: Bounced audio waveforms from identical projects must remain sample-accurate across repeated runs.
5. Cross-Platform Builds: Release binaries compile successfully across Windows (x86_64) and Linux (x86_64).

## 5. Living Progress and Task Tracking

- [x] Core data model for Projects, Tracks, ClipSlots, Clips, Scenes, and Devices.
- [x] Ableton Live Set (`.als`) decompression, XML streaming parsing, and export.
- [x] Ableton Live 12 color theme parsing (`.ask`) and GUI styling.
- [x] Core DSP devices: Operator Synth, Simpler, EQ Eight, Delay, Reverb, Compressor, Limiter, Utility.
- [x] Offline audio rendering and multi-track stem exporter to WAV.
- [x] Full headless CLI suite (`new`, `info`, `inspect`, `render`, `stems`, `play`, `synth`, `convert`, `gui`).
- [x] Elimination of heap allocations in real-time audio callback buffers.
- [x] Key and Scale model with Major, Minor, Modes, Pentatonic, and Blues scales.
- [x] Piano Roll Scale highlighting and "Fold to Scale" functionality.
- [x] MIDI transformation tools: Quantize, Transpose, Strum, Humanize, Invert, Reverse.
- [x] Saturator / Drive audio effect device for harmonic warmth and clip shaping.
- [x] Decoupled Arrangement timeline clips with absolute beat timing.
- [x] Open-source community readiness: Dual License (MIT / Apache-2.0), CONTRIBUTING.md, Issue and PR templates.
- [ ] Multi-clip simultaneous editing in Piano Roll.
- [ ] Full Drum Rack instrument implementation.
- [ ] CLAP plugin hosting support.
