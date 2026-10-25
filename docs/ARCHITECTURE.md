# LiveCraft Technical Architecture and Engineering Manual

A clean-room, native Rust digital audio workstation engineered for deterministic audio rendering, low latency, and structural parity with Ableton Live 12.

## 1. Architectural Philosophy and Design Principles

LiveCraft is built from first principles in safe, native Rust without relying on Electron, web views, or legacy C++ DAW codebases. Its architecture strictly adheres to three foundational tenets:

1. Deterministic Audio Pipeline: Every calculation in the DSP graph produces mathematically verifiable, bit-transparent results across platforms, conforming to the neutral operations guidelines in Ableton Live 12 Manual Chapter 39.
2. Lock-Free Real-Time Safety: The real-time audio thread executing the hardware callback never allocates heap memory, acquires mutual exclusion locks, or makes blocking operating system calls, conforming to Chapter 38.
3. Dual-Surface Execution: Every operation possible within the graphical interface is exposed via the headless command-line interface, enabling automated rendering, set patching, CI audio verification, and terminal playback.

## 2. Workspace Organization and Crate Boundaries

```text
LiveCraft/
|-- Cargo.toml                  # Workspace manifest
|-- docs/                       # Authoritative documentation suite
|   |-- ableton-live-12-reference-manual.pdf  # Definitive 1039-page source of truth
|   |-- ABLETON_12_SPECIFICATION.md          # 43-chapter system specification
|   |-- DEVICES_REFERENCE.md                 # 42 audio FX, 8 MIDI FX, 13 synths catalog
|   |-- MIDI_TOOLS_AND_TUNING.md             # Transformations, Generators, Scala engine
|   |-- KEYBOARD_SHORTCUTS.md                # Comprehensive shortcut mapping
|   |-- ARCHITECTURE.md                      # Engineering design manual
|   `-- README.md                            # Documentation overview
|-- crates/
|   |-- livecraft-core/         # Pure-data document model, .als/.ask parser, commands
|   |-- livecraft-dsp/          # Lock-free audio graph, DSP processors, CPAL engine
|   |-- livecraft-gui/          # Immediate-mode desktop UI (egui / eframe)
|   `-- livecraft-cli/          # Command-line entry point and integration test harness
|-- tasks/                      # Implementation registry, manifest, and specifications
|-- scripts/                    # Task runner and build verification utilities
|-- DESIGN.md                   # Ableton Live 12 UI/UX design tokens and widget specifications
|-- ROADMAP.md                  # Living master roadmap and parity matrix
|-- RESEARCH.md                 # Research dossier and architecture evaluation
`-- CONTRIBUTING.md             # Open-source developer guidelines
```

## 3. Crate Deep Dive

### 3.1 livecraft-core (Domain Model, Formats, and State Mutation)

The [livecraft-core](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/lib.rs) crate is a pure data crate containing zero audio drivers, DSP routines, or platform-specific I/O code. It models the complete DAW project state in memory, parses and serialises external file formats, and provides a reversible command history.

#### Document Data Model

The document model resides in [model.rs](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs):

- [`Project`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs): Root document container managing tempo, time signature, transport status, track lists, scene lists, return tracks, master track, active scale configuration, and tuning system.
- [`Track`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs): Represents Audio, MIDI, Return, and Master tracks. Holds volume in decibels, stereo pan position, mute, solo, arm flags, monitoring mode ([`MonitoringMode`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs)), send gain levels, device chain, clip slots for Session View, arrangement clips for timeline editing, and automation lanes.
- [`ClipSlot`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) and [`Clip`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs): Session matrix elements storing clip payloads ([`ClipPayload::Midi`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) or [`ClipPayload::Audio`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs)), loop parameters, start and end markers, and colour tags.
- [`ArrangementClip`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs): Non-destructive timeline clip container placing a [`Clip`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) at an absolute beat start position with a specific beat duration. Provides non-destructive editing operations:
  - `split_at(split_beat, new_id)`: Divides a timeline clip into two contiguous segments, slicing MIDI notes at the split boundary and adjusting relative beat offsets.
  - `duplicate(new_id, offset_beats)`: Clones an arrangement clip to a new timeline start position.
  - `trim_start(delta_beats)` and `trim_end(delta_beats)`: Non-destructively adjusts clip boundaries along the timeline.
- [`AutomationLane`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) and [`AutomationPoint`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs): Timeline breakpoint curves attached to a target ([`AutomationTarget`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs): TrackVolume, TrackPan, SendLevel, DeviceParam). Points store beat position, target value, and curve tension. The `evaluate_at(beat)` method provides linear interpolation across breakpoint segments with boundary clamping.
- [`ScaleConfig`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) and [`ScaleType`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs): Supports 12 diatonic and ethnic scale modes (Major, Minor, Harmonic Minor, Melodic Minor, Dorian, Phrygian, Lydian, Mixolydian, Locrian, Pentatonic Major, Pentatonic Minor, Blues) across all 12 root keys with pitch snapping and in-scale testing.
- [`TuningSystem`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs): Full Scala `.scl` microtonal tuning file parser supporting arbitrary degree divisions, non-12-TET intervals, cents-to-frequency conversions, and reference pitch calibration.

#### Command History and Undo Redo

The command subsystem resides in [command.rs](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/command.rs):

- [`CommandHistory`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/command.rs): Manages undo and redo stacks with configurable history capacity limits.
- Supports reversible project operations including tempo changes ([`SetTempoAction`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/command.rs)), automation breakpoint additions ([`AddAutomationPointAction`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/command.rs)), and breakpoint removals ([`RemoveAutomationPointAction`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/command.rs)).

#### File Format Interchange

- [`AlsParser`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/als.rs): Decompresses gzip streams and parses authentic Ableton Live 10, 11, and 12 XML hierarchies. Maps XML nodes including `<MidiTrack>`, `<AudioTrack>`, `<ReturnTrack>`, `<DeviceChain>`, `<Mixer>`, `<Volume>`, `<Pan>`, `<Devices>`, `<Operator>`, `<Eq8>`, `<Compressor2>`, `<ClipSlot>`, and `<MidiClip>`. Serialises projects back into valid gzip Ableton Live `.als` archives.
- [`AbletonTheme`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/theme.rs): Parses Ableton `.ask` theme files to extract precise RGB colour palettes for UI rendering.

### 3.2 livecraft-dsp (Audio Engine, Topological Graph, and Processors)

The [livecraft-dsp](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/lib.rs) crate implements real-time audio streaming, dynamic device chaining, voice allocation, and offline rendering.

#### Concurrency and Lock-Free Safety

- Real-Time Decoupling: The user interface and CLI dispatch parameter updates using lock-free message queues ([`AudioParamMsg`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/engine.rs)) via crossbeam ring buffers.
- Pre-allocated Buffers: The audio callback thread never allocates or frees memory on the heap. Scratch arrays and voice stores are allocated during initialization.
- Driver Abstraction: Uses `cpal` for low-latency platform audio streaming (WASAPI on Windows, CoreAudio on macOS, ALSA and JACK on Linux).

#### Dynamic Modular DSP Processor Chain

Modular processing resides in [processor.rs](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/processor.rs):

- [`DspProcessor`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/processor.rs) Trait: Unified interface implemented by all audio effects in the rack:
  - `process_stereo(&mut self, in_l: f32, in_r: f32) -> (f32, f32)`
  - `update_param(&mut self, key: &str, value: f32)`
  - `reset(&mut self)`
  - `is_enabled(&self) -> bool`
  - `set_enabled(&mut self, enabled: bool)`
  - `device_id(&self) -> u64`
- Topological Order: Devices are evaluated in strict series within a track device chain. Adding, removing, or re-ordering devices updates the chain dynamically without interrupting the audio stream.

#### Topological Audio Graph and Voice Scheduling

The audio graph resides in [graph.rs](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/graph.rs):

- Sample-Accurate Voice Allocation: MIDI note events are scheduled with sub-block sample offsets. Active note instances are tracked polyphonically with velocity scaling and voice stealing when voice limits are reached.
- Continuous Audio Streaming: [audio_clip.rs](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/audio_clip.rs) provides [`AudioSampleCache`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/audio_clip.rs) and cubic Hermite interpolation (`hermite_interp`) for smooth timeline audio playback.
- Monitoring Logic: Implements In, Auto, and Off monitoring behaviors conforming to Ableton Live Manual Chapter 15. When a track is armed in Auto mode, playback of existing clips is suppressed in favor of incoming monitor signals. In Off mode, recorded clips continue playing regardless of arm status.
- Multi-Channel Return Summing: Supports an arbitrary count of Return tracks. Track sends feed Return inputs with 32-bit floating point headroom before summing into the Master track.

#### Built-In Studio Device Suite

The DSP suite provides 16 studio-grade instruments and audio processors:

1. [`OperatorSynth`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/operator.rs): 4-operator polyphonic FM and subtractive synthesizer. Features 11 routing algorithms, per-operator waveforms (Sine, Triangle, Sawtooth, Square, Noise), level controls, coarse and fine pitch ratios, independent ADSR envelopes, feedback loop, and resonant 24 dB lowpass filter. Microtonal tuning is integrated via Scala tuning maps.
2. [`SimplerInstrument`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/simpler.rs): Multi-mode polyphonic sample player. Features Classic mode with looping and crossfades, 1-Shot mode with trigger or gate playback, and Slicing mode with transient division across keys.
3. [`DrumRackInstrument`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/drumrack.rs): 16-pad drum machine matrix with synthesized drum models and sample triggers. Features choke groups for mutual exclusion (such as open and closed hi-hat cutoffs), per-pad mute, solo, and pan routing.
4. [`EqEight`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/eq8.rs): 8-band parametric biquad equalizer matching Ableton EQ Eight. Supports LowPass, HighPass, BandPass, Notch, Peaking, LowShelf, and HighShelf filter topologies. Provides analytical response curve calculations (`magnitude_at_freq`, `gain_db_at_freq`, `response_curve_points`) for real-time visual spectrum plotting.
5. [`CompressorEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/compressor.rs): Feedforward dynamic range compressor with threshold, ratio, attack, release, makeup gain, adjustable knee, and real-time gain reduction metering.
6. [`LimiterEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/limiter.rs): Transparent lookahead brickwall peak limiter with ceiling and release controls.
7. [`GateEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/gate.rs): Noise gate with hysteresis return, lookahead, attack, hold, release, and attenuation floor.
8. [`AutoFilterEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/autofilter.rs): Resonant multi-mode filter with LFO modulation and dynamic envelope follower.
9. [`SaturatorEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/saturator.rs): Waveshaping harmonic distortion with Soft Sine, Hard Clip, Analog Warmth, and Foldback transfer curves.
10. [`EqThreeEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/eq3.rs): 3-band DJ isolator equalizer with Low, Mid, and High band gains and kill switches.
11. [`ReduxEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/redux.rs): Sample rate downsampler and bitcrusher with quantization distortion and jitter.
12. [`DelayEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/delay.rs): Stereo ping-pong and tempo-synced delay line with feedback filtering.
13. [`ReverbEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/reverb.rs): Algorithmic Schroeder and Freeverb stereo room reverb with damping, room size, and stereo width.
14. [`ChorusEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/chorus.rs): Multi-delay modulation with stereo widening and shimmer feedback.
15. [`AutoPanEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/autopan.rs): LFO modulation of stereo position or amplitude with Sine and Triangle shapes, phase offset, and Panning or Tremolo modes.
16. [`UtilityEffect`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/limiter.rs): Channel utility with gain (-inf to +35 dB), stereo width (0% mono to 400% wide), pan, and phase inversion.

