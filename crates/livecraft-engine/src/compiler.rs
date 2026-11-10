//! Compiles a Live Set into a `CompiledGraph` (M05 items 5 and 7). This runs on the session side,
//! never on the audio thread: it allocates the processors and buffers the audio thread then uses,
//! and it decodes the audio files the clips play, so the audio thread never reads a file.

use crate::audio_clip::{DecodedAudio, load_file};
use crate::clip::{ArrangedClip, ClipLane, PlayNote, PlayableClip, PlayableSource};
use crate::compiled::{
    CompiledGraph, DeviceSlot, InputRoute, MAX_NOTE_EVENTS, MidiOutRoute, Mixer, Node, NodeKind,
    Transfer, compensate_latency,
};
use crate::transport::Timeline;
use basedrop::{Handle, Shared};
use livecraft_devices::{DeviceProcessor, create_device_processor, is_instrument};
use livecraft_formats::project_dir::{
    ProjectContext, SampleReference, SampleResolution, resolve_sample_reference,
};
use livecraft_model::device::param_table;
use livecraft_model::ids::{DeviceId, TrackId};
use livecraft_model::model::{
    AudioInput, Clip, ClipPayload, Device, MidiClipData, MidiOutput, MonitoringMode, Project,
    Track, TrackType, TuningSystem,
};
use livecraft_model::param::ParamId;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Builds the processor of a device, or None when the device has no native processor.
pub type ProcessorFactory = fn(&Device, f32) -> Option<Box<dyn DeviceProcessor>>;

/// Compiles Sets into graphs. It remembers the layout of the last graph it compiled, so each new
/// graph lists the devices it takes over, and it keeps the decoded audio, so a file is read once.
/// Compile and submit graphs in the same order.
pub struct GraphCompiler {
    sample_rate: f32,
    /// Shares the clip data with the audio thread. Freed by the collector thread.
    handle: Handle,
    factory: ProcessorFactory,
    /// Project directory context used to resolve relative audio clip references.
    project_context: ProjectContext,
    /// The track and device ids of each node of the last graph, in node order.
    previous: Vec<(TrackId, Vec<DeviceId>)>,
    /// The latency each device had in the last graph, in frames.
    latencies: HashMap<DeviceId, usize>,
    /// Decoded audio by canonical resolved path. A file that fails to decode is not cached, so it is retried.
    audio: HashMap<PathBuf, Shared<DecodedAudio>>,
    /// Missing sample references already logged, so repeated compiles do not spam warnings.
    warned_missing: HashSet<SampleReference>,
}

impl GraphCompiler {
    /// `handle` comes from the collector that the engine runs (see `Engine::start`).
    pub fn new(sample_rate: f32, handle: Handle) -> Self {
        Self::with_factory(sample_rate, handle, create_device_processor)
    }

    /// As `new`, with each device's processor built by `factory`.
    pub fn with_factory(sample_rate: f32, handle: Handle, factory: ProcessorFactory) -> Self {
        Self {
            sample_rate,
            handle,
            factory,
            project_context: ProjectContext::default(),
            previous: Vec::new(),
            latencies: HashMap::new(),
            audio: HashMap::new(),
            warned_missing: HashSet::new(),
        }
    }

    /// Sets the `ProjectContext` used to resolve sample references on subsequent compiles.
    pub fn set_project_context(&mut self, context: ProjectContext) {
        if self.project_context != context {
            self.project_context = context;
            self.warned_missing.clear();
        }
    }

    /// Builder variant of [`Self::set_project_context`].
    #[must_use]
    pub fn with_project_context(mut self, context: ProjectContext) -> Self {
        self.project_context = context;
        self
    }

    /// Returns the active `ProjectContext` used to resolve sample references.
    #[must_use]
    pub fn project_context(&self) -> &ProjectContext {
        &self.project_context
    }

