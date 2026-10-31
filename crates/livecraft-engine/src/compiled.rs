//! The compiled audio graph and its block renderer (M05 item 5, TARGET-ARCHITECTURE "Engine
//! Decisions").
//!
//! The session side builds a `CompiledGraph` with `GraphCompiler`. The audio thread renders it one
//! block at a time. Every buffer is allocated when the graph is built, so `render` allocates
//! nothing and takes no lock.
//!
//! Nodes are the tracks, then the return tracks, then the Main track. Tracks feed returns through
//! their sends, and tracks and returns feed Main, so this order is a topological order.

use crate::clip::ClipLane;
use crate::delay::DelayLine;
use crate::graph::TrackMeter;
use livecraft_devices::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::{DeviceId, SceneId, TrackId};
use livecraft_model::param::{ParamId, ParamSpec};

/// Most frames one render call handles. The engine cuts a driver buffer into blocks of this size.
pub const MAX_BLOCK: usize = 512;

/// Most note events one node holds in a block.
pub const MAX_NOTE_EVENTS: usize = 256;

/// Most frames a block holds while a parameter ramps, so the ramp moves in small steps.
pub const RAMP_STEP: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Track,
    Return,
    Main,
}

/// The mix settings of one node, as the Set holds them.
#[derive(Debug, Clone, PartialEq)]
pub struct Mixer {
    pub volume_db: f32,
    pub pan: f32,
    pub is_muted: bool,
    pub is_soloed: bool,
    /// The send level to each return track, by return position. Only tracks have sends.
    pub sends: Vec<f32>,
}

/// A device of a node: whether it is enabled, its processor, and the parameter values the Set
/// holds for it. A disabled device is skipped.
pub struct DeviceSlot {
    pub id: DeviceId,
    pub enabled: bool,
    pub processor: Box<dyn DeviceProcessor>,
    pub values: Vec<(ParamId, f32)>,
    /// The parameter table of the device kind, which says how each parameter is smoothed.
    pub table: &'static [ParamSpec],
    /// Continuous parameters still moving to their target value.
    pub ramps: Vec<Ramp>,
    /// The processor's latency in frames, which the compiler compensates (manual 23.3).
    pub latency: usize,
    /// Delays the signal of a bypassed device by its latency, so bypassing it keeps the timing
    /// (manual 39.2.8).
    bypass: DelayLine,
}

/// A continuous parameter that moves from the value its processor holds toward the value the Set
/// asks for (`ParamSpec::smoothing_ms`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ramp {
    pub id: ParamId,
    pub current: f32,
    pub target: f32,
}

impl DeviceSlot {
    /// A slot for `processor`, with the latency the processor reports. `table` is the device kind's
    /// parameter table, and `values` the values the Set holds for the device.
    pub fn new(
        id: DeviceId,
        enabled: bool,
        processor: Box<dyn DeviceProcessor>,
        values: Vec<(ParamId, f32)>,
        table: &'static [ParamSpec],
    ) -> Self {
        let latency = processor.latency_samples() as usize;
        Self {
            id,
            enabled,
            processor,
            values,
            table,
            ramps: Vec::with_capacity(table.len()),
            latency,
            bypass: DelayLine::new(latency),
        }
    }

    /// Moves each ramp over `frames` frames toward its target and gives the processor the new value.
    /// A ramp that has arrived is dropped. The step is exponential, so a ramp takes the same time at
    /// any block size.
    fn apply_ramps(&mut self, frames: usize, sample_rate: f32) {
        let table = self.table;
        let processor = &mut self.processor;
        self.ramps.retain_mut(|ramp| {
            let smoothing_ms = table
                .get(ramp.id.0 as usize)
                .map_or(0.0, |spec| spec.smoothing_ms);
            let decay = (-(frames as f32) / (smoothing_ms * sample_rate / 1000.0)).exp();
            ramp.current = ramp.target + (ramp.current - ramp.target) * decay;
            if (ramp.current - ramp.target).abs() <= 1e-6 * (1.0 + ramp.target.abs()) {
                ramp.current = ramp.target;
            }
            processor.set_param(ramp.id, ramp.current, 0);
            ramp.current != ramp.target
        });
    }
}

/// Moves the processor of a device that a new graph keeps from the previous graph's slot to the new
/// graph's slot (the transfer list, TARGET-ARCHITECTURE "Engine Decisions"). `from` and `to` are
/// (node index, device slot index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub from: (usize, usize),
    pub to: (usize, usize),
}

