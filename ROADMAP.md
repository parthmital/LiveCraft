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

| Subsystem        | Implemented Components                                                                                                                                                                                                                                                                                                                                                                                | Audit Verdict                                                                                                                   |
| :--------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------ |
| `livecraft-core` | Project, Track, Clip, ClipSlot, Scene, Transport, Device models; `.als` gzip decompression and streaming XML parser; `.ask` theme parser; Command history undo/redo stack; ScaleConfig with 12 modes; Scala `.scl` tuning system; MIDI Transformations (Quantize, Transpose, Chop, Strum, Humanize, Ornament, Span, Invert, Reverse, Time Warp, Velocity Ramp); Generative Tools (Stacks, Euclidean). | Complete architectural foundation aligned with Ableton Live 12 Manual Chapters 3, 5, 8, 10, 11, and 15.                         |
| `livecraft-dsp`  | CPAL audio engine; lock-free AudioGraph; EQ Eight, Delay, Reverb, Compressor, Limiter, Utility, Operator synth, Simpler, Drum Rack, Saturator, Auto Filter, Chorus-Ensemble, Auto Pan-Tremolo, Gate, EQ Three, Redux; Offline faster-than-realtime WAV bounce and stem export.                                                                                                                        | High-performance allocation-free audio pipeline with 16 studio-grade DSP devices. Conforms to neutral operations in Chapter 39. |
| `livecraft-gui`  | Ableton Live 12 styled immediate-mode UI (`egui`/`eframe`/`glow`); Top transport bar; Session View clip matrix with launch triggers; Mixer strips with volume faders and VU meters; Piano Roll note editor with Scale and Fold mode, MIDI tools; Bottom modular device chain with interactive curves; Left library browser; Full Chapter 42 keyboard shortcut mapping.                                | Visually faithful Ableton Live 12 layout with real-time audio engine synchronization.                                           |
| `livecraft-cli`  | Subcommands: `new`, `info`, `inspect`, `render`, `stems`, `play`, `synth`, `convert`, `gui`.                                                                                                                                                                                                                                                                                                          | Rich headless command-line interface. Fully operational for rendering, inspect, and terminal playback.                          |
| Infrastructure   | GitHub CI workflow (fmt, clippy, test, release build for Windows and Ubuntu).                                                                                                                                                                                                                                                                                                                         | Passing clean with 40 unit and integration tests, zero clippy warnings, zero fmt diffs.                                         |

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
- [x] Implement complete suite of MIDI Transformations in Piano Roll:
  - Quantize: Snap note start times and lengths to selected grid divisions (1/4, 1/8, 1/16, 1/32).
  - Transpose: Semitone and octave shift up/down with scale snapping.
  - Chop: Divide selected notes into equal subdivisions (1 to 64 parts).
  - Strum: Successive micro-delays across chord notes with direction control.
  - Humanize: Velocity and micro-timing randomization with adjustable depth.
  - Ornament: Add grace notes and flams before note onsets.
  - Span: Legato note extension to eliminate silent gaps.
  - Invert and Reverse: Pitch inversion and time reversal across selected notes.
  - Time Warp: Non-linear acceleration and deceleration of note timing.
  - Velocity Ramp: Linear velocity contours across note selections.
- [x] Implement Generative MIDI Tools:
  - Stacks: Diatonic chord progression and triad generator.
  - Euclidean: Multi-voice Euclidean rhythm generator based on the Bjorklund algorithm.
- [x] Implement Chapter 42 keyboard shortcut navigation:
  - View toggling: `Tab` (Session/Arrangement), `Shift+Tab` / `F12` (Device/Clip View).
  - Track insertion: `Ctrl+T` (Audio Track), `Ctrl+Shift+T` (MIDI Track).
  - Track controls: `0` (Mute/Activate), `S` (Solo), `C` (Arm).
- [x] Interactive Timeline Clip Editing in Arrangement View: Clip dragging, snapping, duplicating (`Ctrl+D`), split at playhead (`Ctrl+E`), and deletion.
- [ ] Implement Stacked View toggle: Enable simultaneous viewing of Clip Detail and Device Chain, as well as Arrangement Mixer.
- [ ] Follow Actions in Session View: Action triggers (Next, Previous, First, Last, Random) on clip completion with loop counts.
- [ ] Drag-and-Drop sample loading from file system and library browser directly onto Audio Tracks, Simpler, and Drum Rack pads.

### Phase 2: Expanded DSP Device Suite and Instruments (P2)

Focus: Studio-grade sound generation and processing matching Ableton Live Suite.

