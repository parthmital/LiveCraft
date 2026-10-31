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
fn the_mixer_and_transport_commands_reach_their_node_and_the_snapshot() {
    let (mut engine, mut handle, _compiler) = started(&Project::new_default("Mixer"));
    for command in [
        EngineCommand::SetTempo(135.0),
        EngineCommand::SetTrackVolume {
            track: TrackId(1),
            volume_db: -6.5,
        },
        EngineCommand::SetTrackPan {
            track: TrackId(1),
            pan: -0.4,
        },
        EngineCommand::SetTrackMute {
            track: TrackId(1),
            muted: true,
        },
        EngineCommand::SetTrackSolo {
            track: TrackId(1),
            soloed: true,
        },
        EngineCommand::SetTrackVolume {
            track: TrackId(7),
            volume_db: -3.0,
        },
    ] {
        handle.send(command).expect("send");
    }
    render_blocks(&mut engine, 1);
    let mixer = &engine.graph.node_mut(TrackId(1)).expect("track 1").mixer;
    assert_eq!(mixer.volume_db, -6.5);
    assert_eq!(mixer.pan, -0.4);
    assert!(mixer.is_muted && mixer.is_soloed);
    let main = &engine.graph.node_mut(TrackId(7)).expect("main").mixer;
    assert_eq!(main.volume_db, -3.0);
    assert_eq!(handle.snapshot().transport.bpm, 135.0);
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
    project.tracks[0]
        .device_chain
        .push(Device::new(DeviceId(500), "Utility", DeviceType::Utility));
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
    use crate::compiled::{InputRoute, Mixer, Node, NodeKind};
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
                InputRoute::default(),
            )
        })
        .collect();
    let large = CompiledGraph::new(nodes, MAX_NODES + 1, 0, Vec::new(), Vec::new(), [0.0; 128]);
    assert_eq!(
        handle.submit_graph(large),
        Err(SubmitError::TooManyNodes {
            nodes: MAX_NODES + 1,
            max: MAX_NODES,
        })
    );
}

#[test]
fn sanitize_replaces_non_finite_samples_and_keeps_finite_ones_bit_exact() {
    assert_eq!(sanitize_output_sample(f32::NAN), 0.0);
    assert_eq!(sanitize_output_sample(f32::INFINITY), 0.0);
    assert_eq!(sanitize_output_sample(f32::NEG_INFINITY), 0.0);
    for value in [0.0_f32, -0.0, 0.5, -1.0, 3.75, f32::MIN_POSITIVE, f32::MAX] {
        assert_eq!(sanitize_output_sample(value).to_bits(), value.to_bits());
    }
}
