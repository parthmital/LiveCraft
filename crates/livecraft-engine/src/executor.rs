//! The real-time executor and its session-side handle (M05 item 6, TARGET-ARCHITECTURE "Engine
//! Decisions").
//!
//! `Engine` runs on the audio thread. `EngineHandle` runs on the session side. They share only
//! wait-free `rtrb` queues: commands and graphs flow to the audio thread, events flow back, and the
//! graphs the audio thread replaces flow to the collector thread, which frees them. The audio thread
//! applies commands and graph swaps at the start of each driver callback and never allocates.

use crate::collector;
use crate::compiled::{CompiledGraph, MAX_BLOCK, Node, RAMP_STEP};
use crate::graph::sanitize_output_sample;
use crate::snapshot::{ClipState, EngineSnapshot, MAX_NODES, Transport, cpu_load};
use livecraft_devices::ProcessContext;
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
        };
        let handle = EngineHandle {
            commands: command_tx,
            graphs: graph_tx,
            events: event_rx,
            snapshots: snapshot_rx,
        };
        Ok((engine, handle))
    }

    /// Renders `out`, which holds interleaved frames of `channels` samples. It is the only call from
    /// the driver callback.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        if channels == 0 {
            return;
        }
        let started = Instant::now();
        self.apply_pending();
        let frames = out.len() / channels;
        let mut start = 0;
        while start < frames {
            self.apply_due_params();
            let block = self.segment_len(frames - start);
            let context = ProcessContext {
                sample_rate: self.sample_rate,
                frames: block,
                bpm: self.bpm,
                playhead_beats: self.playhead_at(self.frame),
                beats_per_frame: self.beats_per_frame(),
            };
            self.graph.render(&context);
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
        self.publish(frames, started.elapsed().as_secs_f32());
    }

    /// Writes this callback's state into the snapshot and publishes it. Only fixed-size fields are
    /// written, so nothing allocates or locks here.
    fn publish(&mut self, frames: usize, elapsed_seconds: f32) {
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
        snapshot.cpu_load = cpu_load(elapsed_seconds, frames, self.sample_rate);
        self.snapshots.publish();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiled::DeviceSlot;
    use crate::compiled::MAX_BLOCK;
    use crate::compiler::GraphCompiler;
    use crate::delay::DelayLine;
    use basedrop::Collector;
    use livecraft_devices::{AudioIo, DeviceProcessor, EventList, EventSink, NoteEvent};
    use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project};

    const RATE: f32 = 44100.0;

    /// The graph of the Set with a note queued on track 1 for the first block. The instrument sustains
    /// it after that.
    fn sounding(compiler: &mut GraphCompiler, project: &Project) -> CompiledGraph {
        let mut graph = compiler.compile(project);
        let track = graph.node_mut(TrackId(1)).expect("track 1");
        assert!(track.events.push(NoteEvent::On {
            offset: 0,
            pitch: 60,
            velocity: 100,
            freq_hz: 261.6,
        }));
        graph
    }

    /// Starts an engine on the Set, with the compiler that built its graph and shares its collector.
    fn started(project: &Project) -> (Engine, EngineHandle, GraphCompiler) {
        let collector = Collector::new();
        let mut compiler = GraphCompiler::new(RATE, collector.handle());
        let graph = sounding(&mut compiler, project);
        let (engine, handle) = Engine::start(graph, RATE, collector).expect("start");
        (engine, handle, compiler)
    }

    /// Renders `blocks` stereo blocks and returns the output as bit patterns.
    fn render_blocks(engine: &mut Engine, blocks: usize) -> Vec<u32> {
        let mut out = vec![0.0f32; MAX_BLOCK * 2];
        let mut bits = Vec::new();
        for _ in 0..blocks {
            engine.process(&mut out, 2);
            bits.extend(out.iter().map(|sample| sample.to_bits()));
        }
        bits
    }

    #[test]
    fn a_mute_command_silences_the_track_from_the_next_block() {
        let (mut engine, mut handle, _compiler) = started(&Project::new_default("Mute"));
        let before = render_blocks(&mut engine, 1);
        assert!(before.iter().any(|&bits| f32::from_bits(bits) != 0.0));
        handle
            .send(EngineCommand::SetTrackMute {
                track: TrackId(1),
                muted: true,
            })
            .expect("send");
        let after = render_blocks(&mut engine, 1);
        assert!(after.iter().all(|&bits| f32::from_bits(bits) == 0.0));
    }

    #[test]
    fn a_graph_swap_with_the_same_set_leaves_the_output_bit_identical() {
        let project = Project::new_default("Swap");
        let (mut reference, _reference_handle, _reference_compiler) = started(&project);
        let expected = render_blocks(&mut reference, 4);

        let (mut engine, mut handle, mut compiler) = started(&project);
        let mut swapped = render_blocks(&mut engine, 2);
        handle
            .submit_graph(compiler.compile(&project))
            .expect("submit");
        swapped.extend(render_blocks(&mut engine, 2));
        assert_eq!(swapped, expected);

        let mut events = Vec::new();
        handle.drain_events(&mut events);
        assert_eq!(events, vec![EngineEvent::GraphApplied]);
    }

    #[test]
    fn a_full_command_queue_refuses_the_next_command() {
        let (_engine, mut handle, _compiler) = started(&Project::new_default("Full"));
        let command = EngineCommand::SetTempo(100.0);
        for _ in 0..COMMAND_CAPACITY {
            handle.send(command).expect("room in the queue");
        }
        assert_eq!(handle.send(command), Err(QueueFull));
    }

    /// Renders `frames` frames of the left channel, in blocks of at most `MAX_BLOCK` frames.
    fn render_left(engine: &mut Engine, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; MAX_BLOCK * 2];
        let mut left = Vec::with_capacity(frames + MAX_BLOCK);
        while left.len() < frames {
            engine.process(&mut out, 2);
            left.extend(out.as_chunks::<2>().0.iter().map(|frame| frame[0]));
        }
        left
    }

    #[test]
    fn an_arrangement_clip_starts_and_stops_on_its_exact_sample() {
        use crate::audio_clip::DecodedAudio;
        use crate::clip::{ArrangedClip, PlayableClip, PlayableSource};
        use basedrop::Shared;

        // At 120 BPM and 44.1 kHz a beat is 22050 samples, so the clip covers samples 22050 to 44099.
        let collector = Collector::new();
        let mut compiler = GraphCompiler::new(RATE, collector.handle());
        let mut graph = compiler.compile(&Project::new_default("Arranged"));
        let data = DecodedAudio {
            samples_l: vec![0.5; 22050],
            samples_r: vec![0.5; 22050],
            sample_rate: 44100,
            channels: 2,
        };
        let track = graph.node_mut(TrackId(1)).expect("track 1");
        track.clips.arranged.push(ArrangedClip {
            start_beat: 1.0,
            length_beats: 1.0,
            clip: PlayableClip {
                length_beats: 1.0,
                is_looping: false,
                source: PlayableSource::Audio {
                    data: Shared::new(&collector.handle(), data),
                    gain: 1.0,
                    is_warped: false,
                    original_bpm: 120.0,
                },
            },
        });
        let (mut engine, mut handle) = Engine::start(graph, RATE, collector).expect("start");
        handle.send(EngineCommand::SetPlaying(true)).expect("send");
        let left = render_left(&mut engine, 44200);
        assert!(left[..22050].iter().all(|&sample| sample == 0.0));
        assert_ne!(left[22050], 0.0);
        assert_ne!(left[44099], 0.0);
        assert!(left[44100..].iter().all(|&sample| sample == 0.0));
    }

    #[test]
    fn a_session_clip_that_was_not_launched_stays_silent() {
        let mut project = Project::new_default("Timeline");
        let mut clip = Clip::new_midi(900, "Chord", 4.0);
        if let ClipPayload::Midi(ref mut data) = clip.payload {
            data.notes.push(MidiNote::new(60, 0.0, 4.0, 100));
        }
        project.tracks[0].clip_slots[0].clip = Some(clip);
        let collector = Collector::new();
        let mut compiler = GraphCompiler::new(RATE, collector.handle());
        let graph = compiler.compile(&project);
        let (mut engine, mut handle) = Engine::start(graph, RATE, collector).expect("start");
        handle.send(EngineCommand::SetPlaying(true)).expect("send");
        assert!(
            render_left(&mut engine, 4 * MAX_BLOCK)
                .iter()
                .all(|&s| s == 0.0)
        );
    }

    #[test]
    fn a_parameter_command_settles_on_the_value_in_the_set() {
        use livecraft_model::ids::DeviceId;
        let project = Project::new_default("Param");
        let mut edited = project.clone();
        edited.tracks[0].device_chain[0].set_param("osc1_level", 0.25);
        let (mut in_the_set, _set_handle, _set_compiler) = started(&edited);
        let mut expected = render_blocks(&mut in_the_set, 40);

        let (mut engine, mut handle, _compiler) = started(&project);
        handle
            .send(EngineCommand::SetDeviceParam {
                track: TrackId(1),
                device: DeviceId(101),
                param: ParamId(1),
                value: 0.25,
                offset: 0,
            })
            .expect("send");
        let mut actual = render_blocks(&mut engine, 40);
        // The level ramps over 10 ms, so the first blocks differ. The last two have settled.
        let settled = actual.len() - 2 * MAX_BLOCK * 2;
        assert_eq!(actual.split_off(settled), expected.split_off(settled));
    }

    /// Starts the Set with a constant 0.5 input on each listed track: an audio clip of four beats from
    /// beat 0, which the track's devices process, played from beat 0. `setup` edits the graph first.
    fn constant_on_tracks(
        project: &Project,
        tracks: &[TrackId],
        setup: impl FnOnce(&mut CompiledGraph),
    ) -> (Engine, EngineHandle) {
        use crate::audio_clip::DecodedAudio;
        use crate::clip::{ArrangedClip, PlayableClip, PlayableSource};
        use basedrop::Shared;
        let collector = Collector::new();
        let mut compiler = GraphCompiler::new(RATE, collector.handle());
        let mut graph = compiler.compile(project);
        for track_id in tracks {
            let samples = vec![0.5f32; 88200];
            let data = DecodedAudio {
                samples_l: samples.clone(),
                samples_r: samples,
                sample_rate: 44100,
                channels: 2,
            };
            let track = graph.node_mut(*track_id).expect("track");
            track.clips.arranged.push(ArrangedClip {
                start_beat: 0.0,
                length_beats: 4.0,
                clip: PlayableClip {
                    length_beats: 4.0,
                    is_looping: false,
                    source: PlayableSource::Audio {
                        data: Shared::new(&collector.handle(), data),
                        gain: 1.0,
                        is_warped: false,
                        original_bpm: 120.0,
                    },
                },
            });
        }
        setup(&mut graph);
        let (engine, mut handle) = Engine::start(graph, RATE, collector).expect("start");
        handle.send(EngineCommand::SetPlaying(true)).expect("send");
        (engine, handle)
    }

    /// The Set with the constant input on track 1 only.
    fn constant_input(project: &Project) -> (Engine, EngineHandle) {
        constant_on_tracks(project, &[TrackId(1)], |_| {})
    }

    /// A pass-through device that delays the signal by its delay line and reports that latency.
    struct LatencyDevice(DelayLine);

    impl DeviceProcessor for LatencyDevice {
        fn process(
            &mut self,
            _ctx: &ProcessContext,
            io: &mut AudioIo<'_>,
            _events: &EventList,
            _out_events: &mut EventSink,
        ) {
            self.0.apply(&mut *io.left, &mut *io.right);
        }

        fn set_param(&mut self, _id: ParamId, _value: f32, _offset: u32) {}

        fn latency_samples(&self) -> u32 {
            self.0.len() as u32
        }

        fn reset(&mut self) {}
    }

    /// A device slot holding a `LatencyDevice` of `frames` frames.
    fn latent(frames: usize) -> DeviceSlot {
        DeviceSlot::new(
            DeviceId(600),
            true,
            Box::new(LatencyDevice(DelayLine::new(frames))),
            Vec::new(),
            &[],
        )
    }

    /// Checks that `out` is `reference` delayed by `latency` frames, with each path's copy of the
    /// signal adding to exactly twice the reference.
    fn assert_aligned(out: &[f32], reference: &[f32], latency: usize) {
        assert!(reference.iter().any(|&sample| sample != 0.0));
        assert!(out[..latency].iter().all(|&sample| sample == 0.0));
        assert!(
            out[latency..]
                .iter()
                .zip(reference)
                .all(|(sample, source)| sample.to_bits() == (2.0 * source).to_bits())
        );
    }

    #[test]
    fn a_latency_device_on_one_track_leaves_the_main_output_aligned() {
        let project = Project::new_default("Aligned");
        let (mut single, _) = constant_input(&project);
        let reference = render_left(&mut single, 4096);
        let (mut pair, _) = constant_on_tracks(&project, &[TrackId(1), TrackId(2)], |graph| {
            graph.add_device(TrackId(2), latent(128));
        });
        assert_aligned(&render_left(&mut pair, 4096), &reference, 128);
    }

    #[test]
    fn a_latency_device_on_a_return_leaves_the_main_output_aligned() {
        let project = Project::new_default("Return");
        let (mut single, _) = constant_input(&project);
        let reference = render_left(&mut single, 4096);

        // Track 1 sends all of its level to the first return, whose reverb is cleared so that the
        // return passes the signal through its latency device alone.
        let mut sending = project.clone();
        sending.return_tracks[0].device_chain.clear();
        let return_id = sending.return_tracks[0].id;
        sending.tracks[0].sends = vec![0.0; sending.return_tracks.len()];
        sending.tracks[0].sends[0] = 1.0;
        let (mut pair, _) = constant_on_tracks(&sending, &[TrackId(1)], |graph| {
            graph.add_device(return_id, latent(128));
        });
        assert_aligned(&render_left(&mut pair, 4096), &reference, 128);
    }

    /// The Set with a Utility device after the track's devices, which the constant input runs through.
    fn with_utility() -> Project {
        use livecraft_model::ids::DeviceId;
        use livecraft_model::model::{Device, DeviceType};
        let mut project = Project::new_default("Utility");
        project.tracks[0].device_chain.push(Device::new(
            DeviceId(500),
            "Utility",
            DeviceType::Utility,
        ));
        project
    }

    #[test]
    fn a_stepped_parameter_takes_effect_at_its_sample_offset() {
        use livecraft_model::ids::DeviceId;
        let project = with_utility();
        let (mut reference, _) = constant_input(&project);
        let before = render_left(&mut reference, 1024);
        let (mut engine, mut handle) = constant_input(&project);
        handle
            .send(EngineCommand::SetDeviceParam {
                track: TrackId(1),
                device: DeviceId(500),
                param: ParamId(3),
                value: 1.0,
                offset: 300,
            })
            .expect("send");
        let after = render_left(&mut engine, 1024);
        assert!(
            after[..300]
                .iter()
                .zip(&before[..300])
                .all(|(a, b)| a.to_bits() == b.to_bits())
        );
        assert!(before[300..].iter().any(|&sample| sample != 0.0));
        assert!(after[300..].iter().all(|&sample| sample == 0.0));
    }

    #[test]
    fn a_continuous_parameter_ramps_instead_of_jumping() {
        use livecraft_model::ids::DeviceId;
        let (mut engine, mut handle) = constant_input(&with_utility());
        handle
            .send(EngineCommand::SetDeviceParam {
                track: TrackId(1),
                device: DeviceId(500),
                param: ParamId(0),
                value: -60.0,
                offset: 0,
            })
            .expect("send");
        let out = render_left(&mut engine, 4096);
        let peaks: Vec<f32> = out
            .chunks(64)
            .map(|window| window.iter().fold(0.0f32, |peak, s| peak.max(s.abs())))
            .collect();
        assert!(peaks.windows(2).all(|pair| pair[1] <= pair[0]));
        // A jump to -60 dB would leave the second window near silence. The ramp is still above it.
        assert!(peaks[1] > 0.1 * peaks[0]);
        assert!(peaks[6] < 0.5 * peaks[0]);
        assert!(peaks[peaks.len() - 1] < 0.01 * peaks[0]);
    }

    #[test]
    fn the_snapshot_reports_the_transport_and_the_levels() {
        let (mut engine, mut handle, _compiler) = started(&Project::new_default("Levels"));
        handle.send(EngineCommand::SetPlaying(true)).expect("send");
        render_blocks(&mut engine, 2);
        let snapshot = handle.snapshot();
        assert!(snapshot.transport.playing);
        assert_eq!(snapshot.transport.bpm, 120.0);
        // Two blocks of 512 frames at 120 BPM and 44.1 kHz, where a beat is 22050 frames.
        let expected = 1024.0 * 2.0 / 44100.0;
        assert!((snapshot.transport.playhead_beats - expected).abs() < 1e-9);
        assert!(snapshot.node_count > 1);
        assert!(snapshot.nodes[0].meter.peak_l > 0.0);
        assert!(snapshot.cpu_load >= 0.0 && snapshot.cpu_load.is_finite());
    }

    #[test]
    fn the_snapshot_reports_the_launched_clip_of_a_track() {
        let mut project = Project::new_default("Clip state");
        let mut clip = Clip::new_midi(900, "Chord", 4.0);
        if let ClipPayload::Midi(ref mut data) = clip.payload {
            data.notes.push(MidiNote::new(60, 0.0, 4.0, 100));
        }
        project.tracks[0].clip_slots[0].clip = Some(clip);
        let collector = Collector::new();
        let mut compiler = GraphCompiler::new(RATE, collector.handle());
        let graph = compiler.compile(&project);
        let (mut engine, mut handle) = Engine::start(graph, RATE, collector).expect("start");
        handle
            .send(EngineCommand::TriggerClip {
                track: TrackId(1),
                slot: 0,
            })
            .expect("send");
        handle.send(EngineCommand::SetPlaying(true)).expect("send");
        render_blocks(&mut engine, 1);
        assert_eq!(
            handle.snapshot().nodes[0].clip,
            ClipState {
                active_slot: Some(0),
                playing: true,
            }
        );
    }

    #[test]
    fn a_graph_with_more_nodes_than_the_snapshot_holds_is_refused() {
        use crate::clip::ClipLane;
        use crate::compiled::{Mixer, Node, NodeKind};
        let (_engine, mut handle, _compiler) = started(&Project::new_default("Large"));
        let mixer = Mixer {
            volume_db: 0.0,
            pan: 0.0,
            is_muted: false,
            is_soloed: false,
            sends: Vec::new(),
        };
        let nodes: Vec<Node> = (0..=MAX_NODES)
            .map(|index| {
                Node::new(
                    TrackId(index as u64 + 1),
                    NodeKind::Track,
                    mixer.clone(),
                    Vec::new(),
                    ClipLane::new(Vec::new(), Vec::new(), false, 0),
                )
            })
            .collect();
        let large = CompiledGraph::new(nodes, MAX_NODES + 1, 0, Vec::new(), Vec::new());
        assert_eq!(
            handle.submit_graph(large),
            Err(SubmitError::TooManyNodes {
                nodes: MAX_NODES + 1,
                max: MAX_NODES,
            })
        );
    }
}
