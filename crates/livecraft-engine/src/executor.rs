//! The real-time executor and its session-side handle (M05 item 6, TARGET-ARCHITECTURE "Engine
//! Decisions").
//!
//! `Engine` runs on the audio thread. `EngineHandle` runs on the session side. They share only
//! wait-free `rtrb` queues: commands and graphs flow to the audio thread, events flow back, and the
//! graphs the audio thread replaces flow to the collector thread, which frees them. The audio thread
//! applies commands and graph swaps at the start of each driver callback and never allocates.

use crate::collector;
use crate::compiled::{CompiledGraph, MAX_BLOCK, Node};
use crate::graph::sanitize_output_sample;
use livecraft_devices::ProcessContext;
use livecraft_model::ids::{DeviceId, SceneId, TrackId};
use livecraft_model::param::ParamId;
use livecraft_time::beats_per_sample;

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
    /// Sets a parameter of a device on a track, by its id in the device table.
    SetDeviceParam {
        track: TrackId,
        device: DeviceId,
        param: ParamId,
        value: f32,
    },
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

/// The session side: sends commands and graphs, and reads events.
pub struct EngineHandle {
    commands: rtrb::Producer<EngineCommand>,
    graphs: rtrb::Producer<Box<CompiledGraph>>,
    events: rtrb::Consumer<EngineEvent>,
}

impl EngineHandle {
    /// Sends a command. Commands apply in the order they were sent.
    pub fn send(&mut self, command: EngineCommand) -> Result<(), QueueFull> {
        self.commands.push(command).map_err(|_| QueueFull)
    }

    /// Submits a graph built by `GraphCompiler`, in the order it was compiled. The graph replaces
    /// the running one at the next block boundary.
    pub fn submit_graph(&mut self, graph: CompiledGraph) -> Result<(), QueueFull> {
        self.graphs.push(Box::new(graph)).map_err(|_| QueueFull)
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
        };
        let handle = EngineHandle {
            commands: command_tx,
            graphs: graph_tx,
            events: event_rx,
        };
        Ok((engine, handle))
    }

    /// Renders `out`, which holds interleaved frames of `channels` samples. It is the only call from
    /// the driver callback.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        if channels == 0 {
            return;
        }
        self.apply_pending();
        let frames = out.len() / channels;
        let mut start = 0;
        while start < frames {
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
            } => self.graph.set_device_param(track, device, param, value),
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
        let limit = remaining.min(MAX_BLOCK);
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
    use crate::compiled::MAX_BLOCK;
    use crate::compiler::GraphCompiler;
    use basedrop::Collector;
    use livecraft_devices::NoteEvent;
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
    fn a_parameter_command_gives_the_same_output_as_the_value_in_the_set() {
        use livecraft_model::ids::DeviceId;
        let project = Project::new_default("Param");
        let mut edited = project.clone();
        edited.tracks[0].device_chain[0].set_param("osc1_level", 0.25);
        let (mut in_the_set, _set_handle, _set_compiler) = started(&edited);
        let expected = render_blocks(&mut in_the_set, 2);

        let (mut engine, mut handle, _compiler) = started(&project);
        handle
            .send(EngineCommand::SetDeviceParam {
                track: TrackId(1),
                device: DeviceId(101),
                param: ParamId(1),
                value: 0.25,
            })
            .expect("send");
        assert_eq!(render_blocks(&mut engine, 2), expected);
    }
}
