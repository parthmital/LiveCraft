# LiveCraft Architecture & Research Dossier

**Tier: Full** (New creative workstation application, native Rust stack, real-time DSP, file reverse engineering, CLI and GUI).

## 1. Executive Summary & Goals

LiveCraft is a clean-room, native Rust recreation of Ableton Live, inspired by the architecture of `storytold` projects (`PhotoCraft`, `FilmCraft`).
It provides:

1. **Headless & Scriptable First:** Full command-line interface (CLI) for parsing, inspecting, rendering, stem exporting, and playback of Ableton projects.
2. **Deterministic Document & Command Architecture:** Pure-data document model with stable command IDs, full undo/redo history, and zero-allocation real-time audio thread communication.
3. **Ableton Compatibility:** Native `.als` parser and generator (gzipped XML) supporting Live 10, 11, and 12 sets, plus Live 12 theme (`.ask`) parsing.
4. **Authentic Ableton Live UI:** Custom immediate-mode interface (`egui`/`eframe`) implementing Session View (clip launcher matrix), Arrangement View (timeline), mixer, piano roll, and bottom device chain.
5. **Real-time & Offline DSP:** Complete suite of studio devices conforming to Ableton Live 12 Manual specifications.

## 2. Hard Constraints and Manual Grounding

- **Language:** Pure Rust 2021 edition, 100% native.
- **Platform:** Windows primary (WASAPI/DirectSound via CPAL), portable to macOS (CoreAudio) and Linux (ALSA/JACK).
- **Audio Thread Safety (Manual Chapter 38):** The audio callback must never allocate, block on a mutex, or perform disk/network I/O. Inter-thread communication uses atomic variables or lock-free ring buffers.
- **Neutral DSP Operations (Manual Chapter 39):** Neutral operations (gain at 0.0 dB, bypassed devices, undithered 32-bit float rendering) must pass audio bit-for-bit without distortion or coloration.
- **Sample-Accurate MIDI Timing (Manual Chapter 40):** Note events are scheduled at exact sample offsets within audio buffer blocks rather than iterating over note vectors per sample.
- **File Compatibility:** Direct parsing of real Ableton Live `.als` files decompressed via gzip.
- **Offline Rendering:** High-speed, sample-accurate, non-realtime rendering to 24-bit/32-bit float WAV files (~133x faster than real-time).

## 3. Technology Evaluation & Selection

### 3.1 Audio I/O & Offline Audio

- **Selected: `cpal` + `hound`**
  - CPAL: Industry standard Rust low-latency audio device abstraction. Supports WASAPI, ASIO, CoreAudio, ALSA, JACK. Actively maintained by RustAudio.
  - Hound: Pure Rust WAV encoder and decoder. Zero unsafe dependencies, fast, deterministic.
  - Decision: Adopted. CPAL for real-time output stream, Hound for sample loading and offline bouncing.

### 3.2 Graphical User Interface (GUI)

- **Selected: `egui` & `eframe`**
  - Immediate-mode, extremely fast rendering via `glow`.
  - Zero state-synchronization lag with audio engine parameters.
  - Used by `storytold` in `PhotoCraft` and `FilmCraft`.
  - Highly customizable styling, allowing exact replication of Ableton Live 12 flat, high-contrast, compact look.
  - Decision: Adopted.

### 3.3 File Format & Theme Parsing

- **Selected: `flate2` + `quick-xml`**
  - `flate2` (`miniz_oxide` backend): Pure Rust gzip decompression and compression.
  - `quick-xml`: Zero-copy, high throughput streaming XML parser. Parses multi-megabyte Ableton XML projects in milliseconds.
  - Decision: Adopted.

### 3.4 CLI Framework

- **Selected: `clap` (derive)**
  - De facto standard for Rust CLIs. Clean subcommand architecture (`info`, `inspect`, `render`, `stems`, `play`, `synth`, `convert`, `gui`).
  - Decision: Adopted.

## 4. Architectural Blueprint

