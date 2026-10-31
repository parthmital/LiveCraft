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
use livecraft_model::ids::TrackId;
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
    pub fn start(
        graph: CompiledGraph,
        sample_rate: f32,
    ) -> std::io::Result<(Engine, EngineHandle)> {
        let (command_tx, command_rx) = rtrb::RingBuffer::new(COMMAND_CAPACITY);
        let (graph_tx, graph_rx) = rtrb::RingBuffer::new(GRAPH_CAPACITY);
        let (retired_tx, retired_rx) = rtrb::RingBuffer::new(GRAPH_CAPACITY);
        let (event_tx, event_rx) = rtrb::RingBuffer::new(EVENT_CAPACITY);
        collector::spawn(retired_rx)?;
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
            let block = (frames - start).min(MAX_BLOCK);
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
    use livecraft_devices::NoteEvent;
    use livecraft_model::model::Project;

    const RATE: f32 = 44100.0;

    /// The graph of the Set with a note held on track 1. The note is in the block's event list on
    /// every block, which keeps the track sounding until clip playback sends events (M05 item 7).
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
        let project = Project::new_default("Mute");
        let mut compiler = GraphCompiler::new(RATE);
        let (mut engine, mut handle) =
            Engine::start(sounding(&mut compiler, &project), RATE).expect("start");
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
        let mut reference_compiler = GraphCompiler::new(RATE);
        let (mut reference, _reference_handle) =
            Engine::start(sounding(&mut reference_compiler, &project), RATE).expect("start");
        let expected = render_blocks(&mut reference, 4);

        let mut compiler = GraphCompiler::new(RATE);
        let (mut engine, mut handle) =
            Engine::start(sounding(&mut compiler, &project), RATE).expect("start");
        let mut swapped = render_blocks(&mut engine, 2);
        handle
            .submit_graph(sounding(&mut compiler, &project))
            .expect("submit");
        swapped.extend(render_blocks(&mut engine, 2));
        assert_eq!(swapped, expected);

        let mut events = Vec::new();
        handle.drain_events(&mut events);
        assert_eq!(events, vec![EngineEvent::GraphApplied]);
    }

    #[test]
    fn a_full_command_queue_refuses_the_next_command() {
        let project = Project::new_default("Full");
        let mut compiler = GraphCompiler::new(RATE);
        let (_engine, mut handle) =
            Engine::start(sounding(&mut compiler, &project), RATE).expect("start");
        let command = EngineCommand::SetTempo(100.0);
        for _ in 0..COMMAND_CAPACITY {
            handle.send(command).expect("room in the queue");
        }
        assert_eq!(handle.send(command), Err(QueueFull));
    }
}