    /// Compiles the Set into the nodes the renderer runs. A device without a native processor makes
    /// no sound, so it is left out of the chain. A clip whose audio file cannot be read plays nothing.
    pub fn compile(&mut self, project: &Project) -> CompiledGraph {
        let tuning = project.tuning.as_ref();
        let mut nodes = Vec::with_capacity(project.all_tracks().count());
        for track in &project.tracks {
            nodes.push(self.node(track, NodeKind::Track, tuning));
        }
        for (i, track) in project.tracks.iter().enumerate() {
            if let Some(parent_id) = track.parent
                && parent_id != track.id
            {
                nodes[i].parent_idx = project
                    .tracks
                    .iter()
                    .position(|t| t.id == parent_id && t.track_type == TrackType::Group);
            }
        }
        for track in &project.return_tracks {
            nodes.push(self.node(track, NodeKind::Return, tuning));
        }
        nodes.push(self.node(&project.main_track, NodeKind::Main, tuning));

        compensate_latency(
            &mut nodes,
            project.tracks.len(),
            project.return_tracks.len(),
        );
        self.latencies = nodes
            .iter()
            .flat_map(|node| node.devices.iter().map(|slot| (slot.id, slot.latency)))
            .collect();
        let transfers = self.transfers(&nodes);
        self.previous = nodes
            .iter()
            .map(|node| {
                (
                    node.track,
                    node.devices.iter().map(|slot| slot.id).collect(),
                )
            })
            .collect();
        let note_frequencies = std::array::from_fn(|pitch| note_frequency(pitch as u8, tuning));
        CompiledGraph::new(
            nodes,
            project.tracks.len(),
            project.return_tracks.len(),
            Timeline::of(project),
            transfers,
            note_frequencies,
        )
    }

    /// True when `device` now reports a different latency from the one the last graph compiled. The
    /// compensation of a Set is worked out at compile time, so the session recompiles when this holds
    /// (manual 23.3). A device the last graph did not have counts as zero latency.
    pub fn latency_changed(&self, device: &Device) -> bool {
        let compiled = self.latencies.get(&device.id).copied().unwrap_or(0);
        let now = (self.factory)(device, self.sample_rate)
            .map_or(0, |processor| processor.latency_samples() as usize);
        now != compiled
    }

    /// Every device that the previous graph also had, paired with its old and new position.
    fn transfers(&self, nodes: &[Node]) -> Vec<Transfer> {
        let mut transfers = Vec::new();
        for (to_node, node) in nodes.iter().enumerate() {
            let Some(from_node) = self
                .previous
                .iter()
                .position(|(track, _)| *track == node.track)
            else {
                continue;
            };
            let previous_devices = &self.previous[from_node].1;
            for (to_slot, slot) in node.devices.iter().enumerate() {
                if let Some(from_slot) = previous_devices.iter().position(|id| *id == slot.id) {
                    transfers.push(Transfer {
                        from: (from_node, from_slot),
                        to: (to_node, to_slot),
                    });
                }
            }
        }
        transfers
    }

    fn node(&mut self, track: &Track, kind: NodeKind, tuning: Option<&TuningSystem>) -> Node {
        let mut chain: Vec<&Device> = track.device_chain.iter().collect();
        // Instruments come first, so they take the note events before the effects process them.
        chain.sort_by_key(|device| !is_instrument(&device.device_type));
        let devices = chain
            .into_iter()
            .filter_map(|device| {
                (self.factory)(device, self.sample_rate).map(|processor| {
                    DeviceSlot::new(
                        device.id,
                        device.is_enabled,
                        processor,
                        parameter_values(device),
                        param_table(&device.device_type),
                    )
                })
            })
            .collect();
        let mixer = Mixer {
            volume_db: track.volume_db,
            pan: track.pan,
            is_muted: track.is_muted,
            is_soloed: track.is_soloed,
            sends: match kind {
                NodeKind::Track => track.sends.clone(),
                NodeKind::Return | NodeKind::Main => Vec::new(),
            },
        };
        let clips = match kind {
            NodeKind::Track if track.track_type != TrackType::Group => self.lane(track, tuning),
            NodeKind::Track | NodeKind::Return | NodeKind::Main => {
                ClipLane::new(Vec::new(), Vec::new(), false, 0)
            }
        };
        let input = InputRoute {
            channel: match (kind, track.routing.audio_from) {
                (NodeKind::Track, AudioInput::ExternalIn { channel }) => Some(channel),
                _ => None,
            },
            monitoring: track.monitoring,
            is_armed: track.is_armed,
            takes_midi: kind == NodeKind::Track && track.track_type == TrackType::Midi,
        };
        let midi_out = match track.routing.midi_to {
            MidiOutput::External { port, channel }
                if kind == NodeKind::Track && track.track_type == TrackType::Midi =>
            {
                Some(MidiOutRoute { port, channel })
            }
            _ => None,
        };
        Node::new(track.id, kind, mixer, devices, clips, input, midi_out)
    }

