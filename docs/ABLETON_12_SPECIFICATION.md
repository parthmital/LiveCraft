# Ableton Live 12 Complete System Specification and LiveCraft Architecture

This document serves as the comprehensive architectural specification for LiveCraft, derived directly from the official Ableton Live 12 Reference Manual (`docs/ableton-live-12-reference-manual.pdf`, 1039 pages, 43 chapters).

## 1. Chapter by Chapter Specification and LiveCraft Mapping

### Chapter 1: Welcome to Live (pp. 23)

- Overview of Live's dual paradigm: real-time spontaneous performance (Session View) and linear timeline composition (Arrangement View).
- LiveCraft mapping: Unified data document model ([`Project`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs)) powering both Session and Arrangement paradigms simultaneously.

### Chapter 2: First Steps and Settings (pp. 24-34)

- Core settings: Audio driver selection (WASAPI, ASIO, CoreAudio, ALSA, JACK), sample rates (44.1 kHz to 192 kHz), buffer sizes (32 to 2048 samples).
- Theme engine: Visual styling, UI scaling, colour palettes, and contrast levels.
- LiveCraft mapping: [`AudioEngine`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/engine.rs) configuration with CPAL, paired with [`AbletonTheme`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/theme.rs) parser supporting Ableton `.ask` theme files with exact colour reproduction.

### Chapter 3: Live Concepts (pp. 35-61)

- The Control Bar: Transport controls, tempo, time signature, metronome, quantization, follow playback toggle.
- The Browser: Hierarchical asset management (Categories, Places, Collections, Tags).
- Tracks and Signal Flow: Audio Tracks, MIDI Tracks, Return Tracks, Master Track.
- Signal Formats: 32-bit floating point summing, sample-accurate MIDI event scheduling.
- LiveCraft mapping: [`livecraft_core::model`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) defining [`Track`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs), [`TrackType`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs), [`Project`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs), and [`Transport`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs).

### Chapter 4: Working with the Browser (pp. 62-121)

- Category taxonomy: Sounds, Drums, Instruments, Audio Effects, MIDI Effects, Modulators, Max for Live, Plug-Ins, Clips, Samples.
- Live 12 Similarity Search: Sound similarity analysis and hot-swapping.
- Tagging and Filter Groups: Content filtering by Creator, Character, Format, and Mood.
- LiveCraft mapping: [`livecraft_gui::browser`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/browser.rs) with category tree, device instant insertion, and sample preview.

### Chapter 5: Managing Files and Sets (pp. 122-164)

- Live Set (`.als`): Gzip-compressed XML document storing project graph, devices, parameters, and clips.
- Project Folders: Self-contained project hierarchy containing Ableton Project Info, Samples, and Sets.
- Decoding Cache and Analysis Files (`.asd`): Sample waveform overview, tempo detection, and warp markers.
- LiveCraft mapping: [`AlsParser`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/als.rs) providing streaming gzip XML decompression, deserialization, authentic Live 12 tag serialization, and compression.

### Chapter 6: Arrangement View (pp. 165-186)

- Timeline structure: Linear horizontal track lanes with bar, beat, and sixteenth ruler.
- Clip Editing: Split (`Ctrl+E`), Duplicate (`Ctrl+D`), crop, resize, slip editing, and fades.
- Mixer integration: In-line track volume faders, pan controls, mute, solo, and arm buttons in Arrangement View.
- LiveCraft mapping: [`livecraft_gui::arrange`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/arrange.rs) and [`ArrangementClip`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) non-destructive editing operations.

### Chapter 7: Session View (pp. 187-199)

- Clip Slot Matrix: Vertical track strips with launch buttons, slot triggers, stop buttons.
- Scene Launch: Horizontal rows triggered by Master Scene launch buttons with per-scene tempo and time signature.
- Follow Actions: Automatic progression between clips (Next, Previous, First, Last, Any, Other) with probability ratios.
- LiveCraft mapping: [`livecraft_gui::session`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/session.rs) and [`ClipSlot`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs).

### Chapter 8: Clip View (pp. 200-233)