/// One track, return track, or the Main track, with its buffers.
pub struct Node {
    pub track: TrackId,
    pub kind: NodeKind,
    pub mixer: Mixer,
    /// Instruments first, then the effects in chain order.
    pub devices: Vec<DeviceSlot>,
    /// The note events of the current block. Instruments play them.
    pub events: EventList,
    /// The clips of the track. Return and Main nodes have none.
    pub clips: ClipLane,
    /// The level after the device chain, before the mixer.
    pub meter: TrackMeter,
    left: Box<[f32]>,
    right: Box<[f32]>,
    /// Delays a track's input so its sends and its mix start together (see `compensate_latency`).
    input_delay: DelayLine,
    /// Delays this node's output to Main so every path reaches Main at the same time.
    main_delay: DelayLine,
}

impl Node {
    pub fn new(
        track: TrackId,
        kind: NodeKind,
        mixer: Mixer,
        devices: Vec<DeviceSlot>,
        clips: ClipLane,
    ) -> Self {
        Self {
            track,
            kind,
            mixer,
            devices,
            events: EventList::with_capacity(MAX_NOTE_EVENTS),
            clips,
            meter: TrackMeter::default(),
            left: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            right: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            input_delay: DelayLine::default(),
            main_delay: DelayLine::default(),
        }
    }

    /// Delays the track's input by its compensation, so the other tracks and its sends line up.
    fn delay_input(&mut self, frames: usize) {
        self.input_delay
            .apply(&mut self.left[..frames], &mut self.right[..frames]);
    }

    /// Fills this block's note events and audio from the clips the track plays.
    fn schedule_clips(&mut self, ctx: &ProcessContext) {
        let frames = ctx.frames;
        self.clips.schedule(
            ctx,
            &mut self.events,
            &mut self.left[..frames],
            &mut self.right[..frames],
        );
    }

    fn clear(&mut self, frames: usize) {
        self.left[..frames].fill(0.0);
        self.right[..frames].fill(0.0);
    }

    /// Runs the enabled devices over the node's buffers in chain order, then updates the meter.
    fn run_devices(&mut self, ctx: &ProcessContext) {
        let frames = ctx.frames;
        for slot in self.devices.iter_mut() {
            slot.apply_ramps(frames, ctx.sample_rate);
        }
        let mut io = AudioIo {
            left: &mut self.left[..frames],
            right: &mut self.right[..frames],
        };
        let mut sink = EventSink::default();
        for slot in self.devices.iter_mut() {
            if slot.enabled {
                slot.processor
                    .process(ctx, &mut io, &self.events, &mut sink);
            } else {
                slot.bypass.apply(&mut *io.left, &mut *io.right);
            }
        }
        for frame in 0..frames {
            self.meter.update(self.left[frame], self.right[frame]);
        }
        // The events belong to this block only. The next block schedules its own, so none carry over.
        self.events.clear();
    }
}

/// The compiled graph: nodes in topological order, and the Main output level.
pub struct CompiledGraph {
    nodes: Vec<Node>,
    track_count: usize,
    return_count: usize,
    /// The Set's scenes, in row order, so a scene can be triggered by its row.
    pub scenes: Vec<SceneId>,
    /// The level of the output after the Main volume, which is what the device writes out.
    pub output_meter: TrackMeter,
    transfers: Vec<Transfer>,
}

/// Amplitude for a volume in dB. Volumes at or below -59 dB are silent, as the Set's fader shows.
fn volume_gain(volume_db: f32) -> f32 {
    if volume_db <= -59.0 {
        0.0
    } else {
        10.0f32.powf(volume_db / 20.0)
    }
}

/// Constant-power pan gains for a pan from -1 (left) to 1 (right).
fn pan_gains(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
    (angle.cos(), angle.sin())
}

/// The latency a node adds: the latencies of its devices, bypassed ones included.
fn node_latency(node: &Node) -> usize {
    node.devices.iter().map(|slot| slot.latency).sum()
}