    /// The clips a track can play. A track monitoring its input rather than playing clips (manual
    /// 18.4 and the Set's monitor setting) plays none.
    fn lane(&mut self, track: &Track, tuning: Option<&TuningSystem>) -> ClipLane {
        let slots = track
            .clip_slots
            .iter()
            .map(|slot| {
                slot.clip
                    .as_ref()
                    .and_then(|clip| self.playable(clip, tuning))
            })
            .collect();
        let arranged = track
            .arrangement_clips
            .iter()
            .filter_map(|placed| {
                self.playable(&placed.clip, tuning)
                    .map(|clip| ArrangedClip {
                        start_beat: placed.start_beat,
                        length_beats: placed.length_beats,
                        clip,
                    })
            })
            .collect();
        let plays_clips = match track.monitoring {
            MonitoringMode::In => false,
            MonitoringMode::Auto => !track.is_armed,
            MonitoringMode::Off => true,
        };
        let mut lane = ClipLane::new(slots, arranged, plays_clips, MAX_NOTE_EVENTS);
        lane.stop_buttons = track
            .clip_slots
            .iter()
            .map(|slot| slot.has_stop_button)
            .collect();
        lane
    }

    /// A clip prepared for playback, or None when its audio cannot be read.
    fn playable(&mut self, clip: &Clip, tuning: Option<&TuningSystem>) -> Option<PlayableClip> {
        let source = match &clip.payload {
            ClipPayload::Midi(midi) => {
                let notes = play_notes(midi, tuning);
                PlayableSource::Midi(Shared::new(&self.handle, notes))
            }
            ClipPayload::Audio(audio) => PlayableSource::Audio {
                data: self.decoded(&audio.file_path, audio.relative_path.as_deref())?,
                gain: 10.0f32.powf(audio.gain_db / 20.0),
                is_warped: audio.is_warped,
                original_bpm: audio.original_bpm,
            },
        };
        Some(PlayableClip {
            length_beats: clip.length_beats,
            is_looping: clip.is_looping,
            source,
            launch: clip.launch,
            follow: clip.follow,
        })
    }

    /// The decoded audio of a file, resolved with `project_context`, read once and shared.
    fn decoded(
        &mut self,
        file_path: &str,
        relative_path: Option<&str>,
    ) -> Option<Shared<DecodedAudio>> {
        if file_path.trim().is_empty() && relative_path.is_none_or(|rel| rel.trim().is_empty()) {
            return None;
        }
        let reference = SampleReference {
            path: file_path.to_owned(),
            relative_path: relative_path.map(str::to_owned),
        };
        let resolved_path = match resolve_sample_reference(&reference, &self.project_context) {
            SampleResolution::Resolved { resolved_path, .. } => {
                self.warned_missing.remove(&reference);
                resolved_path
            }
            SampleResolution::Missing { .. } => {
                if self.warned_missing.insert(reference.clone()) {
                    log::warn!(
                        "Audio clip file {} cannot be read, so the clip is silent",
                        reference.display_ref()
                    );
                }
                return None;
            }
        };
        let cache_key = canonical_cache_key(&resolved_path);
        if let Some(data) = self.audio.get(&cache_key) {
            return Some(data.clone());
        }
        match load_file(&resolved_path) {
            Ok(decoded) => {
                let data = Shared::new(&self.handle, decoded);
                self.audio.insert(cache_key, data.clone());
                Some(data)
            }
            Err(error) => {
                log::warn!(
                    "Audio clip file {} cannot be read, so the clip is silent: {error}",
                    resolved_path.display()
                );
                None
            }
        }
    }
}