#### Offline Bouncing and Stem Export

Residing in [bouncer.rs](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/bouncer.rs):

- High-speed offline rendering to 16, 24, or 32-bit float WAV files using `hound`, operating approximately 133 times faster than real time.
- Multi-track stem export rendering each track to an isolated audio file in parallel.

### 3.3 livecraft-gui (Immediate Mode User Interface)

The [livecraft-gui](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/lib.rs) crate renders the Ableton Live 12 desktop interface using `egui`, `eframe`, and hardware acceleration via OpenGL (`glow`). Detailed color tokens, spatial dimensions, piecewise tapered dB mappings, and component standards are specified in [DESIGN.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/DESIGN.md).

- Top Control Bar: 9-section control bar managing Link, Tap tempo, BPM LCD with nudge, Time Signature, Metronome, Quantization, Scale toggle and Root selector, Follow toggle, Position counter, Transport controls (Play, Stop, Record), Arrangement Loop, Draw mode, Computer MIDI Keyboard, and CPU load meter.
- Session View: Vertical track columns with clip slot matrices, animated progress fills, slot trigger buttons, track stop buttons, Master Scene launches, and Stop All Clips.
- Mixer View: In/Auto/Off monitoring buttons, Send knobs, Pan knobs with center detents, vertical volume faders with graduated dB markings, calibrated 24-segment stereo peak VU meters, track activators, solo, and arm buttons.
- Arrangement View: Multi-track horizontal timeline, bar and beat ruler, draggable loop brace, scrubbing playhead marker, and clip representations with note previews and waveform contours.
- Clip Detail and Piano Roll: 128-note grid, diatonic scale awareness, Fold to Scale toggle, note creation, velocity handles, and MIDI transformation tools (Chop, Strum, Quantize, Transpose, Humanize, Ornament, Span, Invert, Reverse, Time Warp, Velocity Ramp, Stacks, Euclidean).
- Modular Device Rack: Horizontal device chain showing active devices, interactive parameter knobs, EQ Eight frequency curves, and Saturator transfer functions.
- Browser Sidebar: Hierarchical asset tree covering Collections, Categories (Sounds, Drums, Instruments, Audio Effects, MIDI Effects, Plug-Ins, Clips, Samples), and Places.

