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
use crate::snapshot::TrackMeter;
use crate::transport::Timeline;
use livecraft_devices::{
    AudioIo, DeviceProcessor, EventList, EventSink, NoteEvent, ProcessContext,
};
use livecraft_model::ids::{DeviceId, SceneId, TrackId};
use livecraft_model::model::MonitoringMode;
use livecraft_model::param::{ParamId, ParamSpec};

/// Most frames one render call handles. The engine cuts a driver buffer into blocks of this size.
pub const MAX_BLOCK: usize = 512;

/// Most note events one node holds in a block.
pub const MAX_NOTE_EVENTS: usize = 256;

/// Most frames a block holds while a parameter ramps, so the ramp moves in small steps.
pub const RAMP_STEP: usize = 32;

/// The external audio input of one block: interleaved frames of `channels` samples, as the driver's
/// input stream delivers them.
#[derive(Debug, Clone, Copy)]
pub struct InputBlock<'a> {
    pub samples: &'a [f32],
    pub channels: usize,
}

impl InputBlock<'_> {
    /// No input. Every sample reads as silence.
    pub const SILENT: InputBlock<'static> = InputBlock {
        samples: &[],
        channels: 0,
    };

    /// The sample of an input channel in a frame of the block, or silence when the input has no such
    /// channel.
    fn sample(&self, frame: usize, channel: u16) -> f32 {
        let channel = usize::from(channel);
        if channel >= self.channels {
            return 0.0;
        }
        self.samples
            .get(frame * self.channels + channel)
            .copied()
            .unwrap_or(0.0)
    }
}

/// How a track takes its external input (manual 17.2, page 375, and 17.1, page 374).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputRoute {
    /// The zero-based input channel the track receives, or None when it has no external input.
    pub channel: Option<u16>,
    pub monitoring: MonitoringMode,
    pub is_armed: bool,
    /// Whether the track is a MIDI track. A MIDI track takes the MIDI input while it monitors its input
    /// (manual 17.3, page 375).
    pub takes_midi: bool,
}

/// Where a MIDI track sends its clip notes: an output port and a MIDI channel (manual 17.3, page 375).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiOutRoute {
    /// The index of the output port in the list the system offers (`midi_io::list_midi_ports`).
    pub port: u8,
    /// The MIDI channel, from 0 to 15.
    pub channel: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Track,
    Return,
    Main,
}

