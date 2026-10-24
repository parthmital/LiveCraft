# LiveCraft Technical Architecture & Engineering Manual

A clean-room, native Rust digital audio workstation engineered for high performance, deterministic audio rendering, and complete structural and visual parity with Ableton Live 12.

## 1. Architectural Philosophy and Design Principles

LiveCraft is built from first principles in 100% safe, native Rust without relying on Electron, web views, or legacy C++ DAW codebases. Its architecture strictly adheres to three foundational tenets:

1. **Deterministic Audio Pipeline:** Every calculation in the DSP graph produces mathematically verifiable, bit-transparent results across platforms, conforming to the neutral operations guidelines in Ableton Live 12 Manual Chapter 39.
2. **Lock-Free Real-Time Safety:** The real-time audio thread executing the hardware callback never allocates heap memory (`Vec`, `Box`, `String`), acquires mutual exclusion locks (`Mutex`), or makes blocking operating system calls, conforming to Chapter 38.
3. **Dual-Surface Execution:** Every operation possible within the graphical interface is exposed via the headless command-line interface, enabling automated rendering, set patching, CI audio verification, and terminal playback.

## 2. Workspace Organization and Crate Boundaries

```
LiveCraft/
├── Cargo.toml                  # Workspace manifest
├── docs/                       # Authoritative documentation suite
│   ├── ableton-live-12-reference-manual.pdf  # Definitive 1039-page source of truth
│   ├── ABLETON_12_SPECIFICATION.md          # 43-chapter system specification
│   ├── DEVICES_REFERENCE.md                 # 42 audio FX, 8 MIDI FX, 13 synths catalog
│   ├── MIDI_TOOLS_AND_TUNING.md             # Transformations, Generators, Scala engine
│   ├── KEYBOARD_SHORTCUTS.md                # Comprehensive shortcut mapping
│   └── ARCHITECTURE.md                      # Engineering design manual
└── crates/
    ├── livecraft-core/         # Pure-data document model, .als/.ask parser, commands
    ├── livecraft-dsp/          # Lock-free audio graph, DSP processors, CPAL engine
    ├── livecraft-gui/          # Immediate-mode desktop UI (egui / eframe)
    └── livecraft-cli/          # Command-line entry point & integration test harness
```

### 2.1 `livecraft-core` (Domain Model & Formats)

- **Zero Audio Code:** Contains no DSP, sound generation, or audio drivers. It is a pure data crate optimized for fast serialization, document mutation, and deterministic command histories.
- **Data Model:**
  - `Project`: Root container storing tempo, time signature, transport status, tracks, scenes, return tracks, master track, active scale configuration, and tuning system.
  - `Track`: Audio, MIDI, Return, and Master track definitions with volume, pan, mute, solo, arm, send levels, and device chains.
  - `ClipSlot` & `Clip`: Session matrix slots and clip payloads (MIDI notes or audio sample references).
  - `ArrangementClip`: Timeline-placed clip container with absolute beat start position and beat length.
  - `MidiClipData`: High-performance note store with built-in Live 12 transformations (Quantize, Transpose, Chop, Strum, Humanize, Ornament, Span, Invert, Reverse, Time Warp, Velocity Ramp) and generators (Euclidean, Stacks).
  - `TuningSystem`: Scala `.scl` microtonal tuning definition with frequency mapping.
- **Ableton Format Compatibility:**
  - `als.rs`: Decompresses gzip streams, parses complex Ableton Live 10, 11, and 12 XML hierarchies, and serializes LiveCraft projects back into valid gzip `.als` archives.
  - `theme.rs`: Parses `.ask` theme files to extract precise RGB color palettes.
- **Command History:**
  - `command.rs`: Command pattern undo/redo stack (`CommandHistory`) tracking track additions, note insertions, volume modifications, and device parameter updates.

### 2.2 `livecraft-dsp` (Audio Engine & DSP Rack)

- **Lock-Free Concurrency:**
  - The UI and CLI communicate with the audio engine via atomic values (`AtomicBool`, `AtomicU64`) and lock-free crossbeam ring-buffer queues.
  - No `malloc` or `free` calls occur inside the CPAL `write_audio` stream. Scratch buffers (`[f32; 4096]`) are pre-allocated at initialization.
