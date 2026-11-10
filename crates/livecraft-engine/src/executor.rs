//! The real-time executor and its session-side handle (M05 item 6, TARGET-ARCHITECTURE "Engine
//! Decisions").
//!
//! `Engine` runs on the audio thread. `EngineHandle` runs on the session side. They share only
//! wait-free `rtrb` queues: commands and graphs flow to the audio thread, events flow back, and the
//! graphs the audio thread replaces flow to the collector thread, which frees them. The audio thread
//! applies commands and graph swaps at the start of each driver callback and never allocates.

use crate::clip::PlayableClip;
use crate::collector;
use crate::compiled::{
    CompiledGraph, InputBlock, MAX_BLOCK, MAX_NOTE_EVENTS, Node, RAMP_STEP, StemTarget,
};
use crate::launch::{Fired, FollowTarget, Pcg32, follow_target, pick_action};
use crate::midi::{
    HostClock, MidiMessage, MidiOutEvent, note_event, note_out_event, period_us, sample_offset,
};
use crate::snapshot::{ClipState, EngineSnapshot, MAX_NODES, Transport, cpu_load};
use crate::transport::{MetronomeVoice, Timeline, TransportSettings};
use livecraft_devices::{EventList, ProcessContext};
use livecraft_model::ids::{DeviceId, SceneId, TrackId};
use livecraft_model::model::Quantization;
use livecraft_model::param::ParamId;
use livecraft_time::{Beats, LaunchQuantization, TimeSignatureMap, beats_per_sample, grid_period};
use std::time::Instant;

/// Capacity of the command queue, in commands.
pub const COMMAND_CAPACITY: usize = 1024;
/// Capacity of the graph and retired-graph queues, in graphs.
pub const GRAPH_CAPACITY: usize = 4;
/// Capacity of the event queue, in events.
pub const EVENT_CAPACITY: usize = 256;

/// A change to the running graph or the transport. The audio thread applies it at the start of the
/// next driver callback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineCommand {
    /// A tempo set by hand while playing, as the Control Bar's tempo field does when the Set has tempo
    /// changes: the tempo map is not followed until Back to Arrangement.
    SetTempo(f32),
    /// The Set's one tempo changed. The engine's tempo map takes it in place.
    SetSongTempo(f32),
    SetPlaying(bool),
    SetPlayhead(f64),
    SetTrackVolume {
        track: TrackId,
        volume_db: f32,
    },
    SetTrackPan {
        track: TrackId,
        pan: f32,
    },
    SetTrackMute {
        track: TrackId,
        muted: bool,
    },
    SetTrackSolo {
        track: TrackId,
        soloed: bool,
    },
    SetTrackSend {
        track: TrackId,
        send: usize,
        amount: f32,
    },
    /// Presses the launch button of a Session slot at full velocity (manual 16.2). The clip starts at
    /// its quantized launch time, or at once when the transport is stopped, which starts it.
    TriggerClip {
        track: TrackId,
        slot: usize,
    },
    /// Presses a launch button with a MIDI velocity (manual 16.5).
    PressClip {
        track: TrackId,
        slot: usize,
        velocity: u8,
    },
    /// Releases a launch button, which Gate and Repeat modes act on (manual 16.2).
    ReleaseClip {
        track: TrackId,
        slot: usize,
    },
    /// Jumps within the playing clip by the global quantization period (manual 16.6).
    NudgeClip {
        track: TrackId,
        forward: bool,
    },
    /// Stops every Session clip and lets the tracks play the Arrangement again (manual 3.7, page 42).
    BackToArrangement,
    /// Moves the song position to a beat, at the next global quantization step while playing, as a
    /// locator does (manual 6.4, page 171).
    JumpTo(f64),
    /// The Control Bar settings: global quantization, loop, punch, Follow Actions, and metronome.
    SetTransport(TransportSettings),
    /// The Arrangement Record button (manual 19.3.1). With a count-in, the next start counts in first.
    SetRecording(bool),
    StopClip {
        track: TrackId,
    },
    /// Launches the slots in a scene's row.
    TriggerScene {
        scene: SceneId,
    },
    StopAll,
    /// Sets a parameter of a device on a track, by its id in the device table. The change takes
    /// effect `offset` frames after the first frame of the next driver buffer.
    SetDeviceParam {
        track: TrackId,
        device: DeviceId,
        param: ParamId,
        value: f32,
        offset: u32,
    },
}

/// A parameter change waiting for its frame. `due` is an absolute frame of the engine.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingParam {
    due: u64,
    track: TrackId,
    device: DeviceId,
    param: ParamId,
    value: f32,
}

/// A discrete event from the audio thread, read by the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineEvent {
    /// A submitted graph is running. Its devices have taken over the state of the graph before it.
    GraphApplied,
}

/// A queue was full, so the change was not sent. The caller may retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the engine queue is full")]
pub struct QueueFull;

/// Why a graph was not submitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SubmitError {
    #[error(transparent)]
    Queue(#[from] QueueFull),
    #[error("the graph has {nodes} nodes, and the engine publishes at most {max}")]
    TooManyNodes { nodes: usize, max: usize },
}

/// The session side: sends commands and graphs, reads events, and reads the engine snapshot.
pub struct EngineHandle {
    commands: rtrb::Producer<EngineCommand>,
    graphs: rtrb::Producer<Box<CompiledGraph>>,
    events: rtrb::Consumer<EngineEvent>,
    snapshots: triple_buffer::Output<EngineSnapshot>,
}

