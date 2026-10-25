# LiveCraft Master Roadmap: Production-Grade Ableton Live Recreation

## 1. Executive Overview and Strategic Vision

LiveCraft is an open-source, native Rust digital audio workstation (DAW) engineered for high performance, deterministic audio rendering, and deep interoperability with Ableton Live sets (`.als`).

The strategic target is divided into two continuous tiers:

1. Baseline: 1:1 functional, structural, and visual parity with Ableton Live 12.1+ (Session View, Arrangement View, audio engine, DSP rack, MIDI sequencing, and file format round-trip).
2. Superiority: Exceeding Ableton Live in areas where conventional DAWs struggle:
   - Full headless operation and terminal-driven automation (batch stem exports, headless set patching, CI audio testing).
   - Zero-telemetry, ultra-low resource footprint with immediate startup (< 150 ms).
   - Lock-free, allocation-free pure-Rust audio engine with native memory safety.
   - Built-in collaborative version control friendly formats (bidirectional clean JSON and Git-friendly `.als` diffing).
   - Built-in modern scale and microtonal tuning systems with zero third-party plugin requirements.

## 2. Audit of Existing Codebase and Gap Analysis

### 2.1 Current State Audit

| Subsystem                                                                                                   | Implemented Components                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          | Audit Verdict                                                                                                                                          |
| :---------------------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | :----------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`livecraft-core`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/lib.rs) | Project, Track, Clip, ClipSlot, Scene, Transport, Device models, authentic `.als` gzip XML schema parser and exporter, `.ask` theme parser, Command history undo/redo stack with capacity limits, AutomationLane and AutomationPoint with interpolation, ArrangementClip non-destructive editing (split, duplicate, trim), ScaleConfig with 12 modes, Scala `.scl` tuning system, MIDI transformations, and Generative tools.                                                                                                   | 13 unit tests passing. Complete architectural foundation aligned with Ableton Live 12 Manual Chapters 3, 5, 8, 10, 11, and 15.                         |
| [`livecraft-dsp`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/lib.rs)   | CPAL audio engine, lock-free AudioParamMsg parameter dispatch queue, dynamic topological DSP processor chain (`DspProcessor`), sample-accurate polyphonic voice allocation, continuous audio streaming with Hermite interpolation, In/Auto/Off monitoring logic, multi-channel return summing, Operator 4-operator FM synth with 11 routing algorithms, Simpler 3-mode sampler, Drum Rack 16-pad matrix with choke groups, EQ Eight 8-band parametric filter and curve, Dynamics and Modulation suite, and offline WAV bouncer. | 25 unit tests passing. High-performance allocation-free audio pipeline with 16 studio-grade DSP devices. Conforms to neutral operations in Chapter 39. |
| [`livecraft-gui`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/lib.rs)   | Ableton Live 12 styled immediate-mode UI (`egui`/`eframe`/`glow`), Top control bar, Session View clip matrix with launch triggers, Mixer strips with volume faders and VU meters, Piano Roll note editor with Scale and Fold mode, MIDI tools, Bottom modular device chain with interactive curves, Left library browser, and full Chapter 42 keyboard shortcut mapping.                                                                                                                                                        | Visually faithful Ableton Live 12 layout with real-time audio engine synchronization.                                                                  |
| [`livecraft-cli`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-cli/src/main.rs)  | Subcommands: `new`, `info`, `inspect`, `render`, `stems`, `play`, `synth`, `convert`, `gui`.                                                                                                                                                                                                                                                                                                                                                                                                                                    | 4 integration tests passing. Rich headless command-line interface, fully operational for rendering, inspect, and terminal playback.                    |
| Infrastructure                                                                                              | GitHub CI workflow (fmt, clippy, test, release build for Windows and Ubuntu).                                                                                                                                                                                                                                                                                                                                                                                                                                                   | Passing clean with 44 unit and integration tests across workspace, zero clippy warnings, zero fmt diffs.                                               |