- **Topological Audio Graph:**
  - `AudioGraph`: Evaluates track processing order using a directed acyclic graph (DAG).
  - Tracks process their device chains in series: MIDI Effects -> Instrument -> Audio Effects.
  - Sends are tapped and routed into Return tracks with latency compensation.
  - Return and audio tracks sum into the Master Track with 32-bit floating point headroom.
- **Built-in Device Implementations:**
  - Synthesizers: `Operator` (4-operator polyphonic FM/subtractive), `Simpler` (sample playback with envelopes and filters), `DrumRack` (16-pad drum machine with choke groups).
  - Dynamics & Shaping: `Compressor`, `Limiter`, `Gate`, `Saturator`, `Redux`.
  - Filters & EQ: `EQ Eight`, `EQ Three`, `Auto Filter`, `BiquadFilter`.
  - Time & Modulation: `Delay`, `Reverb`, `Chorus-Ensemble`, `Auto Pan-Tremolo`, `Utility`.
- **Offline Bouncer:**
  - `bouncer.rs`: Renders audio faster than real-time (~133x) directly to 16, 24, or 32-bit float WAV files using `hound`.
  - Multi-track stem exporter renders isolated tracks to parallel audio files.

### 2.3 `livecraft-gui` (Immediate-Mode Interface)

- **Ableton Live 12 Visual Identity:**
  - Built with `egui` and `eframe` using hardware acceleration via OpenGL (`glow`).
  - Dark Neutral theme with signature Ableton amber (`#ffad56`), record red (`#ff5559`), cue blue (`#3c6ab6`), and 70-color track palette.
- **View Layouts:**
  - **Control Bar:** Tap tempo, drag-tempo adjustment, time signature, play/stop/record, bar.beat.sixteenth counter, CPU load meter.
  - **Session View (`Tab`):** Grid of vertical tracks and horizontal scenes, clip slot indicators, animated playback progress bars, stop buttons.
  - **Mixer View:** Vertical faders (-60 dB to +6 dB), stereo peak VU meters, pan knobs, send knobs, Mute, Solo, and Arm buttons.
  - **Arrangement View (`Tab`):** Multi-track horizontal timeline, bar ruler, clip arrangement blocks, moving playhead line.
  - **Clip Detail / Piano Roll:** 128-note grid, scale highlighting, Fold to Scale toggle, note creation, velocity handles, MIDI transformation toolbars.
  - **Modular Device Chain:** Horizontal device racks with interactive parameter knobs, EQ Eight 8-band curve display, and Saturator transfer curve.
  - **Browser Sidebar:** Category browser with instant search, device insertion, and sample previewing.

### 2.4 `livecraft-cli` (Headless Automation)

- Exposes full DAW capabilities for scripts, build pipelines, and terminal users:
  - `livecraft info <set.als>`: Metadata, tempo, track count, scene count.
  - `livecraft inspect <set.als>`: Deep inspection of devices, clips, parameters, JSON output.
  - `livecraft new <project.als>`: Initializes template sets.
  - `livecraft render <set.als> -o out.wav`: Headless offline audio bounce.
  - `livecraft stems <set.als> -o ./stems`: Multi-track stem export.
  - `livecraft play <set.als>`: Terminal audio playback with ASCII stereo VU meters.
  - `livecraft synth`: Waveform rendering from terminal.
  - `livecraft convert`: Bidirectional `.als` <-> JSON conversion.
  - `livecraft gui`: Launches desktop application.

## 3. Strict Conformance to Audio and MIDI Fact Sheets

- **Summing Integrity (Chapter 39):**
  - Audio signals are summed in 32-bit floating point arithmetic without digital clipping before the master output stage.
  - When gain is set to 0.0 dB and no processing is applied, the output audio is identical to the input waveform bit-for-bit (null-test verified).
- **Sample-Accurate MIDI Timing (Chapter 40):**
  - Note triggers are calculated using sub-block sample offsets rather than sample-by-sample vector iterations.
  - Eliminates jitter and preserves micro-timing across multi-channel tracks.