/// Sets the delay lines that bring every path to Main at the same time (manual 23.3). A track delays
/// its input until the slowest track's output would be ready, so its sends and its mix start together.
/// Its direct output to Main then waits for the slowest return. Each return delays its output to the
/// same time. `nodes` must be the tracks, then the returns, then Main.
pub fn compensate_latency(nodes: &mut [Node], track_count: usize, return_count: usize) {
    let (tracks, rest) = nodes.split_at_mut(track_count);
    let (returns, _main) = rest.split_at_mut(return_count);
    let track_max = tracks.iter().map(node_latency).fold(0, usize::max);
    let return_max = returns.iter().map(node_latency).fold(0, usize::max);
    for track in tracks.iter_mut() {
        track.input_delay = DelayLine::new(track_max - node_latency(track));
        track.main_delay = DelayLine::new(return_max);
    }
    for ret in returns.iter_mut() {
        ret.main_delay = DelayLine::new(return_max - node_latency(ret));
    }
}

impl CompiledGraph {
    /// Assembles a graph from its nodes, which must be the tracks, then the return tracks, then the
    /// Main track. `transfers` lists the devices this graph takes over from the one it replaces.
    pub fn new(
        nodes: Vec<Node>,
        track_count: usize,
        return_count: usize,
        scenes: Vec<SceneId>,
        transfers: Vec<Transfer>,
    ) -> Self {
        Self {
            nodes,
            track_count,
            return_count,
            scenes,
            output_meter: TrackMeter::default(),
            transfers,
        }
    }

    /// Takes the processors this graph keeps from `previous`, so their state carries over, and
    /// applies the Set's parameter values to them. It moves pointers and sets parameters only, so it
    /// does not allocate, and the processors the new graph replaces end up in `previous` to be freed
    /// off the audio thread.
    pub fn adopt_state(&mut self, previous: &mut CompiledGraph) {
        for transfer in &self.transfers {
            let source = previous
                .nodes
                .get_mut(transfer.from.0)
                .and_then(|node| node.devices.get_mut(transfer.from.1));
            let target = self
                .nodes
                .get_mut(transfer.to.0)
                .and_then(|node| node.devices.get_mut(transfer.to.1));
            let (Some(source), Some(target)) = (source, target) else {
                continue;
            };
            std::mem::swap(&mut source.processor, &mut target.processor);
            for &(id, value) in &target.values {
                target.processor.set_param(id, value, 0);
            }
        }
    }