fn canonical_cache_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The largest parameter table of any device (EQ Eight's 32 values).
const MAX_PARAMETERS: usize = 32;

/// The stored values of a device, with room for its whole table, so a parameter command adds to them
/// without allocating on the audio thread.
fn parameter_values(device: &Device) -> Vec<(ParamId, f32)> {
    let mut values = Vec::with_capacity(MAX_PARAMETERS);
    values.extend(device.parameters.iter());
    values
}

/// The notes a MIDI clip plays: muted notes are left out, and each frequency comes from the Set's
/// tuning, or from equal temperament at A4 = 440 Hz.
fn play_notes(midi: &MidiClipData, tuning: Option<&TuningSystem>) -> Vec<PlayNote> {
    midi.notes
        .iter()
        .filter(|note| !note.mute)
        .map(|note| PlayNote {
            start: note.start_beat,
            end: note.start_beat + note.duration_beats,
            pitch: note.pitch,
            velocity: note.velocity.round().clamp(0.0, 127.0) as u8,
            freq_hz: note_frequency(note.pitch, tuning),
        })
        .collect()
}

/// The frequency of a MIDI pitch in hertz, from the Set's tuning or from equal temperament at A4 = 440 Hz.
fn note_frequency(pitch: u8, tuning: Option<&TuningSystem>) -> f32 {
    match tuning {
        Some(tuning) => tuning.note_to_freq(pitch) as f32,
        None => 440.0 * 2.0f32.powf((f32::from(pitch) - 69.0) / 12.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiled::MAX_BLOCK;
    use basedrop::Collector;
    use livecraft_devices::{NoteEvent, ProcessContext};
    use livecraft_model::ids::{DeviceId, TrackId};
    use livecraft_model::model::{Device, DeviceType, MidiNote, MidiOutput, Track, TrackType};

    /// A compiler and the collector that frees its shared data. The collector must outlive the graphs.
    fn compiler(collector: &Collector) -> GraphCompiler {
        GraphCompiler::new(44100.0, collector.handle())
    }

    fn compile(project: &Project) -> (Collector, CompiledGraph) {
        let collector = Collector::new();
        let graph = compiler(&collector).compile(project);
        (collector, graph)
    }

    fn context(frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: 44100.0,
            frames,
            bpm: 120.0,
            playhead_beats: 0.0,
            beats_per_frame: 0.0,
        }
    }

    #[test]
    fn the_nodes_are_tracks_then_returns_then_main() {
        let project = Project::new_default("Order");
        let (_collector, graph) = compile(&project);
        let kinds: Vec<NodeKind> = graph.nodes().iter().map(|node| node.kind).collect();
        let expected: Vec<NodeKind> = std::iter::repeat_n(NodeKind::Track, project.tracks.len())
            .chain(std::iter::repeat_n(
                NodeKind::Return,
                project.return_tracks.len(),
            ))
            .chain(std::iter::once(NodeKind::Main))
            .collect();
        assert_eq!(kinds, expected);
    }

    #[test]
    fn an_instrument_is_placed_before_the_effects() {
        let mut project = Project::new_default("Chain");
        project.tracks[0].device_chain.insert(
            0,
            Device::new(DeviceId(500), "Utility", DeviceType::Utility),
        );
        let (_collector, graph) = compile(&project);
        let track = graph
            .nodes()
            .iter()
            .find(|node| node.track == TrackId(1))
            .expect("track 1");
        assert_eq!(track.devices[0].id, DeviceId(101));
        assert_eq!(track.devices[1].id, DeviceId(500));
    }

    #[test]
    fn a_silent_set_renders_silence() {
        let (_collector, mut graph) = compile(&Project::new_default("Silent"));
        graph.render(&context(MAX_BLOCK));
        let (left, right) = graph.output(MAX_BLOCK);
        assert!(left.iter().chain(right).all(|&sample| sample == 0.0));
    }

    #[test]
    fn a_launched_midi_clip_makes_its_instrument_sound() {
        let mut project = Project::new_default("Clip");
        let mut clip = Clip::new_midi(900, "Chord", 4.0);
        if let ClipPayload::Midi(ref mut data) = clip.payload {
            data.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        }
        project.tracks[0].clip_slots[0].clip = Some(clip);
        let (_collector, mut graph) = compile(&project);
        graph.node_mut(TrackId(1)).expect("track 1").clips.launch(0);
        let mut playing = context(MAX_BLOCK);
        playing.beats_per_frame = 120.0 / 60.0 / 44100.0;
        graph.render(&playing);
        let (left, _) = graph.output(MAX_BLOCK);
        assert!(left.iter().any(|&sample| sample != 0.0));
    }

    #[test]
    fn a_note_pushed_into_the_block_makes_its_instrument_sound() {
        let (_collector, mut graph) = compile(&Project::new_default("Note"));
        let track = graph.node_mut(TrackId(1)).expect("track 1");
        assert!(track.events.push(NoteEvent::On {
            offset: 0,
            pitch: 60,
            velocity: 100,
            freq_hz: 261.6,
        }));
        graph.render(&context(MAX_BLOCK));
        let (left, _) = graph.output(MAX_BLOCK);
        assert!(left.iter().any(|&sample| sample != 0.0));
    }

    #[test]
    fn note_events_belong_to_the_block_that_rendered_them() {
        let (_collector, mut graph) = compile(&Project::new_default("Events"));
        let track = graph.node_mut(TrackId(1)).expect("track 1");
        assert!(track.events.push(NoteEvent::On {
            offset: 0,
            pitch: 60,
            velocity: 100,
            freq_hz: 261.6,
        }));
        graph.render(&context(MAX_BLOCK));
        assert!(graph.nodes()[0].events.is_empty());
    }

    #[test]
    fn rendering_the_same_graph_twice_gives_the_same_bits() {
        let render = || {
            let (_collector, mut graph) = compile(&Project::new_default("Repeat"));
            let track = graph.node_mut(TrackId(1)).expect("track 1");
            assert!(track.events.push(NoteEvent::On {
                offset: 3,
                pitch: 64,
                velocity: 90,
                freq_hz: 329.6,
            }));
            let mut out = Vec::new();
            for _ in 0..4 {
                graph.render(&context(MAX_BLOCK));
                out.extend_from_slice(graph.output(MAX_BLOCK).0);
                graph.node_mut(TrackId(1)).expect("track 1").events.clear();
            }
            out
        };
        let first: Vec<u32> = render().iter().map(|s| s.to_bits()).collect();
        let second: Vec<u32> = render().iter().map(|s| s.to_bits()).collect();
        assert_eq!(first, second);
    }

    #[test]
    fn a_missing_audio_file_leaves_its_clip_silent() {
        let mut project = Project::new_default("Missing");
        project.tracks[0].clip_slots[0].clip =
            Some(Clip::new_audio(901, "Missing", "does-not-exist.wav", 4.0));
        let (_collector, mut graph) = compile(&project);
        graph.node_mut(TrackId(1)).expect("track 1").clips.launch(0);
        graph.render(&context(MAX_BLOCK));
        let (left, _) = graph.output(MAX_BLOCK);
        assert!(left.iter().all(|&sample| sample == 0.0));
    }

    #[test]
    fn a_track_added_or_removed_changes_the_node_count_of_the_next_compile() {
        let mut project = Project::new_default("Sync");
        let (_collector, before) = compile(&project);
        project
            .tracks
            .push(Track::new(TrackId(5), "Track 5", TrackType::Audio, 4));
        project
            .tracks
            .push(Track::new(TrackId(6), "Track 6", TrackType::Midi, 4));
        let (_collector, grown) = compile(&project);
        assert_eq!(grown.nodes().len(), before.nodes().len() + 2);

        project.tracks.pop();
        let (_collector, shrunk) = compile(&project);
        assert_eq!(shrunk.nodes().len(), before.nodes().len() + 1);
    }

    #[test]
    fn the_devices_of_a_chain_compile_in_chain_order() {
        let device_ids = |graph: &CompiledGraph| -> Vec<DeviceId> {
            graph
                .nodes()
                .iter()
                .find(|node| node.track == TrackId(2))
                .expect("track 2")
                .devices
                .iter()
                .map(|slot| slot.id)
                .collect()
        };
        let mut project = Project::new_default("ChainOrder");
        project.tracks[1].device_chain = vec![
            Device::new(DeviceId(101), "Delay 1", DeviceType::Delay),
            Device::new(DeviceId(102), "Delay 2", DeviceType::Delay),
            Device::new(DeviceId(103), "Compressor 1", DeviceType::Compressor),
        ];
        let (_collector, graph) = compile(&project);
        assert_eq!(
            device_ids(&graph),
            vec![DeviceId(101), DeviceId(102), DeviceId(103)]
        );

        project.tracks[1].device_chain.reverse();
        let (_collector, reversed) = compile(&project);
        assert_eq!(
            device_ids(&reversed),
            vec![DeviceId(103), DeviceId(102), DeviceId(101)]
        );
    }

    #[test]
    fn a_track_takes_its_input_channel_and_monitor_mode_from_the_set() {
        let mut project = Project::new_default("Input");
        let mut input = Track::new(TrackId(8), "Input", TrackType::Audio, 0);
        input.routing.audio_from = AudioInput::ExternalIn { channel: 1 };
        input.monitoring = MonitoringMode::In;
        input.is_armed = true;
        project.tracks.push(input);
        let (_collector, graph) = compile(&project);
        let route = graph
            .nodes()
            .iter()
            .find(|node| node.track == TrackId(8))
            .expect("track 8")
            .input;
        assert_eq!(
            route,
            InputRoute {
                channel: Some(1),
                monitoring: MonitoringMode::In,
                is_armed: true,
                takes_midi: false,
            }
        );
    }

    #[test]
    fn a_midi_track_takes_midi_input_and_an_audio_track_does_not() {
        let (_collector, graph) = compile(&Project::new_default("Midi"));
        let takes_midi = |track: TrackId| {
            graph
                .nodes()
                .iter()
                .find(|node| node.track == track)
                .expect("the node is in the graph")
                .input
                .takes_midi
        };
        assert!(takes_midi(TrackId(1)));
        assert!(takes_midi(TrackId(2)));
        assert!(!takes_midi(TrackId(3)));
        assert!(!takes_midi(TrackId(5)));
        assert!(!takes_midi(TrackId(7)));
    }

    #[test]
    fn only_a_midi_track_sends_its_notes_to_the_external_port_it_is_routed_to() {
        let mut project = Project::new_default("MIDI out");
        let mut route_track = |id: u64, midi_to: MidiOutput| {
            project
                .tracks
                .iter_mut()
                .find(|track| track.id == TrackId(id))
                .expect("the track is in the Set")
                .routing
                .midi_to = midi_to;
        };
        route_track(
            1,
            MidiOutput::External {
                port: 2,
                channel: 9,
            },
        );
        route_track(
            3,
            MidiOutput::External {
                port: 0,
                channel: 0,
            },
        );
        let (_collector, graph) = compile(&project);
        let route = |track: TrackId| {
            graph
                .nodes()
                .iter()
                .find(|node| node.track == track)
                .expect("the node is in the graph")
                .midi_out
        };
        assert_eq!(
            route(TrackId(1)),
            Some(crate::compiled::MidiOutRoute {
                port: 2,
                channel: 9,
            })
        );
        assert_eq!(route(TrackId(2)), None);
        // Track 3 is an audio track, which has no MIDI output.
        assert_eq!(route(TrackId(3)), None);
    }

    #[test]
    fn the_note_frequencies_follow_the_sets_tuning() {
        let mut project = Project::new_default("Tuning");
        let (_collector, graph) = compile(&project);
        assert!((graph.note_frequencies()[69] - 440.0).abs() < 1e-3);

        project.tuning = Some(TuningSystem {
            base_freq_hz: 432.0,
            ..TuningSystem::equal_temperament_12()
        });
        let (_collector, tuned) = compile(&project);
        assert!((tuned.note_frequencies()[69] - 432.0).abs() < 1e-3);
        // Twelve pitches up is one octave, which doubles the frequency.
        assert!((tuned.note_frequencies()[81] - 864.0).abs() < 1e-2);
    }

    #[test]
    fn only_a_track_takes_an_external_input_channel() {
        let mut project = Project::new_default("Routes");
        project.return_tracks[0].routing.audio_from = AudioInput::ExternalIn { channel: 0 };
        let (_collector, graph) = compile(&project);
        let channel_of = |track: TrackId| {
            graph
                .nodes()
                .iter()
                .find(|node| node.track == track)
                .expect("the node is in the graph")
                .input
                .channel
        };
        assert_eq!(channel_of(TrackId(1)), None);
        assert_eq!(channel_of(TrackId(5)), None);
        assert_eq!(channel_of(TrackId(7)), None);
    }

    fn write_constant_wav(path: &Path, amplitude: f32, frames: usize) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create wav parent");
        }
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
        for _ in 0..frames {
            writer.write_sample(amplitude).expect("write L");
            writer.write_sample(amplitude).expect("write R");
        }
        writer.finalize().expect("finalize wav");
    }

    fn playing_context(frames: usize) -> ProcessContext {
        ProcessContext {
            sample_rate: 44100.0,
            frames,
            bpm: 120.0,
            playhead_beats: 0.0,
            beats_per_frame: 120.0 / 60.0 / 44100.0,
        }
    }

    #[test]
    fn broken_absolute_path_with_valid_relative_path_resolves_via_project_context() {
        let dir =
            std::env::temp_dir().join(format!("livecraft_compiler_rel_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root = dir.join("Song Project");
        livecraft_formats::project_dir::create_project_layout(&root).expect("layout");
        let wav_path = root.join("Samples/Imported/tone.wav");
        write_constant_wav(&wav_path, 0.5, 4096);

        let mut project = Project::new_default("RelResolve");
        project.tracks[2].device_chain.clear();
        project.main_track.device_chain.clear();
        let mut clip = Clip::new_audio(
            901,
            "Tone",
            "C:/definitely_nonexistent_dir_12345/Samples/Imported/tone.wav",
            4.0,
        );
        if let ClipPayload::Audio(ref mut audio) = clip.payload {
            audio.relative_path = Some("Samples/Imported/tone.wav".to_string());
        }
        project.tracks[2].clip_slots[0].clip = Some(clip);

        let collector = Collector::new();
        let ctx = ProjectContext::for_project(&root);
        let mut compiler =
            GraphCompiler::new(44100.0, collector.handle()).with_project_context(ctx.clone());
        assert_eq!(compiler.project_context(), &ctx);

        let mut graph = compiler.compile(&project);
        graph.node_mut(TrackId(3)).expect("track 3").clips.launch(0);
        graph.render(&playing_context(MAX_BLOCK));
        let (left, right) = graph.output(MAX_BLOCK);
        assert!(left.iter().any(|&s| s.abs() > 0.1));
        assert!(right.iter().any(|&s| s.abs() > 0.1));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_project_context_and_repaired_path_refresh_on_subsequent_compile() {
        let dir =
            std::env::temp_dir().join(format!("livecraft_compiler_refresh_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let root_a = dir.join("ProjectA Project");
        let root_b = dir.join("ProjectB Project");
        livecraft_formats::project_dir::create_project_layout(&root_a).expect("layout A");
        livecraft_formats::project_dir::create_project_layout(&root_b).expect("layout B");
        write_constant_wav(&root_a.join("Samples/Imported/tone.wav"), 0.2, 4096);
        write_constant_wav(&root_b.join("Samples/Imported/tone.wav"), 0.8, 4096);
        let repaired_wav = dir.join("repaired.wav");
        write_constant_wav(&repaired_wav, 0.4, 4096);

        let mut project = Project::new_default("Refresh");
        project.tracks[2].device_chain.clear();
        project.main_track.device_chain.clear();
        let mut clip = Clip::new_audio(
            902,
            "Tone",
            "C:/definitely_nonexistent_dir_12345/Samples/Imported/tone.wav",
            4.0,
        );
        if let ClipPayload::Audio(ref mut audio) = clip.payload {
            audio.relative_path = Some("Samples/Imported/tone.wav".to_string());
        }
        project.tracks[2].clip_slots[0].clip = Some(clip);

        let collector = Collector::new();
        let mut compiler = GraphCompiler::new(44100.0, collector.handle());

        // Without ProjectContext, broken absolute + relative path renders silence.
        let mut silent_graph = compiler.compile(&project);
        silent_graph
            .node_mut(TrackId(3))
            .expect("track 3")
            .clips
            .launch(0);
        silent_graph.render(&playing_context(MAX_BLOCK));
        assert!(silent_graph.output(MAX_BLOCK).0.iter().all(|&s| s == 0.0));

        // Setting context to ProjectA resolves and plays ProjectA's WAV.
        compiler.set_project_context(ProjectContext::for_project(&root_a));
        let mut graph_a = compiler.compile(&project);
        graph_a
            .node_mut(TrackId(3))
            .expect("track 3")
            .clips
            .launch(0);
        graph_a.render(&playing_context(MAX_BLOCK));
        let peak_a = graph_a
            .output(MAX_BLOCK)
            .0
            .iter()
            .fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(
            peak_a > 0.05 && peak_a < 0.35,
            "expected ~0.14 peak, got {peak_a}"
        );

        // Moving ProjectContext to ProjectB resolves to ProjectB's file rather than stale cache.
        compiler.set_project_context(ProjectContext::for_project(&root_b));
        let mut graph_b = compiler.compile(&project);
        graph_b
            .node_mut(TrackId(3))
            .expect("track 3")
            .clips
            .launch(0);
        graph_b.render(&playing_context(MAX_BLOCK));
        let peak_b = graph_b
            .output(MAX_BLOCK)
            .0
            .iter()
            .fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(
            peak_b > 0.45,
            "expected ~0.56 peak from ProjectB, got {peak_b}"
        );

        // Fixing a previously missing direct path reloads and plays on next compile().
        project.tracks[2].clip_slots[0].clip = Some(Clip::new_audio(
            903,
            "Repair",
            "C:/definitely_nonexistent_dir_12345/missing.wav",
            4.0,
        ));
        let mut missing_graph = compiler.compile(&project);
        missing_graph
            .node_mut(TrackId(3))
            .expect("track 3")
            .clips
            .launch(0);
        missing_graph.render(&playing_context(MAX_BLOCK));
        assert!(missing_graph.output(MAX_BLOCK).0.iter().all(|&s| s == 0.0));

        if let Some(Clip {
            payload: ClipPayload::Audio(ref mut audio),
            ..
        }) = project.tracks[2].clip_slots[0].clip
        {
            audio.file_path = repaired_wav.to_string_lossy().into_owned();
        }
        let mut repaired_graph = compiler.compile(&project);
        repaired_graph
            .node_mut(TrackId(3))
            .expect("track 3")
            .clips
            .launch(0);
        repaired_graph.render(&playing_context(MAX_BLOCK));
        assert!(
            repaired_graph
                .output(MAX_BLOCK)
                .0
                .iter()
                .any(|&s| s.abs() > 0.1)
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
