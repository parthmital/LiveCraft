# LiveCraft

> A clean-room, native Rust implementation of Ableton Live. Inspired by `storytold` projects (`PhotoCraft`, `FilmCraft`).

LiveCraft is an open-source, high-performance Digital Audio Workstation (DAW) written from the ground up in pure Rust. It combines a real-time, low-latency audio engine, offline bouncing, Ableton Live set (`.als`) compatibility, headless CLI workflows, and a native desktop interface styled after Ableton Live 12.

## Authoritative Ableton Live 12 Reference Suite

The entire specification, behavior, and design of LiveCraft is grounded directly in the official Ableton Live 12 Reference Manual (`docs/ableton-live-12-reference-manual.pdf`, 108 MB, 1039 pages, 43 chapters):

- [Ableton Live 12 Complete System Specification](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/ABLETON_12_SPECIFICATION.md): 43-chapter system mapping, UI views, routing, warping, and audio fact sheet neutral operations.
- [Complete Devices Reference Guide](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/DEVICES_REFERENCE.md): Full technical catalog of all 42 Audio Effects, 8 MIDI Effects, and 13 Instruments from Chapters 29, 30, and 31.
- [MIDI Tools, Scales & Microtonal Tuning](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/MIDI_TOOLS_AND_TUNING.md): Transformations (Chop, Strum, Quantize, Ornament, Span), Generative Tools (Stacks, Euclidean), Scale systems, and Scala `.scl` engine.
- [Complete Keyboard Shortcuts Matrix](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/KEYBOARD_SHORTCUTS.md): Exhaustive shortcut matrix across Windows and macOS from Chapter 42.
- [LiveCraft Technical Architecture](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/ARCHITECTURE.md): Multi-crate modular architecture, lock-free audio thread constraints, and topological DSP graph.

## Key Capabilities

- **100% Native Pure Rust Stack:** Audio engine, DSP graph, format parser, CLI, and GUI written entirely in Rust with zero Electron or web view bloat.
- **Ableton Live Set (`.als`) Compatibility:** Direct decompression and streaming parsing of gzip-compressed Ableton Live XML files (Live 10, 11, and 12). Can load real projects (including 27+ MB complex sets), extract tracks, scenes, clips, notes, and devices, and export projects back to valid `.als` files.
- **Ableton Live 12 Theme Engine:** Parses `.ask` theme files (e.g. `Default Dark Neutral Medium.ask`) to render exact Ableton color palettes, including the signature amber toggles (`#ffad56`), record red (`#ff5559`), cue blue (`#3c6ab6`), and meter gradients.
- **Dual Control Surfaces (CLI & GUI):** Everything executable from the desktop UI is available from the headless CLI, enabling automated audio rendering, stem generation, inspection, format conversion, and terminal playback.
- **Built-in DSP Devices & Instruments:**
  - **Operator Synth:** Polyphonic 4-oscillator subtractive/FM synthesizer (Sine, Saw, Square, Triangle, Noise) with ADSR envelopes and resonant 24dB filters.
  - **Drum Rack:** 16-pad sample & synth drum machine with pad solo/mute, levels, and choke group cutoffs (e.g. open/closed hi-hats).
  - **Simpler:** Polyphonic sample player with pitch tracking, looping, and filter envelopes.
  - **Auto Pan-Tremolo:** LFO modulation of stereo position or amplitude with Sine and Triangle waveforms, stereo phase offset, and Panning/Tremolo modes.
  - **Gate:** Noise gate with hysteresis (Return), lookahead, attack, hold, release, and attenuation floor controls.
  - **EQ Three:** 3-band DJ isolator equalizer with Low/Mid/High band gains (-inf to +6 dB), crossover filters, and band kill switches.
  - **Redux:** Digital downsampling (sample rate reduction) and bit reduction (quantization distortion) with jitter and dry/wet blending.
  - **Saturator:** Multi-curve waveshaping and harmonic saturation effect (Soft Sine, Hard Clip, Analog Warmth, Foldback) with interactive transfer-function curve.
  - **Auto Filter:** Resonant multi-mode filter (Lowpass, Highpass, Bandpass, Notch) with LFO modulation and dynamic envelope follower.
  - **Chorus-Ensemble:** Modulated stereo delay ensemble with quadrature phase widening and shimmer feedback.
  - **EQ Eight:** 8-band parametric biquad equalizer matching Ableton EQ Eight.
  - **Compressor:** Dynamic range compressor with threshold, ratio, attack, release, makeup gain, and gain-reduction metering.
  - **Delay:** Stereo ping-pong and tempo-synced delay line with feedback filtering.
  - **Reverb:** Algorithmic stereo Schroeder/Freeverb room reverb with damping, room size, and stereo width.
  - **Limiter:** Transparent lookahead brickwall peak limiter with ceiling control.
  - **Utility:** Gain (-inf to +35 dB), stereo width (0% mono to 400% wide), pan, and phase inversion.