- Unified Clip Detail: Sample Editor for audio clips, Piano Roll for MIDI clips.
- Simultaneous Clip and Device viewing: Stacked layout introduced in Live 12.
- Loop and Warp Controls: Loop brackets, start/end markers, time signatures.
- LiveCraft mapping: [`livecraft_gui::pianoroll`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/pianoroll.rs) and bottom detail split.

### Chapter 9: Audio Clips, Tempo, and Warping (pp. 234-251)

- Warp Modes:
  - Beats: Granular transient preservation with slicing options (transient, 1/16, 1/8) and loop/decay shaping.
  - Tones: Granular pitch tracking for monophonic audio (vocals, basslines).
  - Texture: Polyphonic texture stretching with adjustable grain size and flux.
  - Re-Pitch: Turntable style speed/pitch coupling.
  - Complex and Complex Pro: Polyphonic formant-preserving phase-vocoder stretching.
- LiveCraft mapping: [`WarpMode`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) and continuous sample streaming in [`AudioSampleCache`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/audio_clip.rs).

### Chapter 10: Editing MIDI (pp. 252-292)

- Piano Roll Editor: Fold to Scale mode, microtonal scale highlighting, 128 MIDI notes, velocity lane, chance/probability lane.
- Multi-Clip Editing: Simultaneous viewing and editing of overlapping MIDI clips across multiple tracks.
- LiveCraft mapping: [`livecraft_gui::pianoroll`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/pianoroll.rs) scale-aware keyboard canvas with 16th-note snapping.

### Chapter 11: MIDI Tools: Transformations and Generators (pp. 293-329)

- Transformation Tools:
  - Arpeggiate: Splits notes into rhythmic patterns across 18 styles.
  - Chop: Slices selected notes into 1 to 64 subdivisions with timing variations.
  - Connect: Synthesizes interstitial notes between sparse melodic anchors.
  - Glissando and Ornament: Adds pitch bends, grace notes, and flams.
  - Quantize: Aligns note start times and durations to musical divisions (1/4 to 1/32).
  - Span: Stretches notes to meet subsequent onsets or grid boundaries.
  - Strum: Staggers chord note start times with acceleration/deceleration.
  - Time Warp: Non-linear timing curves accelerating or dragging notes.
  - Velocity Shaper: Dynamic envelope shaping across note selections.
- Generative Tools:
  - Rhythm: Generates rhythmic note patterns across specified bar spans.
  - Seed: Algorithmic pitch and rhythm generation bounded by scale rules.
  - Shape: Generates melodic contours (sine, ramp, triangle) mapped to scale degrees.
  - Stacks: Diatonic chord progressions and triads constructed on scale degrees.
  - Euclidean: Bjorklund distributed pulse generator across up to 4 polyphonic voices.
- LiveCraft mapping: [`MidiClipData`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) transformation engine.

### Chapter 12: Editing MPE (pp. 330-339)

- MIDI Polyphonic Expression: Per-note pitch bend, slide (CC 74), and pressure (aftertouch).
- LiveCraft mapping: Lock-free per-voice MPE modulation buffers in `livecraft_dsp`.

### Chapter 13: Converting Audio to MIDI (pp. 340-344)

- Three conversion algorithms: Harmony (polyphonic), Melody (monophonic), and Drums (transient-sliced).
- LiveCraft mapping: Fast Fourier Transform (FFT) peak detection and onset analysis pipeline.

### Chapter 14: Using Grooves (pp. 345-350)

- Groove Pool: Swing, timing jitter, and velocity modulation extracted from audio or MIDI templates.
- LiveCraft mapping: Micro-timing offset mapping applied prior to event scheduling.

### Chapter 15: Using Tuning Systems (pp. 351-359)

- Microtonal and Non-Western Scales: Full support for Scala `.scl` and `.ascl` files.
- Reference frequency calibration (such as A3 = 440 Hz) and cents/ratio interval calculations.
- LiveCraft mapping: [`TuningSystem`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) with frequency calibration across [`OperatorSynth`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/operator.rs) and [`SimplerInstrument`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/simpler.rs).

### Chapter 16: Launching Clips (pp. 360-371)