- [x] Operator Synth: 4-oscillator subtractive/FM synthesizer with ADSR envelopes and resonant 24dB filters.
- [x] Drum Rack: 16-pad sample grid with synthesized drum generation, choke groups (hi-hats), pad levels, and pad solo/mute.
- [x] Simpler: Sample player with pitch tracking, looping, and filter envelopes.
- [x] Auto Pan-Tremolo: LFO modulation of stereo position or amplitude with Sine and Triangle waveforms, stereo phase offset, and Panning/Tremolo modes.
- [x] Gate: Noise gate with hysteresis (Return), lookahead, attack, hold, release, and attenuation floor controls.
- [x] EQ Three: 3-band DJ isolator equalizer with Low/Mid/High band gains (-inf to +6 dB), crossover filters, and band kill switches.
- [x] Redux: Digital downsampling (sample rate reduction) and bit reduction (quantization distortion) with jitter and dry/wet blending.
- [x] Auto Filter: Resonant multi-mode filter (Lowpass, Highpass, Bandpass, Notch) with LFO modulation and envelope follower.
- [x] Chorus-Ensemble: Multi-voice delay line modulation with quadrature stereo widening and feedback.
- [x] Saturator: Waveshaping harmonic distortion with Soft Sine, Hard Clip, Analog Warmth, and Foldback curves.
- [x] EQ Eight: 8-band parametric biquad equalizer matching Ableton EQ Eight.
- [x] Compressor: Dynamic range compressor with threshold, ratio, attack, release, makeup gain, and gain-reduction metering.
- [x] Delay: Stereo ping-pong and tempo-synced delay line with feedback filtering.
- [x] Reverb: Algorithmic stereo Schroeder/Freeverb room reverb with damping, room size, and stereo width.
- [x] Limiter: Transparent lookahead brickwall peak limiter with ceiling control.
- [x] Utility: Gain (-inf to +35 dB), stereo width (0% mono to 400% wide), pan, and phase inversion.
- [ ] Wavetable Synth: Dual wavetable oscillators, sub-oscillator, filter morphing, and modulation matrix.
- [ ] Glue Compressor: Analog-modeled VCA bus compressor with sidechain high-pass filter.
- [ ] Pitch-shifting and Time-stretching (Warp Engine): Granular time-stretch algorithm for Beats, Tones, and Texture modes.
- [ ] Melodic Stem Separation CLI and GUI tool: Built-in local offline stem separation for drums, bass, vocals, and instruments.

### Phase 3: Plugin Hosting and Interoperability (P3)

Focus: Ecosystem expansion and open standards.

- [x] Microtonal Tuning System: Scala `.scl` file format support and cents/frequency calibration.
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
- [x] MIDI transformation tools: Quantize, Transpose, Chop, Strum, Humanize, Ornament, Span, Invert, Reverse, Time Warp, Velocity Ramp.
- [x] Generative MIDI tools: Diatonic chord Stacks, multi-voice Euclidean rhythms.
- [x] Saturator / Drive audio effect device for harmonic warmth and clip shaping.
- [x] Decoupled Arrangement timeline clips with absolute beat timing and interactive editing.
- [x] Full Drum Rack instrument implementation with choke groups and 16-pad matrix.
- [x] Auto Filter resonant multi-mode filter with LFO and envelope follower.
- [x] Chorus-Ensemble modulated delay effect with quadrature stereo widening.
- [x] Auto Pan-Tremolo LFO stereo panner and tremolo device.
- [x] Gate noise gate with hysteresis (Return), attack, hold, release, and floor.
- [x] EQ Three 3-band DJ style isolator equalizer with band kill switches.
- [x] Redux sample rate reducer and bitcrusher.
- [x] Microtonal Tuning System with Scala `.scl` format parser and frequency calibration.
- [x] Comprehensive Ableton Live 12 Reference Manual documentation suite (43-chapter spec, 42 audio effects, 8 MIDI effects, 13 instruments, keyboard shortcuts).
- [x] Chapter 42 keyboard navigation: `Shift+Tab`/`F12`, `Ctrl+T`, `Ctrl+Shift+T`, `0`, `S`, `C`.
- [x] Real-time AudioEngine dynamic project and parameter synchronization.
- [x] Complete `.als` XML round-trip preservation of devices, parameters, clips, scale, and tuning.
- [x] Open-source community readiness: Dual License (MIT / Apache-2.0), CONTRIBUTING.md, Issue and PR templates.
- [ ] Multi-clip simultaneous editing in Piano Roll.
- [ ] CLAP plugin hosting support.
