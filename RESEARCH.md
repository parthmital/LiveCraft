# LiveCraft Architecture & Research Dossier

**Tier: Full** (New creative workstation application, native Rust stack, real-time DSP, file reverse engineering, CLI and GUI).

## 1. Executive Summary & Goals

LiveCraft is a clean-room, native Rust recreation of Ableton Live, inspired by the architecture of `storytold` projects (`PhotoCraft`, `FilmCraft`).
It provides:

1. **Headless & Scriptable First:** Full command-line interface (CLI) for parsing, inspecting, rendering, stem exporting, and playback of Ableton projects.
2. **Deterministic Document & Command Architecture:** Pure-data document model with stable command IDs, full undo/redo history, and zero-allocation real-time audio thread communication.
3. **Ableton Compatibility:** Native `.als` parser and generator (gzipped XML) supporting Live 10, 11, and 12 sets, plus Live 12 theme (`.ask`) parsing.
4. **Authentic Ableton Live UI:** Custom immediate-mode interface (`egui`/`eframe`) implementing Session View (clip launcher matrix), Arrangement View (timeline), mixer, piano roll, and bottom device chain.
5. **Real-time & Offline DSP:** Parametric EQ Eight, Delay, Algorithmic Reverb, Compressor, Peak Limiter, Utility, Synthesizer (Operator/Analog multi-oscillator subtractive/FM), and Simpler (sample playback).

## 2. Hard Constraints

- **Language:** Pure Rust 2021/2024 edition, 100% native.
- **Platform:** Windows primary (WASAPI/DirectSound via CPAL), portable to macOS (CoreAudio) and Linux (ALSA/JACK).
- **Audio Thread Safety:** The audio callback must never allocate, block on a mutex, or perform disk/network I/O. Inter-thread communication must use atomic variables or lock-free ring buffers.
- **File Compatibility:** Direct parsing of real Ableton Live `.als` files decompressed via gzip.
- **Offline Rendering:** High-speed, sample-accurate, non-realtime rendering to 24-bit/32-bit float WAV files.

## 3. Technology Evaluation & Selection

### 3.1 Audio I/O & Offline Audio

- **Candidate A: `cpal` + `hound`**
  - _CPAL:_ Industry standard Rust low-latency audio device abstraction. Supports WASAPI, ASIO, CoreAudio, ALSA, JACK. Actively maintained by RustAudio.
  - _Hound:_ Pure Rust WAV encoder and decoder. Zero unsafe dependencies, fast, deterministic.
  - _Decision:_ **ADOPT**. CPAL for real-time output stream, Hound for sample loading and offline bouncing.
- **Candidate B: `rodio`**
  - High-level playback library built on CPAL. Lacks fine-grained audio graph control, per-sample scheduling, and custom buffer management needed for a DAW.
  - _Decision:_ Reject.

### 3.2 Graphical User Interface (GUI)

- **Candidate A: `egui` & `eframe`**
  - Immediate-mode, extremely fast rendering via `wgpu`/`glow`.
  - Zero state-synchronization lag with audio engine parameters.
  - Used by `storytold` in `PhotoCraft` and `FilmCraft`.
  - Highly customizable styling, allowing exact replication of Ableton Live 12's flat, high-contrast, compact look.
  - _Decision:_ **ADOPT**.
- **Candidate B: `iced`**
  - Retained/Elm-style. More boilerplate for dynamic multi-track graphs and rapid parameter tweaking; heavier layout engine.
  - _Decision:_ Reject.
- **Candidate C: `Tauri` / Web Technologies**
  - Violates pure-native Rust requirement, introduces webview IPC latency and memory overhead.
  - _Decision:_ Reject.

### 3.3 File Format & Theme Parsing

- **Candidate A: `flate2` + `quick-xml`**
  - `flate2` (`miniz_oxide` backend): Pure Rust gzip decompression and compression.
  - `quick-xml`: Zero-copy, high throughput streaming XML parser. Ableton sets can be tens of megabytes of XML; `quick-xml` parses them in milliseconds.
  - _Decision:_ **ADOPT**.

### 3.4 CLI Framework

- **Candidate A: `clap` (derive)**
  - De facto standard for Rust CLIs. Clean subcommand architecture (`info`, `inspect`, `render`, `stems`, `play`, `synth`, `gui`).
  - _Decision:_ **ADOPT**.

## 4. Architectural Blueprint

```
livecraft/
├── crates/
│   ├── livecraft-core/     # Pure data document model, ALS/ASK parser, command system
│   │   ├── src/
│   │   │   ├── model.rs    # Project, Track, Clip, Scene, Mixer, Transport
│   │   │   ├── als.rs      # Ableton .als XML deserializer & serializer
│   │   │   ├── theme.rs    # .ask Theme parser & palette mapping
│   │   │   ├── command.rs  # Stable Command enum, Undo/Redo stack
│   │   │   └── lib.rs
│   ├── livecraft-dsp/      # Audio graph, DSP units, synth, audio engine
│   │   ├── src/
│   │   │   ├── engine.rs   # Real-time CPAL stream & lock-free audio thread
│   │   │   ├── graph.rs    # Multi-track audio rendering pipeline
│   │   │   ├── bouncer.rs  # Offline faster-than-realtime WAV & stem exporter
│   │   │   ├── effects/    # EQ Eight, Delay, Reverb, Compressor, Limiter, Utility
│   │   │   ├── instruments/# AnalogSynth (Operator), Simpler (Sampler)
│   │   │   └── lib.rs
│   ├── livecraft-gui/      # Ableton Live 12 GUI recreation with egui/eframe
│   │   ├── src/
│   │   │   ├── app.rs      # Main eframe application loop
│   │   │   ├── transport.rs# Top transport bar (Play, Stop, BPM, Metronome, CPU)
│   │   │   ├── session.rs  # Session View (clip grid, launch buttons, scenes)
│   │   │   ├── arrange.rs  # Arrangement View (timeline tracks, clips, ruler)
│   │   │   ├── mixer.rs    # Track strips, volume faders, VU meters, pan, mute/solo
│   │   │   ├── devices.rs  # Bottom device chain rack & parameter controls
│   │   │   ├── pianoroll.rs# Clip detail & MIDI piano roll note editor
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
2. **CLI Verification:**
   - Run `livecraft info` on Ableton templates.
   - Run `livecraft render` to bounce audio to WAV.
   - Inspect output WAV headers and audio samples.
3. **GUI Verification:**
   - Build native Windows executable and verify UI rendering, transport control, and real-time audio playback.