```
livecraft/
├── docs/                   # Authoritative manual suite
│   ├── ableton-live-12-reference-manual.pdf
│   ├── ABLETON_12_SPECIFICATION.md
│   ├── DEVICES_REFERENCE.md
│   ├── MIDI_TOOLS_AND_TUNING.md
│   ├── KEYBOARD_SHORTCUTS.md
│   └── ARCHITECTURE.md
├── crates/
│   ├── livecraft-core/     # Pure data document model, ALS/ASK parser, command system
│   │   ├── src/
│   │   │   ├── model.rs    # Project, Track, Clip, Scale, Tuning, Device
│   │   │   ├── als.rs      # Ableton .als XML deserializer & serializer
│   │   │   ├── theme.rs    # .ask Theme parser & palette mapping
│   │   │   ├── command.rs  # Stable Command enum, Undo/Redo stack
│   │   │   └── lib.rs
│   ├── livecraft-dsp/      # Audio graph, DSP units, synth, audio engine
│   │   ├── src/
│   │   │   ├── engine.rs   # Real-time CPAL stream & lock-free audio thread
│   │   │   ├── graph.rs    # Multi-track audio rendering pipeline
│   │   │   ├── bouncer.rs  # Offline faster-than-realtime WAV & stem exporter
│   │   │   ├── autopan.rs  # Auto Pan-Tremolo LFO stereo panner
│   │   │   ├── gate.rs     # Noise gate with hysteresis and hold
│   │   │   ├── eq3.rs      # 3-band DJ style isolator equalizer
│   │   │   ├── redux.rs    # Sample rate & bitcrusher effect
│   │   │   ├── eq8.rs      # 8-band parametric biquad equalizer
│   │   │   ├── compressor.rs
│   │   │   ├── saturator.rs
│   │   │   ├── autofilter.rs
│   │   │   ├── chorus.rs
│   │   │   ├── delay.rs
│   │   │   ├── reverb.rs
│   │   │   ├── limiter.rs
│   │   │   ├── operator.rs # 4-Osc FM & subtractive synthesizer
│   │   │   ├── drumrack.rs # 16-pad drum machine with choke groups
│   │   │   ├── simpler.rs  # Sample player
│   │   │   └── lib.rs
│   ├── livecraft-gui/      # Ableton Live 12 GUI recreation with egui/eframe
│   │   ├── src/
│   │   │   ├── app.rs      # Main eframe application loop & shortcuts
│   │   │   ├── transport.rs# Top transport bar (Play, Stop, BPM, Metronome, CPU)
│   │   │   ├── session.rs  # Session View (clip grid, launch buttons, scenes)
│   │   │   ├── arrange.rs  # Arrangement View (timeline tracks, clips, ruler)
│   │   │   ├── mixer.rs    # Track strips, volume faders, VU meters, pan, mute/solo
│   │   │   ├── devices.rs  # Bottom device chain rack & parameter controls
│   │   │   ├── pianoroll.rs# Clip detail, scale fold, & MIDI transformation tools
│   │   │   ├── browser.rs  # Left file & library browser
│   │   │   └── style.rs    # Ableton Live 12 theme & widget styling
│   └── livecraft-cli/      # Headless CLI entry point & subcommands
│       └── src/
│           └── main.rs     # CLI subcommands: info, inspect, render, stems, play, gui
```

## 5. Verification Strategy

1. **Unit & Integration Tests:**
   - Test `.als` decompression and extraction against official Ableton Live 12 template sets.
   - Test DSP filter responses, oscillator waveforms, and mixer summing without clipping.
   - Test offline rendering output byte-for-byte in WAV format.
   - Test MIDI Transformations and Scala `.scl` tuning calculations.
2. **CLI Verification:**
   - Run `livecraft info` on Ableton templates.
   - Run `livecraft render` to bounce audio to WAV.
   - Inspect output WAV headers and audio samples.
3. **GUI Verification:**
   - Build native Windows executable and verify UI rendering, transport control, and real-time audio playback.