### 3.4 livecraft-cli (Headless Automation)

The [livecraft-cli](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-cli/src/main.rs) crate exposes all DAW functions for automated scripts, CI verification, and terminal use:

- `livecraft info <set.als>`: Displays project metadata, tempo, track count, and scene count.
- `livecraft inspect <set.als>`: Deep inspection of devices, clips, parameters, and optional JSON output.
- `livecraft new <project.als>`: Generates a fresh Ableton Live set or JSON template.
- `livecraft render <set.als> -o out.wav`: Offline audio bounce to WAV.
- `livecraft stems <set.als> -o ./stems`: Multi-track stem export.
- `livecraft play <set.als>`: Terminal audio playback with live ASCII stereo VU meters.
- `livecraft synth`: Terminal waveform tone generation.
- `livecraft convert`: Bidirectional format conversion between `.als` and JSON.
- `livecraft gui`: Launches the desktop interface.

## 4. Strict Conformance to Audio and MIDI Fact Sheets

- Summing Integrity (Manual Chapter 39): Audio signals are summed in 32-bit floating point arithmetic without digital clipping before the master output stage. When gain is at 0.0 dB and no processing is applied, output audio matches input audio bit-for-bit.
- Sample-Accurate MIDI Timing (Manual Chapter 40): Note triggers are scheduled using sub-block sample offsets rather than sample-by-sample vector iterations, eliminating jitter and preserving micro-timing across multi-channel tracks.