- Launch Modes: Trigger, Gate, Toggle, Repeat.
- Quantization: None, 8 Bars down to 1/32 note, Legato mode.
- Follow Action probability: A/B ratio triggers for generative song structures.
- LiveCraft mapping: Quantized clip trigger queue in [`AudioGraph`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/graph.rs).

### Chapter 17: Routing and I/O (pp. 372-393)

- Flexible routing matrix: Track In/Out, Pre-FX, Post-FX, Post-Mixer, Sidechain tapping.
- Sub-grouping and Bus Tracks: Multi-channel summing into parent stems.
- LiveCraft mapping: Directed Acyclic Graph (DAG) topological sorting in [`AudioGraph`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/graph.rs).

### Chapter 18: Mixing (pp. 394-411)

- Channel Strip: Volume fader (-inf to +6 dB), stereo pan, Mute, Solo, Crossfader assignment.
- Metering: Peak and RMS visual metering with clipping indicators (+0 dBFS).
- Return Tracks and Master Track: Aux sends with latency-compensated delay buffers.
- LiveCraft mapping: [`livecraft_gui::mixer`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/mixer.rs) and [`AudioGraph`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/graph.rs).

### Chapter 19: Recording New Clips (pp. 412-424)

- Session and Arrangement recording with count-in metronome (1, 2, 4 bars).
- Punch-In and Punch-Out markers for automated timeline overdubs.
- LiveCraft mapping: Circular input capture buffer with ring-buffer synchronization.

### Chapter 20: Bounce to Audio (pp. 425-430)

- Offline faster-than-realtime rendering to 16, 24, and 32-bit float WAV/AIFF.
- Stems Export: All individual tracks bounced simultaneously to isolated audio files.
- LiveCraft mapping: [`OfflineBouncer`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/bouncer.rs) rendering approximately 133 times faster than real time.

### Chapter 21: Comping (pp. 431-436)

- Take Lanes: Recording multiple passes onto stacked lanes, highlighting and splicing best takes into the master lane.
- LiveCraft mapping: Hierarchical sub-lane model per audio/MIDI track.

### Chapter 22: Stem Separation (pp. 437-444)

- Offline multi-source audio separation into 4 isolated stems: Drums, Bass, Vocals, Other.
- LiveCraft mapping: CLI subcommand `livecraft stems` and stem export pipeline.

### Chapter 23: Working with Instruments and Effects (pp. 445-478)

- Device Chains: Left-to-right processing order (MIDI Effect -> Instrument -> Audio Effects).
- Dry/Wet mixing, parameter automation, and device on/off activators.
- LiveCraft mapping: [`Device`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) and [`DspProcessor`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/processor.rs) dynamic topological chain.

### Chapter 24: Using Plug-Ins (pp. 479-490)

- VST3 and CLAP plugin hosting standards with parameter automation and preset loading.
- LiveCraft mapping: Native Rust plugin host harness with cross-platform ABI.

### Chapter 25: Instrument, Drum and Effect Racks (pp. 491-510)

- Modular racks: Parallel processing chains, key/velocity/chain zones, 8 to 16 Macro controls with mapping curves.
- Drum Rack: 16-pad sample/synth matrix with choke groups (such as hi-hat cutoff).
- LiveCraft mapping: [`DrumRackInstrument`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/drumrack.rs) with choke group mutual exclusion.

### Chapters 26 and 27: Automation and Clip Envelopes (pp. 511-545)

- Parameter automation: Breakpoint envelopes across timeline bars for mixer and device controls.
- Clip Modulation: Relative modulation curves residing within clips independent of global timeline automation.
- LiveCraft mapping: [`AutomationLane`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) and [`AutomationPoint`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-core/src/model.rs) with linear interpolation.

### Chapter 28: Working with Video (pp. 546-551)

- Video clip playback synchronized with Arrangement View audio timeline.
- LiveCraft mapping: Frame-accurate video presentation timestamp (PTS) synchronization.

### Chapter 29: Live Audio Effect Reference (pp. 552-690)