impl EngineHandle {
    /// Sends a command. Commands apply in the order they were sent.
    pub fn send(&mut self, command: EngineCommand) -> Result<(), QueueFull> {
        self.commands.push(command).map_err(|_| QueueFull)
    }

    /// Submits a graph built by `GraphCompiler`, in the order it was compiled. The graph replaces
    /// the running one at the next block boundary.
    pub fn submit_graph(&mut self, graph: CompiledGraph) -> Result<(), SubmitError> {
        if graph.nodes().len() > MAX_NODES {
            return Err(SubmitError::TooManyNodes {
                nodes: graph.nodes().len(),
                max: MAX_NODES,
            });
        }
        self.graphs.push(Box::new(graph)).map_err(|_| QueueFull)?;
        Ok(())
    }

    /// The newest snapshot the audio thread has published.
    pub fn snapshot(&mut self) -> &EngineSnapshot {
        self.snapshots.read()
    }

    /// Appends the events the audio thread has sent since the last call.
    pub fn drain_events(&mut self, out: &mut Vec<EngineEvent>) {
        while let Ok(event) = self.events.pop() {
            out.push(event);
        }
    }
}

/// The audio side. The driver callback calls `process`, and nothing else on this struct runs on
/// another thread.
pub struct Engine {
    graph: Box<CompiledGraph>,
    sample_rate: f32,
    commands: rtrb::Consumer<EngineCommand>,
    graphs: rtrb::Consumer<Box<CompiledGraph>>,
    retired: rtrb::Producer<Box<CompiledGraph>>,
    events: rtrb::Producer<EngineEvent>,
    bpm: f32,
    playing: bool,
    /// The playhead in beats at `origin_frame`. Positions are computed from it, so they do not
    /// drift over a long play.
    origin_beats: f64,
    origin_frame: u64,
    /// Frames rendered since the engine started.
    frame: u64,
    /// Parameter changes that take effect at a later frame, in the order they were sent.
    pending: Vec<PendingParam>,
    /// Publishes the state after each callback. The session reads it through `EngineHandle`.
    snapshots: triple_buffer::Input<EngineSnapshot>,
    /// The external input the driver captures, when it captures any.
    input: Option<InputCapture>,
    /// The MIDI input the driver feeds, when the session opened MIDI ports.
    midi: Option<MidiCapture>,
    /// The MIDI output the engine feeds, when the session opened MIDI output ports.
    midi_out: Option<MidiOutCapture>,
    /// The clock the buffers start on. `set_midi_input` replaces it with the MIDI input's clock.
    clock: HostClock,
    /// The launch clock minus the song position (`launch`). It changes only when the song position jumps,
    /// so launched clips keep their sync through loops and jumps.
    clock_offset: f64,
    /// True after a tempo change by hand or by a scene, until Back to Arrangement: the tempo map is not
    /// followed meanwhile.
    tempo_override: bool,
    /// The scene whose time signature replaced the Set's, after it launched.
    scene_signature: Option<usize>,
    settings: TransportSettings,
    /// Picks Follow Actions. It is seeded from the Set, so a render repeats.
    rng: Pcg32,
    /// A launched scene's tempo and signature, waiting for the scene's launch time on the launch clock.
    pending_scene: Option<(f64, usize)>,
    /// A song position jump waiting for its launch clock time: (time, target beat).
    pending_jump: Option<(f64, f64)>,
    /// The song position at the start of the last segment, to see the Arrangement Loop's end crossed.
    last_song: f64,
    recording: bool,
    /// The song position where the count-in ends and recording starts (manual 19.6).
    count_in_end: Option<f64>,
    metronome: MetronomeVoice,
    click_left: Box<[f32]>,
    click_right: Box<[f32]>,
    stem_target: StemTarget,
}

/// The external input the driver captures: the samples its input stream pushed, and the block being
/// rendered.
struct InputCapture {
    samples: rtrb::Consumer<f32>,
    channels: usize,
    /// The samples of the current block, at most `MAX_BLOCK` frames. `Engine::set_input` allocates it,
    /// so `fill` allocates nothing.
    block: Box<[f32]>,
}

/// The MIDI input the driver feeds, and the note events of the buffer being rendered.
struct MidiCapture {
    /// One queue per open input port.
    messages: Vec<rtrb::Consumer<MidiMessage>>,
    /// The note events of the buffer, at their offsets in the buffer and in offset order. It is
    /// allocated with room for `MAX_NOTE_EVENTS` in `Engine::set_midi_input`, so filling it allocates
    /// nothing.
    notes: EventList,
}

/// The MIDI output the engine feeds: the queue to the MIDI output thread, and the anchor that maps a
/// buffer's frames to host time. The anchor is None until the first buffer.
struct MidiOutCapture {
    events: rtrb::Producer<MidiOutEvent>,
    anchor: triple_buffer::Input<Option<(u64, u64)>>,
}

impl InputCapture {
    /// Takes the next `frames` frames of input into the block and returns it. A sample the stream has
    /// not delivered reads as silence, so an underrun is silent rather than stale.
    fn fill(&mut self, frames: usize) -> InputBlock<'_> {
        let len = frames * self.channels;
        for sample in &mut self.block[..len] {
            *sample = self.samples.pop().unwrap_or(0.0);
        }
        InputBlock {
            samples: &self.block[..len],
            channels: self.channels,
        }
    }
}

