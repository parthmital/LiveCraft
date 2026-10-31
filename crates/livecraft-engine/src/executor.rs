//! The real-time executor and its session-side handle (M05 item 6, TARGET-ARCHITECTURE "Engine
//! Decisions").
//!
//! `Engine` runs on the audio thread. `EngineHandle` runs on the session side. They share only
//! wait-free `rtrb` queues: commands and graphs flow to the audio thread, events flow back, and the
//! graphs the audio thread replaces flow to the collector thread, which frees them. The audio thread
//! applies commands and graph swaps at the start of each driver callback and never allocates.

use crate::collector;
use crate::compiled::{CompiledGraph, InputBlock, MAX_BLOCK, MAX_NOTE_EVENTS, Node, RAMP_STEP};
use crate::midi::{
    HostClock, MidiMessage, MidiOutEvent, note_event, note_out_event, period_us, sample_offset,
};
use crate::snapshot::{ClipState, EngineSnapshot, MAX_NODES, Transport, cpu_load};
use livecraft_devices::{EventList, ProcessContext};
use livecraft_model::ids::{DeviceId, SceneId, TrackId};
use livecraft_model::param::ParamId;
use livecraft_time::beats_per_sample;
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
    SetTempo(f32),
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
    /// Launches a session slot on a track.
    TriggerClip {
        track: TrackId,
        slot: usize,
    },
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
        let engine = Engine {
            graph: Box::new(graph),
            sample_rate,
            commands: command_rx,
            graphs: graph_rx,
            retired: retired_tx,
            events: event_tx,
            bpm: 120.0,
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

    /// Renders `out`, which holds interleaved frames of `channels` samples. It is the only call from
    /// the driver callback. The buffer starts at the engine clock's time now.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        let block_start_us = self.clock.now_us();
        self.process_at(out, channels, block_start_us);
    }

    /// As `process`, for a buffer that starts at `block_start_us` on the engine clock. `process` reads that
    /// time from the clock. Tests pass a time they chose, so the frames of MIDI input are known.
    pub fn process_at(&mut self, out: &mut [f32], channels: usize, block_start_us: u64) {
        if channels == 0 {
            return;
        }
        let started = Instant::now();
        // The first frame of this buffer, and when it starts: the anchor for MIDI output.
        let anchor = (self.frame, block_start_us);
        self.apply_pending();
        let frames = out.len() / channels;
        self.receive_midi(block_start_us, frames);
        let mut start = 0;
        while start < frames {
            self.apply_due_params();
            let block = self.segment_len(frames - start);
            self.push_midi_notes(start, block);
            let context = ProcessContext {
                sample_rate: self.sample_rate,
                frames: block,
                bpm: self.bpm,
                playhead_beats: self.playhead_at(self.frame),
                beats_per_frame: self.beats_per_frame(),
            };
            let input = self
                .input
                .as_mut()
                .map_or(InputBlock::SILENT, |capture| capture.fill(block));
            self.graph.render_with_input(&context, input);
            self.push_midi_out();
            let (left, right) = self.graph.output(block);
            let samples = &mut out[start * channels..(start + block) * channels];
            for (frame, chunk) in samples.chunks_exact_mut(channels).enumerate() {
                let l = sanitize_output_sample(left[frame]);
                let r = sanitize_output_sample(right[frame]);
                if channels == 1 {
                    chunk[0] = (l + r) * 0.5;
                } else {
                    chunk[0] = l;
                    chunk[1] = r;
                    chunk[2..].fill(0.0);
                }
            }
            self.frame += block as u64;
            start += block;
        }
        self.publish(frames, started.elapsed().as_secs_f32(), anchor);
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
        snapshot.transport = Transport {
            playing: self.playing,
            playhead_beats,
            bpm: self.bpm,
        };
        let nodes = self.graph.nodes();
        snapshot.node_count = nodes.len().min(MAX_NODES);
        for (out, node) in snapshot.nodes.iter_mut().zip(nodes) {
            out.meter = node.meter;
            out.clip = ClipState {
                active_slot: node.clips.active_slot,
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
            }
            EngineCommand::SetPlaying(playing) => {
                self.restart_playhead();
                self.playing = playing;
            }
            EngineCommand::SetPlayhead(beats) => {
                self.origin_beats = beats;
                self.origin_frame = self.frame;
            }
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
            EngineCommand::TriggerClip { track, slot } => self.graph.launch_clip(track, slot),
            EngineCommand::StopClip { track } => self.graph.stop_clip(track),
            EngineCommand::TriggerScene { scene } => self.graph.launch_scene(scene),
            EngineCommand::StopAll => self.graph.stop_all(),
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
        let Some(boundary) = self.graph.next_boundary(position) else {
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

/// Replaces NaN and infinity with silence; finite samples pass through bit-exactly.
#[inline]
fn sanitize_output_sample(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

#[cfg(test)]
mod tests;
