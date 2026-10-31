//! The real-time executor and its session-side handle (M05 item 6, TARGET-ARCHITECTURE "Engine
//! Decisions").
//!
//! `Engine` runs on the audio thread. `EngineHandle` runs on the session side. They share only
//! wait-free `rtrb` queues: commands and graphs flow to the audio thread, events flow back, and the
//! graphs the audio thread replaces flow to the collector thread, which frees them. The audio thread
//! applies commands and graph swaps at the start of each driver callback and never allocates.

use crate::collector;
use crate::compiled::{CompiledGraph, MAX_BLOCK, Node, RAMP_STEP};
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
        snapshot.output = self.graph.output_meter;
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

/// Replaces NaN and infinity with silence; finite samples pass through bit-exactly.
#[inline]
fn sanitize_output_sample(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

#[cfg(test)]
mod tests;