impl Engine {
    /// Creates the audio side and the session handle around the first graph, and starts the
    /// collector thread.
    /// `shared` is the collector behind the handle the compiler shares clip data with. It moves to
    /// the collector thread.
    pub fn start(
        graph: CompiledGraph,
        sample_rate: f32,
        shared: basedrop::Collector,
    ) -> std::io::Result<(Engine, EngineHandle)> {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(COMMAND_CAPACITY);
        let (graph_tx, graph_rx) = rtrb::RingBuffer::new(GRAPH_CAPACITY);
        let (retired_tx, retired_rx) = rtrb::RingBuffer::new(GRAPH_CAPACITY);
        let (event_tx, event_rx) = rtrb::RingBuffer::new(EVENT_CAPACITY);
        if graph.nodes().len() > MAX_NODES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "the graph has {} nodes, and the engine publishes at most {MAX_NODES}",
                    graph.nodes().len()
                ),
            ));
        }
        let (snapshot_tx, snapshot_rx) = triple_buffer::triple_buffer(&EngineSnapshot::default());
        collector::spawn(retired_rx, shared)?;
        let settings = graph.timeline.transport;
        let rng = Pcg32::new(graph.timeline.follow_seed);
        let bpm = graph.timeline.tempo.bpm_at(Beats(0.0)) as f32;
        let engine = Engine {
            graph: Box::new(graph),
            sample_rate,
            commands: command_rx,
            graphs: graph_rx,
            retired: retired_tx,
            events: event_tx,
            bpm,
            playing: false,
            origin_beats: 0.0,
            origin_frame: 0,
            frame: 0,
            pending: Vec::with_capacity(COMMAND_CAPACITY),
            snapshots: snapshot_tx,
            input: None,
            midi: None,
            midi_out: None,
            clock: HostClock::start(),
            clock_offset: 0.0,
            tempo_override: false,
            scene_signature: None,
            settings,
            rng,
            pending_scene: None,
            pending_jump: None,
            last_song: 0.0,
            recording: false,
            count_in_end: None,
            metronome: MetronomeVoice::default(),
            click_left: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            click_right: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            stem_target: StemTarget::Main,
        };
        let handle = EngineHandle {
            commands: command_tx,
            graphs: graph_tx,
            events: event_rx,
            snapshots: snapshot_rx,
        };
        Ok((engine, handle))
    }

    /// Reads the external input from `samples`, which holds interleaved frames of `channels` samples
    /// as the driver's input stream pushes them. Call it before the engine starts on a driver. The
    /// block buffer is allocated here, not on the audio thread.
    pub fn set_input(&mut self, samples: rtrb::Consumer<f32>, channels: usize) {
        self.input = Some(InputCapture {
            samples,
            channels,
            block: vec![0.0; MAX_BLOCK * channels].into_boxed_slice(),
        });
    }

    /// Reads MIDI input from the port queues in `messages`, whose messages are stamped on `clock`. In each
    /// buffer, the note on and note off messages that arrived during the previous buffer period play in this
    /// buffer, on their frames, in every MIDI track that takes MIDI input (`Node::monitors_midi`). The engine
    /// reads `clock` at the start of each buffer from now on, so the MIDI threads and the buffers share one
    /// time origin. Call it before the engine starts on a driver. The note list is allocated here.
    pub fn set_midi_input(&mut self, messages: Vec<rtrb::Consumer<MidiMessage>>, clock: HostClock) {
        self.clock = clock;
        self.midi = Some(MidiCapture {
            messages,
            notes: EventList::with_capacity(MAX_NOTE_EVENTS),
        });
    }

    /// Sends the clip notes of the MIDI tracks routed to an external port into `events`, each stamped with
    /// the engine frame at which it plays, and publishes the anchor the MIDI output thread maps frames to host
    /// time with. Call it before the engine starts on a driver.
    pub fn set_midi_output(
        &mut self,
        events: rtrb::Producer<MidiOutEvent>,
        anchor: triple_buffer::Input<Option<(u64, u64)>>,
    ) {
        self.midi_out = Some(MidiOutCapture { events, anchor });
    }

    /// Selects which stem path the running graph isolates. Passing `None` resets to `StemTarget::Main`.
    pub fn set_stem_target(&mut self, target: Option<StemTarget>) {
        let target = target.unwrap_or(StemTarget::Main);
        self.stem_target = target;
        self.graph.set_stem_target(target);
    }

    /// Maximum effect tail length in samples for the active graph and stem target.
    pub fn tail_samples(&self) -> u64 {
        self.graph.max_tail_samples()
    }

    /// Renders `out`, which holds interleaved frames of `channels` samples. It is the only call from
    /// the driver callback. The buffer starts at the engine clock's time now.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        let block_start_us = self.clock.now_us();
        self.process_at(out, channels, block_start_us);
    }

    /// Deterministic offline render path for `VirtualDriver` and `OfflineBouncer`.
    /// Disables the metronome and derives `block_start_us` from the sample frame count rather than
    /// a wall clock, then runs the same core block processor as `process_at`.
    pub fn process_offline(&mut self, out: &mut [f32], channels: usize) {
        let sr = u128::from((self.sample_rate.max(1.0)) as u64);
        let block_start_us = (u128::from(self.frame) * 1_000_000 / sr) as u64;
        self.process_at_inner(out, channels, block_start_us, false, true);
    }

    /// As `process`, for a buffer that starts at `block_start_us` on the engine clock. `process` reads that
    /// time from the clock. Tests pass a time they chose, so the frames of MIDI input are known.
    pub fn process_at(&mut self, out: &mut [f32], channels: usize, block_start_us: u64) {
        self.process_at_inner(out, channels, block_start_us, true, false);
    }

    fn process_at_inner(
        &mut self,
        out: &mut [f32],
        channels: usize,
        block_start_us: u64,
        measure_cpu: bool,
        suppress_metronome: bool,
    ) {
        if channels == 0 {
            return;
        }
        let started = measure_cpu.then(Instant::now);
        // The first frame of this buffer, and when it starts: the anchor for MIDI output.
        let anchor = (self.frame, block_start_us);
        self.apply_pending();
        let frames = out.len() / channels;
        self.receive_midi(block_start_us, frames);
        let mut start = 0;
        while start < frames {
            self.apply_due_params();
            self.advance_transport();
            self.fire_launches();
            let block = self.segment_len(frames - start);
            self.push_midi_notes(start, block);
            let song = self.playhead_at(self.frame);
            let dt = self.beats_per_frame();
            let counting_in = self.count_in_end.is_some();
            let context = ProcessContext {
                sample_rate: self.sample_rate,
                frames: block,
                bpm: self.bpm,
                playhead_beats: song,
                // During a count-in the clips stay silent, as when the transport is stopped.
                beats_per_frame: if counting_in { 0.0 } else { dt },
            };
            let input = self
                .input
                .as_mut()
                .map_or(InputBlock::SILENT, |capture| capture.fill(block));
            self.graph.clock = song + self.clock_offset;
            self.graph.render_with_input(&context, input);
            self.push_midi_out();
            if suppress_metronome {
                self.click_left[..block].fill(0.0);
                self.click_right[..block].fill(0.0);
                self.metronome = MetronomeVoice::default();
            } else {
                self.render_metronome(song, dt, block);
            }
            let (left, right) = self.graph.output(block);
            let samples = &mut out[start * channels..(start + block) * channels];
            for (frame, chunk) in samples.chunks_exact_mut(channels).enumerate() {
                let l = sanitize_output_sample(left[frame] + self.click_left[frame]);
                let r = sanitize_output_sample(right[frame] + self.click_right[frame]);
                if channels == 1 {
                    chunk[0] = (l + r) * 0.5;
                } else {
                    chunk[0] = l;
                    chunk[1] = r;
                    chunk[2..].fill(0.0);
                }
            }
            self.last_song = song;
            self.frame += block as u64;
            start += block;
        }
        let elapsed = started.map_or(0.0, |t| t.elapsed().as_secs_f32());
        self.publish(frames, elapsed, anchor);
    }

    /// Pops the MIDI messages that arrived and turns the note on and note off ones into note events at
    /// their offsets in this buffer (`sample_offset`). The list is sorted so that a note off comes before
    /// a note on at the same offset (`EventList::sort_by_offset`). Notes beyond its capacity are dropped.
    fn receive_midi(&mut self, block_start_us: u64, frames: usize) {
        let Some(midi) = self.midi.as_mut() else {
            return;
        };
        midi.notes.clear();
        let period = period_us(frames, self.sample_rate);
        let frequency = self.graph.note_frequencies();
        for queue in midi.messages.iter_mut() {
            while let Ok(message) = queue.pop() {
                let offset = sample_offset(
                    message.time_us,
                    block_start_us,
                    period,
                    frames,
                    self.sample_rate,
                );
                if let Some(event) = note_event(&message, offset as u32, frequency) {
                    let _ = midi.notes.push(event);
                }
            }
        }
        midi.notes.sort_by_offset();
    }

    /// Hands the MIDI note events that fall in the segment `start..start + block` to the tracks that take
    /// MIDI input, at their offsets within the segment.
    fn push_midi_notes(&mut self, start: usize, block: usize) {
        let Some(midi) = self.midi.as_ref() else {
            return;
        };
        for event in midi.notes.iter() {
            let offset = event.offset() as usize;
            if (start..start + block).contains(&offset) {
                self.graph.push_live_note(event.at((offset - start) as u32));
            }
        }
    }

    /// Copies the clip notes of the MIDI tracks routed to an external port into the MIDI output queue, each at
    /// its frame in the engine. The segment just rendered starts at `self.frame`. A full queue drops the note,
    /// because the audio thread never waits.
    fn push_midi_out(&mut self) {
        let Some(midi) = self.midi_out.as_mut() else {
            return;
        };
        for node in self.graph.nodes() {
            let Some(route) = node.midi_out else {
                continue;
            };
            for event in node.midi_out_notes.iter() {
                let frame = self.frame + u64::from(event.offset());
                let _ = midi
                    .events
                    .push(note_out_event(frame, route.port, route.channel, event));
            }
        }
    }

    /// Writes this callback's state into the snapshot and publishes it, and publishes the anchor the
    /// callback started on to the MIDI output thread. Only fixed-size values are written, so nothing
    /// allocates or locks here.
    fn publish(&mut self, frames: usize, elapsed_seconds: f32, anchor: (u64, u64)) {
        let playhead_beats = self.playhead_at(self.frame);
        let snapshot = self.snapshots.input_buffer_mut();
        let tracks = self.graph.track_count();
        snapshot.transport = Transport {
            playing: self.playing,
            playhead_beats,
            bpm: self.bpm,
            recording: self.recording,
            counting_in: self.count_in_end.is_some(),
            back_to_arrangement: self.graph.nodes()[..tracks]
                .iter()
                .any(|node| node.clips.launcher.left_arrangement),
        };
        let nodes = self.graph.nodes();
        snapshot.node_count = nodes.len().min(MAX_NODES);
        for (out, node) in snapshot.nodes.iter_mut().zip(nodes) {
            out.meter = node.meter;
            out.clip = ClipState {
                active_slot: node.clips.active_slot(),
                queued_slot: node.clips.launcher.queued_slot(),
                playing: node.clips.playing,
            };
        }
        snapshot.output = self.graph.output_meter;
        snapshot.cpu_load = cpu_load(elapsed_seconds, frames, self.sample_rate);
        snapshot.frames = self.frame;
        self.snapshots.publish();
        if let Some(midi) = self.midi_out.as_mut() {
            midi.anchor.write(Some(anchor));
        }
    }

    /// Applies the queued commands, then swaps in the newest submitted graph if there is room to
    /// retire the one it replaces.
    fn apply_pending(&mut self) {
        while let Ok(command) = self.commands.pop() {
            self.apply(command);
        }
        if self.retired.slots() == 0 {
            return;
        }
        if let Ok(mut next) = self.graphs.pop() {
            next.adopt_state(&mut self.graph);
            next.clock = self.graph.clock;
            next.set_stem_target(self.stem_target);
            self.settings = next.timeline.transport;
            if self
                .scene_signature
                .is_some_and(|row| row >= next.timeline.scenes.len())
            {
                self.scene_signature = None;
            }
            let previous = std::mem::replace(&mut self.graph, next);
            // Room was checked above, so the push cannot fail and nothing is freed here.
            let _ = self.retired.push(previous);
            // A full event queue drops the event: the audio thread never waits.
            let _ = self.events.push(EngineEvent::GraphApplied);
        }
    }

    fn apply(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::SetTempo(bpm) => {
                self.restart_playhead();
                self.bpm = bpm;
                self.tempo_override = true;
            }
            EngineCommand::SetSongTempo(bpm) => {
                if self.graph.timeline.tempo.set_constant_bpm(f64::from(bpm))
                    && !self.tempo_override
                {
                    self.restart_playhead();
                    self.bpm = bpm;
                } else if !self.graph.timeline.tempo.is_constant() {
                    self.restart_playhead();
                    self.bpm = bpm;
                    self.tempo_override = true;
                }
            }
            EngineCommand::SetPlaying(playing) => self.set_playing(playing),
            EngineCommand::SetPlayhead(beats) => self.jump_song(beats),
            EngineCommand::PressClip {
                track,
                slot,
                velocity,
            } => self.press(track, slot, velocity),
            EngineCommand::ReleaseClip { track, slot } => self.release(track, slot),
            EngineCommand::NudgeClip { track, forward } => self.nudge(track, forward),
            EngineCommand::BackToArrangement => self.back_to_arrangement(),
            EngineCommand::JumpTo(beats) => {
                if self.playing {
                    let at = self.launch_time(LaunchQuantization::Global);
                    self.pending_jump = Some((at, beats));
                } else {
                    self.jump_song(beats);
                }
            }
            EngineCommand::SetTransport(settings) => self.settings = settings,
            EngineCommand::SetRecording(recording) => self.recording = recording,
            EngineCommand::SetTrackVolume { track, volume_db } => {
                if let Some(node) = self.node(track) {
                    node.mixer.volume_db = volume_db;
                }
            }
            EngineCommand::SetTrackPan { track, pan } => {
                if let Some(node) = self.node(track) {
                    node.mixer.pan = pan;
                }
            }
            EngineCommand::SetTrackMute { track, muted } => {
                if let Some(node) = self.node(track) {
                    node.mixer.is_muted = muted;
                }
            }
            EngineCommand::SetTrackSolo { track, soloed } => {
                if let Some(node) = self.node(track) {
                    node.mixer.is_soloed = soloed;
                }
            }
            EngineCommand::SetTrackSend {
                track,
                send,
                amount,
            } => {
                if let Some(level) = self
                    .node(track)
                    .and_then(|node| node.mixer.sends.get_mut(send))
                {
                    *level = amount;
                }
            }
            EngineCommand::TriggerClip { track, slot } => self.press(track, slot, 127),
            EngineCommand::StopClip { track } => self.stop_tracks(Some(track)),
            EngineCommand::TriggerScene { scene } => self.launch_scene(scene),
            EngineCommand::StopAll => self.stop_tracks(None),
            EngineCommand::SetDeviceParam {
                track,
                device,
                param,
                value,
                offset,
            } => {
                let event = PendingParam {
                    due: self.frame + u64::from(offset),
                    track,
                    device,
                    param,
                    value,
                };
                // A full queue applies the change at once rather than dropping it.
                if self.pending.len() < self.pending.capacity() {
                    self.pending.push(event);
                } else {
                    self.graph.set_device_param(
                        event.track,
                        event.device,
                        event.param,
                        event.value,
                    );
                }
            }
        }
    }

    /// Applies, in the order they were sent, the parameter changes whose frame has come.
    fn apply_due_params(&mut self) {
        let mut index = 0;
        while index < self.pending.len() {
            if self.pending[index].due <= self.frame {
                let event = self.pending.remove(index);
                self.graph
                    .set_device_param(event.track, event.device, event.param, event.value);
            } else {
                index += 1;
            }
        }
    }

    fn node(&mut self, track: TrackId) -> Option<&mut Node> {
        self.graph.node_mut(track)
    }

    /// Keeps the playhead where it is when the tempo or the transport changes.
    fn restart_playhead(&mut self) {
        self.origin_beats = self.playhead_at(self.frame);
        self.origin_frame = self.frame;
    }

    /// Frames to render from the current frame, at most `remaining` and `MAX_BLOCK`. A segment ends on
    /// the frame where the playhead reaches the next arrangement clip boundary, so the clip starts or
    /// stops on its exact sample.
    fn segment_len(&self, remaining: usize) -> usize {
        let mut limit = remaining.min(MAX_BLOCK);
        if self.graph.ramping() {
            limit = limit.min(RAMP_STEP);
        }
        if let Some(due) = self
            .pending
            .iter()
            .map(|event| event.due)
            .filter(|due| *due > self.frame)
            .min()
        {
            limit = limit.min((due - self.frame) as usize);
        }
        if !self.playing {
            return limit;
        }
        let position = self.playhead_at(self.frame);
        if !self.tempo_override && self.graph.timeline.tempo.ramps_at(Beats(position)) {
            limit = limit.min(RAMP_STEP);
        }
        let Some(boundary) = self.next_song_event(position) else {
            return limit;
        };
        let estimate = ((boundary - position) / self.beats_per_frame()).ceil();
        if !estimate.is_finite() || estimate >= limit as f64 {
            return limit;
        }
        // The estimate can be one frame off through rounding, so step to the first frame whose
        // playhead has reached the boundary.
        let mut frames = estimate.max(1.0) as u64;
        while frames > 1 && self.playhead_at(self.frame + frames - 1) >= boundary {
            frames -= 1;
        }
        while self.playhead_at(self.frame + frames) < boundary {
            frames += 1;
        }
        (frames as usize).min(limit)
    }

    /// The first song position after `position` at which something must start on its sample: an
    /// Arrangement clip boundary, a launch, stop, Follow Action, or Repeat, a scene's tempo, a jump, a tempo
    /// change, the end of the Arrangement Loop, or the end of the count-in.
    fn next_song_event(&self, position: f64) -> Option<f64> {
        let clock = position + self.clock_offset;
        let follow = self.settings.follow_actions_enabled;
        let tracks = self.graph.track_count();
        let launches = self.graph.nodes()[..tracks]
            .iter()
            .filter_map(|node| node.clips.launcher.next_event(clock, follow))
            .chain(self.pending_scene.map(|(at, _)| at))
            .chain(self.pending_jump.map(|(at, _)| at))
            .filter(|at| *at > clock)
            .map(|at| at - self.clock_offset);
        let tempo = (!self.tempo_override)
            .then(|| self.graph.timeline.tempo.next_change_after(Beats(position)))
            .flatten()
            .map(|beats| beats.0);
        let loop_end = (self.settings.loop_enabled
            && self.count_in_end.is_none()
            && position < self.settings.loop_end())
        .then(|| self.settings.loop_end());
        launches
            .chain(self.graph.next_boundary(position))
            .chain(tempo)
            .chain(loop_end)
            .chain(self.count_in_end)
            .filter(|at| *at > position)
            .reduce(f64::min)
    }

    /// The time signature in effect: a launched scene's, or the Set's.
    fn signature(&self) -> &TimeSignatureMap {
        signature_of(&self.graph.timeline, self.scene_signature)
    }

    /// The launch clock time of the next step of `quantization` from now, with `Global` read from the
    /// Control Bar (manual 16.4). While the transport is stopped, now.
    fn launch_time(&self, quantization: LaunchQuantization) -> f64 {
        let song = self.playhead_at(self.frame);
        quantized_clock(
            song,
            song + self.clock_offset,
            quantization,
            self.settings.global_quantization,
            self.signature(),
            self.playing,
        )
    }

    /// Moves the song position to `beats` now, keeping the launch clock running on.
    fn jump_song(&mut self, beats: f64) {
        let song = self.playhead_at(self.frame);
        self.clock_offset += song - beats;
        self.origin_beats = beats;
        self.origin_frame = self.frame;
        self.last_song = beats;
    }

    /// Starts or stops the transport (manual 6.3). Stopping stops every Session clip. Starting while the
    /// Arrangement Record button is on, with a count-in set, first plays the count-in bars before the
    /// start position (manual 19.6, page 420).
    fn set_playing(&mut self, playing: bool) {
        self.restart_playhead();
        if playing && !self.playing {
            let bars = self.settings.metronome.count_in_bars;
            if self.recording && bars > 0 {
                let start = self.playhead_at(self.frame);
                let (numerator, denominator) = self.signature().signature_at(Beats(start));
                let bar = 4.0 * f64::from(numerator) / f64::from(denominator.max(1));
                self.jump_song(start - f64::from(bars) * bar);
                self.count_in_end = Some(start);
            }
        }
        if !playing {
            for node in self.graph.tracks_mut() {
                node.clips.launcher.halt();
                node.clips.changed();
            }
            self.count_in_end = None;
            self.pending_scene = None;
            self.pending_jump = None;
        }
        self.playing = playing;
    }

    /// Starts the transport for a launch while it is stopped, so the clip starts now.
    fn start_for_launch(&mut self) {
        if !self.playing {
            self.restart_playhead();
            self.playing = true;
        }
    }

    /// A down on a launch button (manual 16.2). An empty slot with a Clip Stop button stops the track.
    fn press(&mut self, track: TrackId, slot: usize, velocity: u8) {
        self.start_for_launch();
        let song = self.playhead_at(self.frame);
        let clock = song + self.clock_offset;
        let global = self.settings.global_quantization;
        let (timeline, tracks) = self.graph.split_timeline();
        let signature = signature_of(timeline, self.scene_signature);
        let Some(node) = tracks.iter_mut().find(|node| node.track == track) else {
            return;
        };
        let Some(info) = node.clips.info(slot) else {
            if node.clips.stop_buttons.get(slot).copied().unwrap_or(false) {
                let at = quantized_clock(
                    song,
                    clock,
                    LaunchQuantization::Global,
                    global,
                    signature,
                    true,
                );
                node.clips.launcher.stop(at);
            }
            return;
        };
        let quantization = info.launch.quantization.to_launch();
        let at = quantized_clock(song, clock, quantization, global, signature, true);
        let grid = match quantization.resolve(global.grid()) {
            LaunchQuantization::Grid(grid) => Some(grid),
            _ => None,
        };
        let period = grid.and_then(|grid| grid_period(Beats(song), grid, signature));
        node.clips.launcher.press(slot, &info, velocity, at, period);
    }

    /// An up on a launch button (manual 16.2).
    fn release(&mut self, track: TrackId, slot: usize) {
        let song = self.playhead_at(self.frame);
        let clock = song + self.clock_offset;
        let global = self.settings.global_quantization;
        let playing = self.playing;
        let (timeline, tracks) = self.graph.split_timeline();
        let signature = signature_of(timeline, self.scene_signature);
        let Some(node) = tracks.iter_mut().find(|node| node.track == track) else {
            return;
        };
        let Some(info) = node.clips.info(slot) else {
            return;
        };
        let quantization = info.launch.quantization.to_launch();
        let at = quantized_clock(song, clock, quantization, global, signature, playing);
        node.clips.launcher.release(slot, &info, at);
    }

    /// Stops one track's clip, or every track's, at the next global quantization step (the Clip Stop
    /// buttons and Stop All Clips).
    fn stop_tracks(&mut self, track: Option<TrackId>) {
        let at = self.launch_time(LaunchQuantization::Global);
        for node in self.graph.tracks_mut() {
            if track.is_none_or(|track| node.track == track) {
                node.clips.launcher.stop(at);
            }
        }
    }

    /// Launches every clip in a scene's row at the next global quantization step, presses the Clip Stop
    /// button of each empty slot that has one, and applies the scene's tempo and time signature then.
    fn launch_scene(&mut self, scene: SceneId) {
        let Some(row) = self.graph.scene_row(scene) else {
            return;
        };
        self.start_for_launch();
        let at = self.launch_time(LaunchQuantization::Global);
        for node in self.graph.tracks_mut() {
            let lane = &mut node.clips;
            if lane.info(row).is_some() {
                lane.launcher.follow_launch(row, at);
            } else if lane.stop_buttons.get(row).copied().unwrap_or(false) {
                lane.launcher.stop(at);
            }
        }
        self.pending_scene = Some((at, row));
    }

    /// Jumps within a track's playing clip by the global quantization period (manual 16.6).
    fn nudge(&mut self, track: TrackId, forward: bool) {
        let song = self.playhead_at(self.frame);
        let grid = self.settings.global_quantization.grid();
        let Some(period) = grid_period(Beats(song), grid, self.signature()) else {
            return;
        };
        let Some(node) = self.graph.node_mut(track) else {
            return;
        };
        let Some(info) = node
            .clips
            .active_slot()
            .and_then(|slot| node.clips.info(slot))
        else {
            return;
        };
        node.clips
            .launcher
            .nudge(if forward { period } else { -period }, &info);
        node.clips.changed();
    }

    /// Back to Arrangement: every track plays the Arrangement again, and the tempo and time signature
    /// follow the Set again (manual 3.7, page 42).
    fn back_to_arrangement(&mut self) {
        for node in self.graph.tracks_mut() {
            node.clips.launcher.halt();
            node.clips.launcher.left_arrangement = false;
            node.clips.changed();
        }
        self.tempo_override = false;
        self.scene_signature = None;
    }

    /// Applies what the song position has reached: the end of the count-in, the end of the Arrangement
    /// Loop, a quantized jump, a launched scene's tempo and signature, and the tempo map.
    fn advance_transport(&mut self) {
        if !self.playing {
            return;
        }
        let epsilon = (0.5 * self.beats_per_frame()).max(1e-9);
        let song = self.playhead_at(self.frame);
        if let Some(end) = self.count_in_end
            && song >= end - epsilon
        {
            self.count_in_end = None;
        }
        // "To repeatedly play a section of the Arrangement, activate the Arrangement Loop" (manual 6.6).
        let end = self.settings.loop_end();
        if self.settings.loop_enabled
            && self.count_in_end.is_none()
            && self.settings.loop_length > 0.0
            && self.last_song < end - epsilon
            && song >= end - epsilon
        {
            self.jump_song(self.settings.loop_start);
        }
        let clock = self.playhead_at(self.frame) + self.clock_offset;
        if let Some((at, target)) = self.pending_jump
            && at <= clock + epsilon
        {
            self.pending_jump = None;
            self.jump_song(target);
        }
        if let Some((at, row)) = self.pending_scene
            && at <= clock + epsilon
        {
            self.pending_scene = None;
            let scene = self.graph.timeline.scenes.get(row);
            let (tempo, signature) = scene.map_or((None, false), |scene| {
                (scene.tempo, scene.signature.is_some())
            });
            if let Some(tempo) = tempo {
                self.restart_playhead();
                self.bpm = tempo;
                self.tempo_override = true;
            }
            if signature {
                self.scene_signature = Some(row);
            }
        }
        if !self.tempo_override {
            let song = self.playhead_at(self.frame);
            let bpm = self.graph.timeline.tempo.bpm_at(Beats(song)) as f32;
            if bpm != self.bpm {
                self.restart_playhead();
                self.bpm = bpm;
            }
        }
    }

    /// Does every launch, stop, Repeat, clip end, and Follow Action that is due now, on every track
    /// (manual 16.2 to 16.7).
    fn fire_launches(&mut self) {
        if !self.playing || self.count_in_end.is_some() {
            return;
        }
        let epsilon = (0.5 * self.beats_per_frame()).max(1e-9);
        let song = self.playhead_at(self.frame);
        let clock = song + self.clock_offset;
        let follow_enabled = self.settings.follow_actions_enabled;
        let global = self.settings.global_quantization;
        let scene_signature = self.scene_signature;
        let rng = &mut self.rng;
        let (timeline, tracks) = self.graph.split_timeline();
        let signature = signature_of(timeline, scene_signature);
        for node in tracks.iter_mut() {
            let lane = &mut node.clips;
            loop {
                let slots = &lane.slots;
                let fired = lane.launcher.fire(clock, epsilon, follow_enabled, |slot| {
                    slots
                        .get(slot)
                        .and_then(Option::as_ref)
                        .map(PlayableClip::info)
                });
                match fired {
                    None => break,
                    Some(Fired::FollowDue(slot)) => {
                        let Some(info) = lane.info(slot) else {
                            continue;
                        };
                        let action = pick_action(&info.follow, rng);
                        match follow_target(action, slot, &lane.filled, rng) {
                            FollowTarget::Nothing => {}
                            FollowTarget::Stop => lane.launcher.stop(clock),
                            FollowTarget::Launch(target) => {
                                // "Follow Actions circumvent global quantization but not clip quantization."
                                // (manual 16.7, page 367)
                                let quantization =
                                    lane.info(target).map_or(Quantization::Global, |target| {
                                        target.launch.quantization
                                    });
                                let at = match quantization {
                                    Quantization::None | Quantization::Global => clock,
                                    other => quantized_clock(
                                        song,
                                        clock,
                                        other.to_launch(),
                                        global,
                                        signature,
                                        true,
                                    ),
                                };
                                lane.launcher.follow_launch(target, at);
                            }
                        }
                    }
                    Some(_) => lane.changed(),
                }
            }
        }
    }

    /// Renders the metronome for a segment into the click buffers: while playing with the metronome on
    /// (only while recording, after the punch-in point, when so set), and always during a count-in
    /// (manual 19.4.1 and 19.6).
    fn render_metronome(&mut self, song: f64, dt: f64, block: usize) {
        let settings = self.settings;
        let counting_in = self.count_in_end.is_some();
        let audible = self.playing
            && (counting_in
                || (settings.metronome_enabled
                    && (!settings.metronome.only_while_recording
                        || (self.recording
                            && !(settings.punch_in && song < settings.loop_start)))));
        let left = &mut self.click_left[..block];
        let right = &mut self.click_right[..block];
        left.fill(0.0);
        right.fill(0.0);
        let signature = signature_of(&self.graph.timeline, self.scene_signature);
        self.metronome.render(
            &settings.metronome,
            signature,
            song,
            dt,
            self.sample_rate,
            audible,
            left,
            right,
        );
    }

    fn beats_per_frame(&self) -> f64 {
        if self.playing {
            beats_per_sample(f64::from(self.bpm), f64::from(self.sample_rate))
        } else {
            0.0
        }
    }

    /// The playhead in beats at a frame.
    fn playhead_at(&self, frame: u64) -> f64 {
        self.origin_beats + (frame - self.origin_frame) as f64 * self.beats_per_frame()
    }
}

/// The time signature in effect: the launched scene's at `row`, or the Set's.
fn signature_of(timeline: &Timeline, row: Option<usize>) -> &TimeSignatureMap {
    row.and_then(|row| timeline.scenes.get(row))
        .and_then(|scene| scene.signature.as_ref())
        .unwrap_or(&timeline.signature)
}

/// The launch clock time of the next step of `quantization` after the song position `song`, whose
/// launch clock time is `clock`. `Global` takes the Control Bar's setting. While stopped, `clock`.
fn quantized_clock(
    song: f64,
    clock: f64,
    quantization: LaunchQuantization,
    global: Quantization,
    signature: &TimeSignatureMap,
    playing: bool,
) -> f64 {
    if !playing {
        return clock;
    }
    let resolved = quantization.resolve(global.grid());
    let target = livecraft_time::quantize_launch(Beats(song), resolved, signature).0;
    clock + (target - song).max(0.0)
}

/// Replaces NaN and infinity with silence; finite samples pass through bit-exactly.
#[inline]
fn sanitize_output_sample(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

#[cfg(test)]
mod tests;