- **Ableton Live 12 Scale System & MIDI Tools:**
  - **Keys and Scales Engine:** Full diatonic and modern scale integration across 12 roots and 12 modes (Major, Minor, Harmonic Minor, Melodic Minor, Dorian, Phrygian, Lydian, Mixolydian, Locrian, Pentatonics, Blues).
  - **Fold to Scale:** Piano roll filters non-scale rows on demand, with visual scale highlights and root markers.
  - **MIDI Transformations:** Algorithmic clip transforms directly from UI or CLI (Quantize, Transpose, Chop, Strum, Humanize, Ornament, Span, Invert, Reverse, Time Warp, Velocity Ramp).
  - **Generative Tools:** Diatonic chord Stacks and multi-voice Euclidean rhythm generation based on the Bjorklund algorithm.
  - **Microtonal Tuning Systems:** Native Scala `.scl` file format support for non-standard pitch distributions and cents calibrations.
- **Real-Time & Faster-Than-Realtime Audio:**
  - Allocation-free, lock-free real-time stream using `cpal` (WASAPI on Windows, CoreAudio on macOS, ALSA/JACK on Linux).
  - High-speed offline bounce to 16, 24, or 32-bit float WAV using `hound` (~133x faster than real-time).
  - Stem export: one command bounces each track to an isolated stem file.

## Workspace Architecture

The workspace is organized into four decoupled crates:

```
livecraft/
├── .github/
│   └── workflows/
│       └── ci.yml          # Multi-platform CI pipeline (fmt, clippy, test, release build)
├── docs/                   # Authoritative documentation suite
│   ├── ableton-live-12-reference-manual.pdf
│   ├── ABLETON_12_SPECIFICATION.md
│   ├── DEVICES_REFERENCE.md
│   ├── MIDI_TOOLS_AND_TUNING.md
│   ├── KEYBOARD_SHORTCUTS.md
│   └── ARCHITECTURE.md
├── crates/
│   ├── livecraft-core/     # Pure-data document model, .als/.ask parser, command pattern
│   ├── livecraft-dsp/      # Lock-free audio graph, DSP effects, synths, CPAL engine, bouncer
│   ├── livecraft-gui/      # Ableton Live 12 desktop GUI (egui / eframe / glow)
│   └── livecraft-cli/      # Headless CLI entry point & integration tests
├── ROADMAP.md              # Living master roadmap (parity matrix, tasks, verification)
├── RESEARCH.md             # Research-driven development architecture dossier
├── CONTRIBUTING.md         # Open-source developer & contributor guidelines
└── Cargo.toml              # Cargo workspace configuration
```

## Installation & Build

### Prerequisites

- Rust 1.80+ (Rust 2021 edition)
- Windows 10/11, macOS, or Linux

### Building from Source

```bash
# Clone the repository
git clone https://github.com/storytold/livecraft.git
cd livecraft

# Run test suite
cargo test --workspace

# Build optimized release binary
cargo build --release

# The compiled executable is available at:
# target/release/livecraft.exe (Windows) or target/release/livecraft (macOS/Linux)
```

## CLI Usage

LiveCraft is fully operable from the terminal:

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

Renders an Ableton set to a WAV audio file faster than real-time:

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
# Or open directly with a project
livecraft gui path/to/project.als
```

## Graphical User Interface (GUI)

The desktop application recreates Ableton Live 12's signature interface:

- **Top Control Bar:** Tap tempo, drag-to-adjust BPM, 4/4 time signature, metronome toggle, Play/Stop/Record buttons, Bar.Beat.Sixteenth counter, View mode toggle, CPU load meter.
- **Session View (`Tab` or `[|||]`):** Track headers with color tags, vertical clip slot matrix, launch arrows, real-time animated loop progress bars, track stop buttons, Master Scene launch column, and "Stop All Clips".
- **Mixer View:** Vertical volume faders (-60 dB to +6 dB), stereo peak VU meters (green/yellow/red), pan knobs, Send A (Reverb) and Send B (Delay) knobs, Track Activator (mute), Solo (`S`), and Arm (`●`) buttons.
- **Arrangement View (`Tab` or `[=]`):** Horizontal timeline with bar/beat time ruler, track lanes with color-coded clips and note previews, and real-time moving playhead line.
- **Bottom Detail Panel:**
  - **Device Chain:** Horizontal modular rack showing selected track's devices (EQ Eight, Auto Filter, Auto Pan-Tremolo, Gate, EQ Three, Redux, Saturator, Compressor, Delay, Reverb, Operator, Utility).
  - **Clip Detail / Piano Roll:** 128-note keyboard grid, click to insert/remove notes, 16th-note snap grid, Fold to Scale toggle, moving playhead indicator, and MIDI toolbars (Chop, Ornament, Span, Euclid, Stacks, Quantize, Transpose, Strum, Humanize, Invert, Reverse, Velocity Ramp).
- **Left Browser Sidebar:** Searchable categories (Instruments, Audio Effects, Samples, Places) with one-click device insertion and track creation (`+ MIDI`, `+ Audio`).

### Keyboard Shortcuts (Ableton Live 12 Chapter 42)

- `Space`: Play / Stop playback
- `Tab`: Switch between Session View and Arrangement View
- `Shift + Tab` or `F12`: Toggle between Device View and Clip View
- `Ctrl + T`: Insert Audio Track
- `Ctrl + Shift + T`: Insert MIDI Track
- `0`: Toggle Mute / Track Activator on selected track
- `S`: Toggle Solo on selected track
- `C`: Toggle Arm on selected track
- `Ctrl + Z`: Undo
- `Ctrl + Y`: Redo
- `Ctrl + S`: Save project