- Detailed specification of all 42 built-in Audio Effects: Amp, Auto Filter, Auto Pan-Tremolo, Auto Shift, Beat Repeat, Cabinet, Channel EQ, Chorus-Ensemble, Compressor, Corpus, Delay, Drum Buss, Dynamic Tube, Echo, EQ Eight, EQ Three, Erosion, External Audio Effect, Filter Delay, Gate, Glue Compressor, Grain Delay, Hybrid Reverb, Limiter, Looper, Multiband Dynamics, Overdrive, Pedal, Phaser-Flanger, Redux, Resonators, Reverb, Roar, Saturator, Shifter, Spectral Resonator, Spectral Time, Spectrum, Tuner, Utility, Vinyl Distortion, Vocoder.
- Detailed parameter breakdown documented in [DEVICES_REFERENCE.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/DEVICES_REFERENCE.md).

### Chapter 30: Live MIDI Effect Reference (pp. 691-703)

- Detailed specification of 8 MIDI Effects: Arpeggiator, CC Control, Chord, Note Length, Pitch, Random, Scale, Velocity.
- Detailed parameter breakdown documented in [DEVICES_REFERENCE.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/DEVICES_REFERENCE.md).

### Chapter 31: Live Instrument Reference (pp. 704-823)

- Detailed specification of 13 built-in Instruments: Analog, Collision, Drift, Drum Sampler, Electric, External Instrument, Impulse, Meld, Operator, Sampler, Simpler, Tension, Wavetable.
- Detailed parameter breakdown documented in [DEVICES_REFERENCE.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/DEVICES_REFERENCE.md).

### Chapters 32 and 33: Max for Live Devices and Extensibility (pp. 824-864)

- Live Object Model (LOM) architecture and modulation devices (LFO, Shaper, Envelope Follower).
- LiveCraft mapping: Extensible DSP device trait enabling modular custom processing blocks.

### Chapter 34: MIDI and Key Remote Control (pp. 865-873)

- Computer Keyboard as MIDI Controller (keys A-K as white keys, W,E,T,Y,U as black keys, Z/X for octave transpose, C/V for velocity adjustment).
- MIDI Map Mode (`Ctrl+M`) and Key Map Mode (`Ctrl+K`).
- LiveCraft mapping: [`livecraft_gui::transport`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-gui/src/transport.rs) and virtual keyboard input mapper.

### Chapters 35 and 36: Hardware Control Surfaces (Push 1 and Push 2) (pp. 874-972)

- 64-pad grid integration, 8 endless rotary encoders, touch strip, navigation, and display feedback.
- LiveCraft mapping: Dedicated USB/MIDI surface controller loop.

### Chapter 37: Synchronization (Link, Tempo Follower, MIDI Sync) (pp. 973-981)

- Ableton Link protocol over local network, MIDI Clock in/out, and Audio tempo follower.
- LiveCraft mapping: Lock-free phase alignment ring buffer in `livecraft_dsp`.

### Chapter 38: Computer Audio Resources and Strategies (pp. 982-988)

- Real-time CPU management: Overload indicator, track freezing, multi-core DSP execution, allocation avoidance.
- LiveCraft mapping: Allocation-free audio callback in [`AudioEngine`](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/crates/livecraft-dsp/src/engine.rs) using lock-free parameter dispatch.

### Chapters 39 and 40: Audio and MIDI Fact Sheets (pp. 989-1002)

- Neutral Operations: Sample-accurate summing, 32-bit floating point headroom, bit-transparent bypassing, zero-latency gain scaling.
- Non-Neutral Operations: Dithering, sample rate conversion, phase shifts in IIR filters.
- LiveCraft mapping: Strict adherence to neutral DSP mathematics, preserving full dynamic range without unwanted harmonic coloration.

### Chapters 41 and 42: Accessibility and Keyboard Navigation (pp. 1003-1036)

- Tab key navigation, screen reader accessibility text, full shortcut matrix for Windows and macOS.
- Complete shortcut reference compiled in [KEYBOARD_SHORTCUTS.md](file:///d:/PARTH/FINISHED%20WORK/DEVELOPMENT/LiveCraft/docs/KEYBOARD_SHORTCUTS.md).
