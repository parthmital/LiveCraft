//! The compiled audio graph and its block renderer (M05 item 5, TARGET-ARCHITECTURE "Engine
//! Decisions").
//!
//! The session side builds a `CompiledGraph` with `GraphCompiler`. The audio thread renders it one
//! block at a time. Every buffer is allocated when the graph is built, so `render` allocates
//! nothing and takes no lock.
//!
//! Nodes are the tracks, then the return tracks, then the Main track. Tracks feed returns through
//! their sends, and tracks and returns feed Main, so this order is a topological order.

use crate::graph::TrackMeter;
use livecraft_devices::{AudioIo, DeviceProcessor, EventList, EventSink, ProcessContext};
use livecraft_model::ids::{DeviceId, SceneId, TrackId};
use livecraft_model::param::ParamId;

/// Most frames one render call handles. The engine cuts a driver buffer into blocks of this size.
pub const MAX_BLOCK: usize = 512;

/// Most note events one node holds in a block.
pub const MAX_NOTE_EVENTS: usize = 256;

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
    /// The level after the device chain, before the mixer.
    pub meter: TrackMeter,
    left: Box<[f32]>,
    right: Box<[f32]>,
}

impl Node {
    pub fn new(track: TrackId, kind: NodeKind, mixer: Mixer, devices: Vec<DeviceSlot>) -> Self {
        Self {
            track,
            kind,
            mixer,
            devices,
            events: EventList::with_capacity(MAX_NOTE_EVENTS),
            meter: TrackMeter::default(),
            left: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            right: vec![0.0; MAX_BLOCK].into_boxed_slice(),
        }
    }

    fn clear(&mut self, frames: usize) {
        self.left[..frames].fill(0.0);
        self.right[..frames].fill(0.0);
    }

    /// Runs the enabled devices over the node's buffers in chain order, then updates the meter.
    fn run_devices(&mut self, ctx: &ProcessContext) {
        let frames = ctx.frames;
        let mut io = AudioIo {
            left: &mut self.left[..frames],
            right: &mut self.right[..frames],
        };
        let mut sink = EventSink::default();
        for slot in self.devices.iter_mut().filter(|slot| slot.enabled) {
            slot.processor
                .process(ctx, &mut io, &self.events, &mut sink);
        }
        for frame in 0..frames {
            self.meter.update(self.left[frame], self.right[frame]);
        }
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
            track.run_devices(&ProcessContext { frames, ..*ctx });
            if track.mixer.is_muted || (any_solo && !track.mixer.is_soloed) {
                continue;
            }
            let gain = volume_gain(track.mixer.volume_db);
            let (pan_l, pan_r) = pan_gains(track.mixer.pan);
            for frame in 0..frames {
                let out_l = track.left[frame] * gain * pan_l;
                let out_r = track.right[frame] * gain * pan_r;
                main.left[frame] += out_l;
                main.right[frame] += out_r;
                for (ret, level) in returns.iter_mut().zip(&track.mixer.sends) {
                    ret.left[frame] += out_l * level;
                    ret.right[frame] += out_r * level;
                }
            }
        }

        // Returns add their volume to Main without panning, as the mixer did before M05.
        for ret in returns.iter_mut() {
            ret.run_devices(&ProcessContext { frames, ..*ctx });
            let gain = volume_gain(ret.mixer.volume_db);
            for frame in 0..frames {
                main.left[frame] += ret.left[frame] * gain;
                main.right[frame] += ret.right[frame] * gain;
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