## 3. Comprehensive Feature and Workflow Roadmap

### Phase 0: Audio Engine Real-Time Hardening and Core Data Integrity (P0)

Focus: Lock-free concurrency, zero allocations on the audio thread, and robust document modeling.

- [x] Fix audio callback allocations: Pre-allocated ring buffers and static scratch arrays in CPAL callback.
- [x] Decouple audio state with lock-free parameter dispatch queue (`AudioParamMsg`).
- [x] Implement dynamic modular DSP processor chain (`DspProcessor`) with topological ordering.
- [x] Implement sample-accurate voice allocation and polyphonic note lifecycle tracking.
- [x] Implement audio clip continuous sample streaming with cubic Hermite interpolation.
- [x] Implement In/Auto/Off monitoring logic and multi-channel return summing with 32-bit float headroom.
- [x] Integrate global Key and Scale systems into Project and Clip (12 roots and 12 modes).
- [x] Add timeline automation envelopes and breakpoint curve model with undo/redo actions.
- [x] Implement non-destructive arrangement clip editing (split at beat, duplicate, trim).
- [x] Integrate Scala `.scl` microtonal tuning engine across synthesizers and samplers.
- [ ] Implement multi-threaded track processing pool with work-stealing for multi-core DSP execution.
- [ ] Add dynamic buffer size switching and sample rate conversion (32000 to 192000 Hz).
- [ ] Add true ASIO driver support on Windows alongside WASAPI.

### Phase 1: Ableton Live 12 Interface Parity and Workflow Polish (P1)

Focus: Visual elegance, fluid interactive controls, and Ableton 12 design paradigms.

- [x] Implement Piano Roll Scale Mode: Key and root selector, in-scale note highlighting, and Fold to Scale toggle.
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
- [x] Interactive Timeline Clip Editing in Arrangement View: Clip dragging, snapping, duplicating, split at playhead, and deletion.
- [ ] Implement Stacked View toggle: Enable simultaneous viewing of Clip Detail and Device Chain, as well as Arrangement Mixer.
- [ ] Follow Actions in Session View: Action triggers (Next, Previous, First, Last, Random) on clip completion with loop counts.
- [ ] Drag-and-Drop sample loading from file system and library browser directly onto Audio Tracks, Simpler, and Drum Rack pads.

### Phase 2: Expanded DSP Device Suite and Instruments (P2)

Focus: Studio-grade sound generation and processing matching Ableton Live Suite.

- [x] Operator Synth: 4-operator polyphonic FM and subtractive synthesizer with 11 routing algorithms, per-op waveforms, ADSR envelopes, feedback, and resonant filter.
- [x] Simpler: Polyphonic sample player with Classic, 1-Shot, and Slicing modes, looping, crossfade, and pitch tracking.
- [x] Drum Rack: 16-pad sample and synth drum machine with choke groups (hi-hats), pad levels, per-pad solo, mute, and pan routing.
- [x] EQ Eight: 8-band parametric biquad equalizer with 7 filter topologies and analytical response curve calculation.
- [x] Auto Pan-Tremolo: LFO modulation of stereo position or amplitude with Sine and Triangle waveforms, stereo phase offset, and Panning/Tremolo modes.
- [x] Gate: Noise gate with hysteresis (Return), lookahead, attack, hold, release, and attenuation floor controls.
- [x] EQ Three: 3-band DJ isolator equalizer with Low, Mid, and High band gains and band kill switches.
- [x] Redux: Digital downsampling (sample rate reduction) and bit reduction (quantization distortion) with jitter and dry/wet blending.
- [x] Auto Filter: Resonant multi-mode filter (Lowpass, Highpass, Bandpass, Notch) with LFO modulation and envelope follower.
- [x] Chorus-Ensemble: Multi-voice delay line modulation with quadrature stereo widening and feedback.
- [x] Saturator: Waveshaping harmonic distortion with Soft Sine, Hard Clip, Analog Warmth, and Foldback curves.
- [x] Compressor: Dynamic range compressor with threshold, ratio, attack, release, makeup gain, adjustable knee, and gain reduction metering.
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

