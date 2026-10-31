//! Compiles a Live Set into a `CompiledGraph` (M05 items 5 and 7). This runs on the session side,
//! never on the audio thread: it allocates the processors and buffers the audio thread then uses,
//! and it decodes the audio files the clips play, so the audio thread never reads a file.

use crate::audio_clip::{AudioSampleCache, DecodedAudio};
use crate::clip::{ArrangedClip, ClipLane, PlayNote, PlayableClip, PlayableSource};
use crate::compiled::{
    CompiledGraph, DeviceSlot, MAX_NOTE_EVENTS, Mixer, Node, NodeKind, Transfer, compensate_latency,
};
use basedrop::{Handle, Shared};
use livecraft_devices::{DeviceProcessor, create_device_processor, is_instrument};
use livecraft_model::device::param_table;
use livecraft_model::ids::{DeviceId, TrackId};
use livecraft_model::model::{
    Clip, ClipPayload, Device, MidiClipData, MonitoringMode, Project, Track, TuningSystem,
};
use livecraft_model::param::ParamId;
use std::collections::HashMap;
use std::path::Path;

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
    /// The track and device ids of each node of the last graph, in node order.
    previous: Vec<(TrackId, Vec<DeviceId>)>,
    /// The latency each device had in the last graph, in frames.
    latencies: HashMap<DeviceId, usize>,
    /// Decoded audio by file path. A file that fails to decode is not cached, so it is retried.
    audio: HashMap<String, Shared<DecodedAudio>>,
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
            previous: Vec::new(),
            latencies: HashMap::new(),
            audio: HashMap::new(),
        }
    }

    /// Compiles the Set into the nodes the renderer runs. A device without a native processor makes
    /// no sound, so it is left out of the chain. A clip whose audio file cannot be read plays nothing.
    pub fn compile(&mut self, project: &Project) -> CompiledGraph {
        let tuning = project.tuning.as_ref();
        let mut nodes = Vec::with_capacity(project.all_tracks().count());
        for track in &project.tracks {
            nodes.push(self.node(track, NodeKind::Track, tuning));
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
        CompiledGraph::new(
            nodes,
            project.tracks.len(),
            project.return_tracks.len(),
            project.scenes.iter().map(|scene| scene.id).collect(),
            transfers,
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
            NodeKind::Track => self.lane(track, tuning),
            NodeKind::Return | NodeKind::Main => ClipLane::new(Vec::new(), Vec::new(), false, 0),
        };
        Node::new(track.id, kind, mixer, devices, clips)
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
        ClipLane::new(slots, arranged, plays_clips, MAX_NOTE_EVENTS)
    }

    /// A clip prepared for playback, or None when its audio cannot be read.
    fn playable(&mut self, clip: &Clip, tuning: Option<&TuningSystem>) -> Option<PlayableClip> {
        let source = match &clip.payload {
            ClipPayload::Midi(midi) => {
                let notes = play_notes(midi, tuning);
                PlayableSource::Midi(Shared::new(&self.handle, notes))
            }
            ClipPayload::Audio(audio) => PlayableSource::Audio {
                data: self.decoded(&audio.file_path)?,
                gain: 10.0f32.powf(audio.gain_db / 20.0),
                is_warped: audio.is_warped,
                original_bpm: audio.original_bpm,
            },
        };
        Some(PlayableClip {
            length_beats: clip.length_beats,
            is_looping: clip.is_looping,
            source,
        })
    }

    /// The decoded audio of a file, read once and shared.
    fn decoded(&mut self, path: &str) -> Option<Shared<DecodedAudio>> {
        if path.is_empty() {
            return None;
        }
        if let Some(data) = self.audio.get(path) {
            return Some(data.clone());
        }
        match AudioSampleCache::load_file(Path::new(path)) {
            Ok(decoded) => {
                let data = Shared::new(&self.handle, decoded);
                self.audio.insert(path.to_string(), data.clone());
                Some(data)
            }
            Err(error) => {
                log::warn!("Audio clip file {path} cannot be read, so the clip is silent: {error}");
                None
            }
        }
    }
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
            freq_hz: match tuning {
                Some(tuning) => tuning.note_to_freq(note.pitch) as f32,
                None => 440.0 * 2.0f32.powf((f32::from(note.pitch) - 69.0) / 12.0),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiled::MAX_BLOCK;
    use basedrop::Collector;
    use livecraft_devices::{NoteEvent, ProcessContext};
    use livecraft_model::ids::{DeviceId, TrackId};
    use livecraft_model::model::{DeviceType, MidiNote};

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
        graph.launch_clip(TrackId(1), 0);
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
        graph.launch_clip(TrackId(1), 0);
        graph.render(&context(MAX_BLOCK));
        let (left, _) = graph.output(MAX_BLOCK);
        assert!(left.iter().all(|&sample| sample == 0.0));
    }
}