/// Which signal path [`CompiledGraph::render_with_input`] isolates into [`CompiledGraph::output`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StemTarget {
    /// Normal mix at the Main track's post-fader output.
    #[default]
    Main,
    /// Dry post-fader/post-pan output of a single node (track, group, return, or Main).
    DryNode(TrackId),
    /// Wet output of a single node routed through sends, returns, and Main effects.
    WetNode(TrackId),
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
    /// Index in `CompiledGraph::nodes` of the parent Group track that sums this track, if any.
    pub parent_idx: Option<usize>,
    pub mixer: Mixer,
    /// Instruments first, then the effects in chain order.
    pub devices: Vec<DeviceSlot>,
    /// The note events of the current block. Instruments play them.
    pub events: EventList,
    /// The clips of the track. Return and Main nodes have none.
    pub clips: ClipLane,
    /// The external input the track receives, and how it monitors it. Only tracks have one.
    pub input: InputRoute,
    /// Where the track sends its clip notes, when it is a MIDI track routed to an external port.
    pub midi_out: Option<MidiOutRoute>,
    /// The clip note events of the current block that go to `midi_out`. They are copied out of `events`
    /// before the devices run, because the devices clear `events`.
    pub midi_out_notes: EventList,
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
        input: InputRoute,
        midi_out: Option<MidiOutRoute>,
    ) -> Self {
        Self {
            track,
            kind,
            parent_idx: None,
            mixer,
            devices,
            events: EventList::with_capacity(MAX_NOTE_EVENTS),
            clips,
            input,
            midi_out,
            midi_out_notes: EventList::with_capacity(MAX_NOTE_EVENTS),
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

    /// Adds the external input channel to both sides of the track while the track monitors its input,
    /// before the devices run, so the devices process what the track hears (manual 17.1, page 374).
    fn monitor_input(&mut self, input: &InputBlock<'_>, frames: usize) {
        let Some(channel) = self.input.channel else {
            return;
        };
        if !self.monitors_input() {
            return;
        }
        for frame in 0..frames {
            let sample = input.sample(frame, channel);
            self.left[frame] += sample;
            self.right[frame] += sample;
        }
    }

    /// Whether the track monitors its input now. In always does, Off never does, and Auto does while
    /// the track is armed (manual 17.1, page 374). Auto does not check for clips: an armed track plays
    /// no clips (`GraphCompiler::lane`).
    fn monitors_input(&self) -> bool {
        match self.input.monitoring {
            MonitoringMode::In => true,
            MonitoringMode::Auto => self.input.is_armed,
            MonitoringMode::Off => false,
        }
    }

    /// Whether the track takes the MIDI input now: a MIDI track that monitors its input.
    fn monitors_midi(&self) -> bool {
        self.input.takes_midi && self.monitors_input()
    }

    /// Fills this block's note events and audio from the clips the track plays. When the track sends its
    /// notes to an external port, the notes its clips schedule are copied to `midi_out_notes`.
    fn schedule_clips(&mut self, ctx: &ProcessContext, clock: f64) {
        let frames = ctx.frames;
        let before = self.events.len();
        self.clips.schedule(
            ctx,
            clock,
            &mut self.events,
            &mut self.left[..frames],
            &mut self.right[..frames],
        );
        if self.midi_out.is_some() {
            self.midi_out_notes.clear();
            for event in self.events.iter().skip(before) {
                // A full list drops the note, as the block's own events do.
                let _ = self.midi_out_notes.push(*event);
            }
        }
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

/// The compiled graph: nodes in topological order, the song timeline, and the Main output level.
pub struct CompiledGraph {
    nodes: Vec<Node>,
    track_count: usize,
    return_count: usize,
    /// Indices into `nodes[..track_count]` ordered so every child runs before its parent Group.
    track_order: Vec<usize>,
    /// Active isolation target for rendering.
    stem_target: StemTarget,
    /// Preallocated tap buffers for isolated dry stem rendering.
    tap_left: Box<[f32]>,
    tap_right: Box<[f32]>,
    /// The tempo and time signature maps, the scenes in row order, and the transport settings.
    pub timeline: Timeline,
    /// The launch clock at the first frame of the block being rendered (`launch`). The engine sets it.
    pub clock: f64,
    /// The level of the output after the Main volume, which is what the device writes out.
    pub output_meter: TrackMeter,
    transfers: Vec<Transfer>,
    /// The frequency of each MIDI pitch in hertz, from the Set's tuning (`GraphCompiler::compile`).
    note_frequencies: [f32; 128],
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

/// Computes a child-before-parent topological order over `nodes[..track_count]`, clearing any
/// invalid or cyclic `parent_idx` references so rendering never loops or double-borrows.
pub fn topological_track_order(nodes: &mut [Node], track_count: usize) -> Vec<usize> {
    let count = track_count.min(nodes.len());
    let mut child_counts = vec![0usize; count];
    for (idx, node) in nodes[..count].iter_mut().enumerate() {
        if let Some(parent) = node.parent_idx {
            if parent < count && parent != idx {
                child_counts[parent] += 1;
            } else {
                node.parent_idx = None;
            }
        }
    }

    let mut queue = std::collections::VecDeque::with_capacity(count);
    for (idx, &deg) in child_counts.iter().enumerate() {
        if deg == 0 {
            queue.push_back(idx);
        }
    }

    let mut order = Vec::with_capacity(count);
    let mut seen = vec![false; count];
    while let Some(idx) = queue.pop_front() {
        if seen[idx] {
            continue;
        }
        seen[idx] = true;
        order.push(idx);
        if let Some(parent) = nodes[idx].parent_idx
            && child_counts[parent] > 0
        {
            child_counts[parent] -= 1;
            if child_counts[parent] == 0 {
                queue.push_back(parent);
            }
        }
    }

    if order.len() < count {
        for (idx, flag) in seen.iter().enumerate() {
            if !*flag {
                nodes[idx].parent_idx = None;
                order.push(idx);
            }
        }
    }

    order
}

/// Sets the delay lines that bring every path to Main at the same time (manual 23.3). A track delays
/// its input until the slowest track's output would be ready, so its sends and its mix start together.
/// Its direct output to Main then waits for the slowest return. Each return delays its output to the
/// same time. `nodes` must be the tracks, then the returns, then Main.
pub fn compensate_latency(nodes: &mut [Node], track_count: usize, return_count: usize) {
    let order = topological_track_order(nodes, track_count);
    let (tracks, rest) = nodes.split_at_mut(track_count);
    let (returns, _main) = rest.split_at_mut(return_count);

    let mut subtree_latency = vec![0usize; track_count];
    let mut child_max = vec![0usize; track_count];
    for &idx in &order {
        let total = node_latency(&tracks[idx]) + child_max[idx];
        subtree_latency[idx] = total;
        if let Some(parent) = tracks[idx].parent_idx {
            child_max[parent] = child_max[parent].max(total);
        }
    }

    let root_max = (0..track_count)
        .filter(|&idx| tracks[idx].parent_idx.is_none())
        .map(|idx| subtree_latency[idx])
        .fold(0, usize::max);
    let return_max = returns.iter().map(node_latency).fold(0, usize::max);

    for (idx, track) in tracks.iter_mut().enumerate() {
        if let Some(parent) = track.parent_idx {
            track.input_delay = DelayLine::new(child_max[parent] - subtree_latency[idx]);
            track.main_delay = DelayLine::new(0);
        } else {
            track.input_delay = DelayLine::new(root_max - subtree_latency[idx]);
            track.main_delay = DelayLine::new(return_max);
        }
    }
    for ret in returns.iter_mut() {
        ret.main_delay = DelayLine::new(return_max - node_latency(ret));
    }
}

fn is_self_or_descendant(tracks: &[Node], mut idx: usize, ancestor: usize) -> bool {
    for _ in 0..tracks.len() {
        if idx == ancestor {
            return true;
        }
        match tracks.get(idx).and_then(|n| n.parent_idx) {
            Some(parent) if parent < tracks.len() => idx = parent,
            _ => return false,
        }
    }
    false
}

fn is_solo_active(tracks: &[Node], idx: usize) -> bool {
    let mut cur = Some(idx);
    for _ in 0..tracks.len() {
        let Some(node_idx) = cur else {
            break;
        };
        if tracks[node_idx].mixer.is_soloed {
            return true;
        }
        cur = tracks[node_idx].parent_idx;
    }
    tracks
        .iter()
        .enumerate()
        .any(|(j, node)| node.mixer.is_soloed && is_self_or_descendant(tracks, j, idx))
}

fn split_track_and_parent(
    tracks: &mut [Node],
    idx: usize,
    parent: usize,
) -> (&mut Node, &mut Node) {
    if parent < idx {
        let (before, at_and_after) = tracks.split_at_mut(idx);
        (&mut at_and_after[0], &mut before[parent])
    } else {
        let (up_to_idx, after) = tracks.split_at_mut(idx + 1);
        (&mut up_to_idx[idx], &mut after[parent - idx - 1])
    }
}

fn add_track_to_parent(
    tracks: &mut [Node],
    idx: usize,
    parent: usize,
    frames: usize,
    gain: f32,
    pan_l: f32,
    pan_r: f32,
) {
    if parent == idx || parent >= tracks.len() {
        return;
    }
    let (track, parent_node) = split_track_and_parent(tracks, idx, parent);
    for frame in 0..frames {
        parent_node.left[frame] += track.left[frame] * gain * pan_l;
        parent_node.right[frame] += track.right[frame] * gain * pan_r;
    }
}

#[allow(clippy::too_many_arguments)]
fn add_track_to_parent_and_return(
    tracks: &mut [Node],
    idx: usize,
    parent: usize,
    target_ret: Option<&mut Node>,
    r_target: usize,
    frames: usize,
    gain: f32,
    pan_l: f32,
    pan_r: f32,
) {
    if parent == idx || parent >= tracks.len() {
        return;
    }
    let (track, parent_node) = split_track_and_parent(tracks, idx, parent);
    let level = track.mixer.sends.get(r_target).copied().unwrap_or(0.0);
    match target_ret {
        Some(ret) if level != 0.0 => {
            for frame in 0..frames {
                let out_l = track.left[frame] * gain * pan_l;
                let out_r = track.right[frame] * gain * pan_r;
                ret.left[frame] += out_l * level;
                ret.right[frame] += out_r * level;
                parent_node.left[frame] += out_l;
                parent_node.right[frame] += out_r;
            }
        }
        _ => {
            for frame in 0..frames {
                parent_node.left[frame] += track.left[frame] * gain * pan_l;
                parent_node.right[frame] += track.right[frame] * gain * pan_r;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn add_track_to_parent_and_returns(
    tracks: &mut [Node],
    idx: usize,
    parent: usize,
    returns: &mut [Node],
    frames: usize,
    gain: f32,
    pan_l: f32,
    pan_r: f32,
) {
    if parent == idx || parent >= tracks.len() {
        return;
    }
    let (track, parent_node) = split_track_and_parent(tracks, idx, parent);
    for frame in 0..frames {
        let out_l = track.left[frame] * gain * pan_l;
        let out_r = track.right[frame] * gain * pan_r;
        for (ret, level) in returns.iter_mut().zip(&track.mixer.sends) {
            ret.left[frame] += out_l * level;
            ret.right[frame] += out_r * level;
        }
        parent_node.left[frame] += out_l;
        parent_node.right[frame] += out_r;
    }
}

fn node_tail_samples(node: &Node) -> u64 {
    node.devices
        .iter()
        .filter(|slot| slot.enabled)
        .filter_map(|slot| slot.processor.tail_samples())
        .fold(0u64, u64::saturating_add)
}

#[derive(Clone, Copy)]
enum TargetMode {
    FullMain,
    DryTrack(usize),
    WetTrack(usize),
    DryReturn(usize),
    WetReturn(usize),
    Silent,
}

impl CompiledGraph {
    /// Assembles a graph from its nodes, which must be the tracks, then the return tracks, then the
    /// Main track. `transfers` lists the devices this graph takes over from the one it replaces, and
    /// `note_frequencies` gives the frequency of each MIDI pitch.
    pub fn new(
        mut nodes: Vec<Node>,
        track_count: usize,
        return_count: usize,
        timeline: Timeline,
        transfers: Vec<Transfer>,
        note_frequencies: [f32; 128],
    ) -> Self {
        let track_order = topological_track_order(&mut nodes, track_count);
        Self {
            nodes,
            track_count,
            return_count,
            track_order,
            stem_target: StemTarget::Main,
            tap_left: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            tap_right: vec![0.0; MAX_BLOCK].into_boxed_slice(),
            timeline,
            clock: 0.0,
            output_meter: TrackMeter::default(),
            transfers,
            note_frequencies,
        }
    }

    /// Selects which stem path `render_with_input` isolates into `output`.
    pub fn set_stem_target(&mut self, target: StemTarget) {
        self.stem_target = target;
    }

    /// The currently active stem isolation target.
    pub fn stem_target(&self) -> StemTarget {
        self.stem_target
    }

    fn resolve_mode(&self) -> TargetMode {
        let main_idx = self.track_count + self.return_count;
        match self.stem_target {
            StemTarget::Main => TargetMode::FullMain,
            StemTarget::DryNode(id) => match self.nodes.iter().position(|n| n.track == id) {
                Some(idx) if idx == main_idx => TargetMode::FullMain,
                Some(idx) if idx < self.track_count => TargetMode::DryTrack(idx),
                Some(idx) if idx < main_idx => TargetMode::DryReturn(idx - self.track_count),
                _ => TargetMode::Silent,
            },
            StemTarget::WetNode(id) => match self.nodes.iter().position(|n| n.track == id) {
                Some(idx) if idx == main_idx => TargetMode::FullMain,
                Some(idx) if idx < self.track_count => TargetMode::WetTrack(idx),
                Some(idx) if idx < main_idx => TargetMode::WetReturn(idx - self.track_count),
                _ => TargetMode::Silent,
            },
        }
    }

    /// Maximum effect tail length in samples across the nodes active under the current `StemTarget`.
    pub fn max_tail_samples(&self) -> u64 {
        let tracks = &self.nodes[..self.track_count];
        let returns = &self.nodes[self.track_count..self.track_count + self.return_count];
        let main = &self.nodes[self.nodes.len() - 1];

        let max_track_path_tail = |filter: &dyn Fn(usize) -> bool| -> u64 {
            let mut best = 0u64;
            for &idx in &self.track_order {
                if !filter(idx) {
                    continue;
                }
                let mut total = node_tail_samples(&tracks[idx]);
                let mut cur = tracks[idx].parent_idx;
                for _ in 0..tracks.len() {
                    let Some(p) = cur else {
                        break;
                    };
                    if filter(p) {
                        total = total.saturating_add(node_tail_samples(&tracks[p]));
                    }
                    cur = tracks[p].parent_idx;
                }
                best = best.max(total);
            }
            best
        };

        let returns_max = returns.iter().map(node_tail_samples).fold(0u64, u64::max);
        let main_tail = node_tail_samples(main);

        match self.resolve_mode() {
            TargetMode::FullMain => max_track_path_tail(&|_| true)
                .saturating_add(returns_max)
                .saturating_add(main_tail),
            TargetMode::DryTrack(target) => {
                max_track_path_tail(&|idx| is_self_or_descendant(tracks, idx, target))
            }
            TargetMode::WetTrack(target) => max_track_path_tail(&|idx| {
                is_self_or_descendant(tracks, idx, target)
                    || is_self_or_descendant(tracks, target, idx)
            })
            .saturating_add(returns_max)
            .saturating_add(main_tail),
            TargetMode::DryReturn(r_idx) => max_track_path_tail(&|_| true)
                .saturating_add(returns.get(r_idx).map_or(0, node_tail_samples)),
            TargetMode::WetReturn(r_idx) => max_track_path_tail(&|_| true)
                .saturating_add(returns.get(r_idx).map_or(0, node_tail_samples))
                .saturating_add(main_tail),
            TargetMode::Silent => 0,
        }
    }

    /// The frequency of each MIDI pitch in hertz. The engine reads it to turn MIDI input into notes.
    pub fn note_frequencies(&self) -> &[f32; 128] {
        &self.note_frequencies
    }

    /// Adds a note to every MIDI track that takes MIDI input now (`Node::monitors_midi`), for the block
    /// being rendered. A track whose note list is full drops the note, because the audio thread never
    /// waits.
    pub fn push_live_note(&mut self, event: NoteEvent) {
        for node in self.nodes[..self.track_count]
            .iter_mut()
            .filter(|node| node.monitors_midi())
        {
            let _ = node.events.push(event);
        }
    }

    /// Takes the processors this graph keeps from `previous`, so their state carries over, and
    /// applies the Set's parameter values to them. It moves pointers and sets parameters only, so it
    /// does not allocate, and the processors the new graph replaces end up in `previous` to be freed
    /// off the audio thread.
    pub fn adopt_state(&mut self, previous: &mut CompiledGraph) {
        let (tracks, old_tracks) = (self.track_count, previous.track_count);
        for node in &mut self.nodes[..tracks] {
            if let Some(old) = previous.nodes[..old_tracks]
                .iter_mut()
                .find(|old| old.track == node.track)
            {
                node.clips.adopt(&mut old.clips);
            }
        }
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

    /// The track nodes, in track order.
    pub fn tracks_mut(&mut self) -> &mut [Node] {
        &mut self.nodes[..self.track_count]
    }

    /// How many nodes are tracks; they come first.
    pub fn track_count(&self) -> usize {
        self.track_count
    }

    /// The timeline, and the track nodes to change while reading it.
    pub fn split_timeline(&mut self) -> (&Timeline, &mut [Node]) {
        (&self.timeline, &mut self.nodes[..self.track_count])
    }

    /// The row of a scene, if the Set has it.
    pub fn scene_row(&self, scene: SceneId) -> Option<usize> {
        self.timeline
            .scenes
            .iter()
            .position(|settings| settings.id == scene)
    }

    /// The first beat after `position` at which a track starts or stops an arrangement clip.
    pub fn next_boundary(&self, position: f64) -> Option<f64> {
        self.nodes[..self.track_count]
            .iter()
            .filter_map(|node| node.clips.next_boundary(position))
            .reduce(f64::min)
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
        self.track_order = topological_track_order(&mut self.nodes, self.track_count);
        compensate_latency(&mut self.nodes, self.track_count, self.return_count);
    }

    /// True while a continuous parameter is still ramping. The engine then renders in short steps.
    pub fn ramping(&self) -> bool {
        self.nodes
            .iter()
            .any(|node| node.devices.iter().any(|slot| !slot.ramps.is_empty()))
    }

    /// The left and right output of the last rendered block.
    pub fn output(&self, frames: usize) -> (&[f32], &[f32]) {
        let frames = frames.min(MAX_BLOCK);
        match self.stem_target {
            StemTarget::DryNode(id) if self.nodes.last().is_some_and(|main| main.track != id) => {
                (&self.tap_left[..frames], &self.tap_right[..frames])
            }
            _ => {
                let main = &self.nodes[self.nodes.len() - 1];
                (&main.left[..frames], &main.right[..frames])
            }
        }
    }

    /// Renders one block of `ctx.frames` frames (at most `MAX_BLOCK`) with no external input.
    pub fn render(&mut self, ctx: &ProcessContext) {
        self.render_with_input(ctx, InputBlock::SILENT);
    }

    /// Renders one block of `ctx.frames` frames (at most `MAX_BLOCK`) into the output buffers. `input`
    /// is the external input of the block, which the tracks that monitor it hear.
    pub fn render_with_input(&mut self, ctx: &ProcessContext, input: InputBlock<'_>) {
        let frames = ctx.frames.min(MAX_BLOCK);
        let mode = self.resolve_mode();
        let clock = self.clock;
        let CompiledGraph {
            nodes,
            track_count,
            return_count,
            track_order,
            tap_left,
            tap_right,
            output_meter,
            ..
        } = self;

        tap_left[..frames].fill(0.0);
        tap_right[..frames].fill(0.0);

        let (tracks, mixing) = nodes.split_at_mut(*track_count);
        let (returns, main) = mixing.split_at_mut(*return_count);
        let main = &mut main[0];

        for track in tracks.iter_mut() {
            track.clear(frames);
        }
        for ret in returns.iter_mut() {
            ret.clear(frames);
        }
        main.clear(frames);

        if matches!(mode, TargetMode::Silent) {
            return;
        }

        let any_solo = tracks.iter().any(|node| node.mixer.is_soloed);
        let has_groups = tracks.iter().any(|node| node.parent_idx.is_some());
        let block_ctx = ProcessContext { frames, ..*ctx };

        for &idx in track_order.iter() {
            let should_run = match mode {
                TargetMode::FullMain | TargetMode::DryReturn(_) | TargetMode::WetReturn(_) => true,
                TargetMode::DryTrack(target) => is_self_or_descendant(tracks, idx, target),
                TargetMode::WetTrack(target) => {
                    is_self_or_descendant(tracks, idx, target)
                        || is_self_or_descendant(tracks, target, idx)
                }
                TargetMode::Silent => false,
            };
            if !should_run {
                continue;
            }

            let audible = match mode {
                TargetMode::DryTrack(_) | TargetMode::WetTrack(_) => !tracks[idx].mixer.is_muted,
                TargetMode::FullMain | TargetMode::DryReturn(_) | TargetMode::WetReturn(_) => {
                    let solo_ok = if !any_solo {
                        true
                    } else if has_groups {
                        is_solo_active(tracks, idx)
                    } else {
                        tracks[idx].mixer.is_soloed
                    };
                    !tracks[idx].mixer.is_muted && solo_ok
                }
                TargetMode::Silent => false,
            };

            let parent_idx = tracks[idx].parent_idx;
            {
                let track = &mut tracks[idx];
                track.schedule_clips(&block_ctx, clock);
                track.monitor_input(&input, frames);
                track.run_devices(&block_ctx);
                track.delay_input(frames);
            }

            // A muted or unsoloed track is silent, but it still runs its delay so later blocks line up.
            let gain = if audible {
                volume_gain(tracks[idx].mixer.volume_db)
            } else {
                0.0
            };
            let (pan_l, pan_r) = pan_gains(tracks[idx].mixer.pan);

            match mode {
                TargetMode::DryTrack(target) if idx == target => {
                    let track = &tracks[idx];
                    for frame in 0..frames {
                        let out_l = track.left[frame] * gain * pan_l;
                        let out_r = track.right[frame] * gain * pan_r;
                        tap_left[frame] = out_l;
                        tap_right[frame] = out_r;
                        output_meter.update(out_l, out_r);
                    }
                }
                TargetMode::DryTrack(_) => {
                    if let Some(p) = parent_idx {
                        add_track_to_parent(tracks, idx, p, frames, gain, pan_l, pan_r);
                    }
                }
                TargetMode::DryReturn(r_target) | TargetMode::WetReturn(r_target) => {
                    if let Some(p) = parent_idx {
                        add_track_to_parent_and_return(
                            tracks,
                            idx,
                            p,
                            returns.get_mut(r_target),
                            r_target,
                            frames,
                            gain,
                            pan_l,
                            pan_r,
                        );
                    } else if let Some(ret) = returns.get_mut(r_target) {
                        let track = &tracks[idx];
                        let level = track.mixer.sends.get(r_target).copied().unwrap_or(0.0);
                        if level != 0.0 {
                            for frame in 0..frames {
                                let out_l = track.left[frame] * gain * pan_l;
                                let out_r = track.right[frame] * gain * pan_r;
                                ret.left[frame] += out_l * level;
                                ret.right[frame] += out_r * level;
                            }
                        }
                    }
                }
                TargetMode::FullMain | TargetMode::WetTrack(_) => {
                    if let Some(p) = parent_idx {
                        add_track_to_parent_and_returns(
                            tracks, idx, p, returns, frames, gain, pan_l, pan_r,
                        );
                    } else {
                        let track = &mut tracks[idx];
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
                }
                TargetMode::Silent => {}
            }
        }

        match mode {
            TargetMode::DryTrack(_) | TargetMode::Silent => return,
            TargetMode::DryReturn(r_target) => {
                if let Some(ret) = returns.get_mut(r_target) {
                    ret.run_devices(&block_ctx);
                    let gain = if ret.mixer.is_muted {
                        0.0
                    } else {
                        volume_gain(ret.mixer.volume_db)
                    };
                    for frame in 0..frames {
                        let out_l = ret.left[frame] * gain;
                        let out_r = ret.right[frame] * gain;
                        tap_left[frame] = out_l;
                        tap_right[frame] = out_r;
                        output_meter.update(out_l, out_r);
                    }
                }
                return;
            }
            TargetMode::WetReturn(r_target) => {
                if let Some(ret) = returns.get_mut(r_target) {
                    ret.run_devices(&block_ctx);
                    let gain = if ret.mixer.is_muted {
                        0.0
                    } else {
                        volume_gain(ret.mixer.volume_db)
                    };
                    for frame in 0..frames {
                        let (ret_l, ret_r) =
                            ret.main_delay.process(ret.left[frame], ret.right[frame]);
                        main.left[frame] += ret_l * gain;
                        main.right[frame] += ret_r * gain;
                    }
                }
            }
            TargetMode::FullMain | TargetMode::WetTrack(_) => {
                // Returns add their volume to Main without panning, as the mixer did before M05.
                for ret in returns.iter_mut() {
                    ret.run_devices(&block_ctx);
                    let gain = if ret.mixer.is_muted {
                        0.0
                    } else {
                        volume_gain(ret.mixer.volume_db)
                    };
                    for frame in 0..frames {
                        let (ret_l, ret_r) =
                            ret.main_delay.process(ret.left[frame], ret.right[frame]);
                        main.left[frame] += ret_l * gain;
                        main.right[frame] += ret_r * gain;
                    }
                }
            }
        }

        main.run_devices(&block_ctx);
        let gain = if main.mixer.is_muted {
            0.0
        } else {
            volume_gain(main.mixer.volume_db)
        };
        for frame in 0..frames {
            main.left[frame] *= gain;
            main.right[frame] *= gain;
            output_meter.update(main.left[frame], main.right[frame]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::GraphCompiler;
    use basedrop::Collector;
    use livecraft_model::ids::{DeviceId, TrackId};
    use livecraft_model::model::{
        Clip, ClipPayload, Device, DeviceType, MidiNote, Project, Track, TrackType,
    };

    fn ctx(frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: 44100.0,
            frames,
            bpm: 120.0,
            playhead_beats: 0.0,
            beats_per_frame: 120.0 / 60.0 / 44100.0,
        }
    }

    fn note_clip(id: u64, pitch: u8) -> Clip {
        let mut clip = Clip::new_midi(id, "Note", 4.0);
        if let ClipPayload::Midi(ref mut mdata) = clip.payload {
            mdata.notes.push(MidiNote::new(pitch, 0.0, 2.0, 100));
        }
        clip
    }

    #[test]
    fn group_tracks_sum_children_in_topological_order_even_when_group_comes_first() {
        let mut project = Project::new_default("Groups");
        project.main_track.device_chain.clear();

        let group_id = TrackId(10);
        let mut group = Track::new(group_id, "Bus", TrackType::Group, 0);
        // Attenuate group by -6 dB so its output differs from raw sum.
        group.volume_db = -6.0;

        let mut child = Track::new(TrackId(1), "Synth", TrackType::Midi, 4);
        child.parent = Some(group_id);
        child
            .device_chain
            .push(Device::new(DeviceId(101), "Operator", DeviceType::Operator));
        child.clip_slots[0].clip = Some(note_clip(901, 60));

        // Put Group BEFORE its child to verify topological ordering.
        project.tracks = vec![group, child];

        let collector = Collector::new();
        let mut graph = GraphCompiler::new(44100.0, collector.handle()).compile(&project);
        graph.node_mut(TrackId(1)).expect("child").clips.launch(0);
        graph.render(&ctx(MAX_BLOCK));
        let (main_l, _) = graph.output(MAX_BLOCK);
        let main_peak = main_l.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(main_peak > 0.01, "group should sum child audio");

        // Dry child output should be ~2x louder (+6 dB) than the -6 dB group output.
        let mut dry_graph = GraphCompiler::new(44100.0, collector.handle()).compile(&project);
        dry_graph
            .node_mut(TrackId(1))
            .expect("child")
            .clips
            .launch(0);
        dry_graph.set_stem_target(StemTarget::DryNode(TrackId(1)));
        dry_graph.render(&ctx(MAX_BLOCK));
        let (dry_l, _) = dry_graph.output(MAX_BLOCK);
        let dry_peak = dry_l.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        let ratio = dry_peak / main_peak;
        let expected_ratio = 10.0f32.powf(6.0 / 20.0) / pan_gains(0.0).0;
        assert!(
            (ratio - expected_ratio).abs() < 0.05,
            "expected ~{expected_ratio} ratio, got {ratio}"
        );
    }

    #[test]
    fn stem_target_isolates_dry_and_wet_tracks_and_returns() {
        let mut project = Project::new_default("StemIsolation");
        project.tracks[0].clip_slots[0].clip = Some(note_clip(901, 60));
        // Track 1 sends to Return A (Reverb), Main has Limiter + we add Delay on Return A.
        project.tracks[0].sends = vec![1.0, 0.0];
        project.return_tracks[0].device_chain =
            vec![Device::new(DeviceId(701), "Delay", DeviceType::Delay)];
        project.return_tracks[0].device_chain[0].set_param("dry_wet", 1.0);
        project.main_track.device_chain =
            vec![Device::new(DeviceId(702), "Utility", DeviceType::Utility)];
        project.main_track.device_chain[0].set_param("gain_db", -6.0);

        // Track 2 also plays on a second synth so we verify isolation excludes it.
        project.tracks[1].device_chain.push(Device::new(
            DeviceId(703),
            "Operator",
            DeviceType::Operator,
        ));
        project.tracks[1].clip_slots[0].clip = Some(note_clip(902, 72));
        project.tracks[1].sends = vec![0.0, 0.0];

        let collector = Collector::new();
        let compile_launched = |target: StemTarget| {
            let mut graph = GraphCompiler::new(44100.0, collector.handle()).compile(&project);
            graph.node_mut(TrackId(1)).expect("t1").clips.launch(0);
            graph.node_mut(TrackId(2)).expect("t2").clips.launch(0);
            graph.set_stem_target(target);
            graph
        };

        // DryNode(Track 1) has NO Main -6 dB attenuation and NO Track 2.
        let mut dry_t1 = compile_launched(StemTarget::DryNode(TrackId(1)));
        assert_eq!(dry_t1.max_tail_samples(), 0);
        dry_t1.render(&ctx(MAX_BLOCK));
        let dry_peak = dry_t1
            .output(MAX_BLOCK)
            .0
            .iter()
            .fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(dry_peak > 0.01);

        // WetNode(Track 1) goes through Main -6 dB Utility (peak ~half of dry in first block before delay).
        let mut wet_t1 = compile_launched(StemTarget::WetNode(TrackId(1)));
        assert!(wet_t1.max_tail_samples() > 0);
        wet_t1.render(&ctx(MAX_BLOCK));
        let wet_peak = wet_t1
            .output(MAX_BLOCK)
            .0
            .iter()
            .fold(0.0f32, |m, &s| m.max(s.abs()));
        let expected_ratio = 10.0f32.powf(6.0 / 20.0) / pan_gains(0.0).0;
        assert!((dry_peak / wet_peak - expected_ratio).abs() < 0.05);

        // DryNode(Return B) receives zero sends -> silent.
        let mut dry_ret_b = compile_launched(StemTarget::DryNode(project.return_tracks[1].id));
        dry_ret_b.render(&ctx(MAX_BLOCK));
        assert!(dry_ret_b.output(MAX_BLOCK).0.iter().all(|&s| s == 0.0));
    }
}