- [x] Authentic Ableton Live 12 Set XML Schema mapping and gzip `.als` serialization.
- [x] Microtonal Tuning System: Scala `.scl` file format support and cents/frequency calibration across synthesizers.
- [ ] CLAP (Clever Audio Plugin) host implementation: Pure Rust hosting of CLAP instruments and audio effects.
- [ ] VST3 plugin host implementation on Windows, macOS, and Linux.
- [ ] MIDI Hardware Control Surface integration (Push 2/3, Novation Launchpad, generic MIDI Learn).
- [ ] Complete `.als` round-trip fidelity validation against Ableton Live 10, 11, and 12 sets without data loss.
- [ ] Sound library and preset system with tag filtering, category search, and fast audio preview.

## 4. Acceptance Criteria and Verification Regimes

Every milestone and pull request must meet the following automated criteria:

1. Zero Audio Callback Allocations: Audio processing callbacks must execute without calling heap allocators.
2. Continuous Formatting and Linting: `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` must pass with zero warnings.
3. Test Coverage: All 42 unit and integration tests across `livecraft-core`, `livecraft-dsp`, and `livecraft-cli` must pass without failures.
4. Deterministic Offline Bouncing: Bounced audio waveforms from identical projects must remain sample-accurate across repeated runs.
5. Cross-Platform Builds: Release binaries compile successfully across Windows (x86_64) and Linux (x86_64).

## 5. Living Progress and Autonomous Task Registry

LiveCraft is tracked via an autonomous 28-task execution manifest ([tasks/manifest.json](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/tasks/manifest.json)). Currently 14 of 28 tasks (50.0%) are verified and completed:

- [x] Task 1: Lock-Free Parameter Dispatch and Audio State Decoupling (`task-001`)
- [x] Task 2: Dynamic Modular DSP Processor Chain and Topological Order (`task-002`)
- [x] Task 3: Sample-Accurate Voice Allocation and Polyphonic Note Lifecycle (`task-003`)
- [x] Task 4: Audio Clip Continuous Sample Streaming and Hermite Interpolation (`task-004`)
- [x] Task 5: In/Auto/Off Monitoring Logic and Multi-Channel Return Summing (`task-005`)
- [x] Task 6: Authentic Ableton Live 12 XML Schema Mapping (.als Parser) (`task-006`)
- [x] Task 7: Authentic Ableton Live 12 Set Serializer (.als Exporter) (`task-007`)
- [x] Task 8: Timeline Automation Envelopes and Breakpoint Curve Model (`task-008`)
- [x] Task 9: Arrangement Clip Non-Destructive Editing (`task-009`)
- [x] Task 10: Scala .scl Microtonal Engine Integration Across Synthesizers (`task-010`)
- [x] Task 11: Operator 4-Operator FM Synthesizer with 11 Routing Algorithms (`task-011`)
- [x] Task 12: Simpler Polyphonic Sampler (Classic, 1-Shot, Slicing) (`task-012`)
- [x] Task 13: Drum Rack 16-Pad Matrix, Choke Groups and Solo/Mute Routing (`task-013`)
- [x] Task 14: EQ Eight 8-Band Parametric Filter and Frequency Response Curve (`task-014`)
- [ ] Task 15: Dynamics Suite (Compressor Knee, Brickwall Limiter, Gate Hysteresis) (`task-015`)
- [ ] Task 16: Modulation and Space Effects (`task-016`)
- [ ] Tasks 17 to 24: Ableton Live 12 Desktop Interface Parity
- [ ] Tasks 25 to 28: Headless Workflows, Visual Regression Testing, CI and Documentation Audit