    /// Every node: the tracks, the returns, and Main last.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The node of a track, return, or the Main track.
    pub fn node_mut(&mut self, track: TrackId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|node| node.track == track)
    }

    /// Launches a session slot on a track (its clip replaces the one playing).
    pub fn launch_clip(&mut self, track: TrackId, slot: usize) {
        if let Some(node) = self.node_mut(track) {
            node.clips.launch(slot);
        }
    }

    /// Stops the clip a track is playing.
    pub fn stop_clip(&mut self, track: TrackId) {
        if let Some(node) = self.node_mut(track) {
            node.clips.stop();
        }
    }

    /// The first beat after `position` at which a track starts or stops an arrangement clip.
    pub fn next_boundary(&self, position: f64) -> Option<f64> {
        self.nodes[..self.track_count]
            .iter()
            .filter_map(|node| node.clips.next_boundary(position))
            .reduce(f64::min)
    }

    /// Launches the slot in the scene's row on every track that has a clip there.
    pub fn launch_scene(&mut self, scene: SceneId) {
        let Some(row) = self.scenes.iter().position(|id| *id == scene) else {
            return;
        };
        for node in &mut self.nodes[..self.track_count] {
            if matches!(node.clips.slots.get(row), Some(Some(_))) {
                node.clips.launch(row);
            }
        }
    }

    /// Sets a parameter of a device on a track. The stored values follow, so a graph swap keeps the
    /// value. A continuous parameter ramps from the value it has now (`ParamSpec::smoothing_ms`); a
    /// stepped one takes the new value at once.
    pub fn set_device_param(
        &mut self,
        track: TrackId,
        device: DeviceId,
        param: ParamId,
        value: f32,
    ) {
        let Some(node) = self.node_mut(track) else {
            return;
        };
        let Some(slot) = node.devices.iter_mut().find(|slot| slot.id == device) else {
            return;
        };
        let previous = slot
            .values
            .iter()
            .find(|(id, _)| *id == param)
            .map(|entry| entry.1);
        if let Some(entry) = slot.values.iter_mut().find(|(id, _)| *id == param) {
            entry.1 = value;
        } else if slot.values.len() < slot.values.capacity() {
            slot.values.push((param, value));
        }
        let smoothing_ms = slot
            .table
            .get(param.0 as usize)
            .map_or(0.0, |spec| spec.smoothing_ms);
        if smoothing_ms <= 0.0 {
            slot.ramps.retain(|ramp| ramp.id != param);
            slot.processor.set_param(param, value, 0);
        } else if let Some(ramp) = slot.ramps.iter_mut().find(|ramp| ramp.id == param) {
            ramp.target = value;
        } else {
            slot.ramps.push(Ramp {
                id: param,
                current: previous.unwrap_or(value),
                target: value,
            });
        }
    }

    /// Adds a device to a track and recomputes the latency compensation. A session edit compiles a
    /// new graph instead, so this is for tests and for code that builds a graph by hand.
    pub fn add_device(&mut self, track: TrackId, slot: DeviceSlot) {
        let Some(node) = self.node_mut(track) else {
            return;
        };
        node.devices.push(slot);
        compensate_latency(&mut self.nodes, self.track_count, self.return_count);
    }

    /// True while a continuous parameter is still ramping. The engine then renders in short steps.
    pub fn ramping(&self) -> bool {
        self.nodes
            .iter()
            .any(|node| node.devices.iter().any(|slot| !slot.ramps.is_empty()))
    }

    /// Stops the clips on every track.
    pub fn stop_all(&mut self) {
        for node in &mut self.nodes[..self.track_count] {
            node.clips.stop();
        }
    }

    /// The left and right output of the last rendered block.
    pub fn output(&self, frames: usize) -> (&[f32], &[f32]) {
        let main = &self.nodes[self.nodes.len() - 1];
        (&main.left[..frames], &main.right[..frames])
    }

    /// Renders one block of `ctx.frames` frames (at most `MAX_BLOCK`) into the output buffers.
    pub fn render(&mut self, ctx: &ProcessContext) {
        let frames = ctx.frames.min(MAX_BLOCK);
        let any_solo = self.nodes[..self.track_count]
            .iter()
            .any(|node| node.mixer.is_soloed);
        let CompiledGraph {
            nodes,
            track_count,
            return_count,
            output_meter,
            ..
        } = self;
        let (tracks, mixing) = nodes.split_at_mut(*track_count);
        let (returns, main) = mixing.split_at_mut(*return_count);
        let main = &mut main[0];
        main.clear(frames);
        for ret in returns.iter_mut() {
            ret.clear(frames);
        }

        for track in tracks.iter_mut() {
            track.clear(frames);
            track.schedule_clips(&ProcessContext { frames, ..*ctx });
            track.run_devices(&ProcessContext { frames, ..*ctx });
            track.delay_input(frames);
            // A muted or unsoloed track is silent, but it still runs its delay so later blocks line up.
            let audible = !(track.mixer.is_muted || (any_solo && !track.mixer.is_soloed));
            let gain = if audible {
                volume_gain(track.mixer.volume_db)
            } else {
                0.0
            };
            let (pan_l, pan_r) = pan_gains(track.mixer.pan);
            for frame in 0..frames {
                let out_l = track.left[frame] * gain * pan_l;
                let out_r = track.right[frame] * gain * pan_r;
                for (ret, level) in returns.iter_mut().zip(&track.mixer.sends) {
                    ret.left[frame] += out_l * level;
                    ret.right[frame] += out_r * level;
                }
                let (main_l, main_r) = track.main_delay.process(out_l, out_r);
                main.left[frame] += main_l;
                main.right[frame] += main_r;
            }
        }

        // Returns add their volume to Main without panning, as the mixer did before M05.
        for ret in returns.iter_mut() {
            ret.run_devices(&ProcessContext { frames, ..*ctx });
            let gain = volume_gain(ret.mixer.volume_db);
            for frame in 0..frames {
                let (ret_l, ret_r) = ret.main_delay.process(ret.left[frame], ret.right[frame]);
                main.left[frame] += ret_l * gain;
                main.right[frame] += ret_r * gain;
            }
        }

        main.run_devices(&ProcessContext { frames, ..*ctx });
        let gain = volume_gain(main.mixer.volume_db);
        for frame in 0..frames {
            main.left[frame] *= gain;
            main.right[frame] *= gain;
            output_meter.update(main.left[frame], main.right[frame]);
        }
    }
}
