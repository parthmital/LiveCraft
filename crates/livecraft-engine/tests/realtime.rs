//! The audio path renders a block without allocating (M05 items 5 and 6, M06 items 5 and 6 for audio
//! and MIDI input). The render runs inside the allocation check, which aborts on any allocation or free.

use livecraft_devices::{NoteEvent, ProcessContext};
use livecraft_engine::compiled::MAX_BLOCK;
use livecraft_engine::compiler::GraphCompiler;
use livecraft_engine::executor::{Engine, EngineCommand, EngineHandle};
use livecraft_engine::midi::{HostClock, MidiMessage, period_us};
use livecraft_engine::rt::checked;
use livecraft_model::ids::{DeviceId, TrackId};
use livecraft_model::model::{
    AudioInput, Device, DeviceType, MonitoringMode, Project, Track, TrackType,
};

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

fn context() -> ProcessContext {
    ProcessContext {
        sample_rate: 44100.0,
        frames: MAX_BLOCK,
        bpm: 120.0,
        playhead_beats: 0.0,
        beats_per_frame: 0.0,
    }
}

#[test]
fn rendering_a_block_with_notes_allocates_nothing() {
    let collector = basedrop::Collector::new();
    let mut graph =
        GraphCompiler::new(44100.0, collector.handle()).compile(&Project::new_default("Realtime"));
    let ctx = context();
    let track = graph.node_mut(TrackId(1)).expect("track 1");
    assert!(track.events.push(NoteEvent::On {
        offset: 0,
        pitch: 60,
        velocity: 100,
        freq_hz: 261.6,
    }));
    checked(|| graph.render(&ctx));
    let (left, _) = graph.output(MAX_BLOCK);
    assert!(left.iter().any(|&sample| sample != 0.0));
}

#[test]
fn the_driver_callback_applies_a_command_and_a_swap_without_allocating() {
    let project = Project::new_default("Callback");
    let collector = basedrop::Collector::new();
    let mut compiler = GraphCompiler::new(44100.0, collector.handle());
    let (mut engine, mut handle) =
        Engine::start(compiler.compile(&project), 44100.0, collector).expect("start the engine");
    handle
        .submit_graph(compiler.compile(&project))
        .expect("submit a graph");
    handle
        .send(EngineCommand::SetTrackVolume {
            track: TrackId(1),
            volume_db: -3.0,
        })
        .expect("send a command");
    let mut out = vec![0.0f32; MAX_BLOCK * 2];
    checked(|| engine.process(&mut out, 2));
    assert!(out.iter().all(|sample| sample.is_finite()));
}

#[test]
fn rendering_a_launched_clip_allocates_nothing() {
    use livecraft_model::model::{Clip, ClipPayload, MidiNote};

    let mut project = Project::new_default("Clip");
    let mut clip = Clip::new_midi(900, "Chord", 4.0);
    if let ClipPayload::Midi(ref mut data) = clip.payload {
        data.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        data.notes.push(MidiNote::new(64, 1.0, 2.0, 100));
    }
    project.tracks[0].clip_slots[0].clip = Some(clip);
    let collector = basedrop::Collector::new();
    let mut compiler = GraphCompiler::new(44100.0, collector.handle());
    let mut graph = compiler.compile(&project);
    graph.launch_clip(TrackId(1), 0);
    let ctx = ProcessContext {
        sample_rate: 44100.0,
        frames: MAX_BLOCK,
        bpm: 120.0,
        playhead_beats: 0.0,
        beats_per_frame: 2.0 / 44100.0,
    };
    checked(|| graph.render(&ctx));
    let (left, _) = graph.output(MAX_BLOCK);
    assert!(left.iter().any(|&sample| sample != 0.0));
}

/// An empty Set with one audio track that has no devices and receives input channel `channel`. The
/// track is panned hard left, so its left output is its input.
fn monitored_set(channel: u16, monitoring: MonitoringMode, armed: bool) -> Project {
    let mut project = Project::new_default("Input");
    project.tracks.clear();
    project.return_tracks.clear();
    project.main_track.device_chain.clear();
    let mut track = Track::new(TrackId(5), "Input", TrackType::Audio, 0);
    track.routing.audio_from = AudioInput::ExternalIn { channel };
    track.monitoring = monitoring;
    track.is_armed = armed;
    track.pan = -1.0;
    project.tracks.push(track);
    project
}

/// Starts an engine on `project` with a stereo input ring that the test fills.
fn engine_with_input(project: &Project) -> (Engine, EngineHandle, rtrb::Producer<f32>) {
    let collector = basedrop::Collector::new();
    let graph = GraphCompiler::new(44100.0, collector.handle()).compile(project);
    let (mut engine, handle) = Engine::start(graph, 44100.0, collector).expect("start the engine");
    let (producer, consumer) = rtrb::RingBuffer::new(MAX_BLOCK * 8 * 2);
    engine.set_input(consumer, 2);
    (engine, handle, producer)
}

/// Pushes interleaved input samples into the ring, as the input stream would.
fn push(input: &mut rtrb::Producer<f32>, samples: &[f32]) {
    for &sample in samples {
        input.push(sample).expect("the input ring has room");
    }
}

/// Interleaved stereo input of `frames` frames, silent except for frame `at`, which holds `frame`.
fn impulse(frames: usize, at: usize, frame: [f32; 2]) -> Vec<f32> {
    let mut samples = vec![0.0; frames * 2];
    samples[at * 2..at * 2 + 2].copy_from_slice(&frame);
    samples
}

/// Renders `frames` stereo frames through the driver callback, under the allocation check.
fn render_frames(engine: &mut Engine, frames: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; frames * 2];
    checked(|| engine.process(&mut out, 2));
    out
}

#[test]
fn a_monitored_track_hears_its_input_channel_on_the_frame_it_arrives() {
    let (mut engine, _handle, mut input) =
        engine_with_input(&monitored_set(1, MonitoringMode::In, false));
    push(&mut input, &impulse(MAX_BLOCK, 10, [0.25, 0.5]));
    let out = render_frames(&mut engine, MAX_BLOCK);
    // The track takes input channel 1, so frame 10 carries 0.5 and never 0.25.
    let mut expected = vec![0.0; MAX_BLOCK * 2];
    expected[20] = 0.5;
    assert_eq!(out, expected);
}

#[test]
fn an_auto_track_that_is_armed_hears_its_input() {
    let (mut engine, _handle, mut input) =
        engine_with_input(&monitored_set(1, MonitoringMode::Auto, true));
    push(&mut input, &impulse(MAX_BLOCK, 10, [0.25, 0.5]));
    let out = render_frames(&mut engine, MAX_BLOCK);
    let mut expected = vec![0.0; MAX_BLOCK * 2];
    expected[20] = 0.5;
    assert_eq!(out, expected);
}

#[test]
fn an_auto_track_that_is_not_armed_is_silent_on_its_input() {
    let (mut engine, _handle, mut input) =
        engine_with_input(&monitored_set(1, MonitoringMode::Auto, false));
    push(&mut input, &impulse(MAX_BLOCK, 10, [0.25, 0.5]));
    let out = render_frames(&mut engine, MAX_BLOCK);
    assert!(out.iter().all(|&sample| sample == 0.0));
}

#[test]
fn an_off_track_is_silent_on_its_input_even_when_armed() {
    let (mut engine, _handle, mut input) =
        engine_with_input(&monitored_set(1, MonitoringMode::Off, true));
    push(&mut input, &impulse(MAX_BLOCK, 10, [0.25, 0.5]));
    let out = render_frames(&mut engine, MAX_BLOCK);
    assert!(out.iter().all(|&sample| sample == 0.0));
}

#[test]
fn each_block_reads_the_input_that_follows_the_block_before_it() {
    let (mut engine, _handle, mut input) =
        engine_with_input(&monitored_set(1, MonitoringMode::In, false));
    // The channel the track takes carries the frame number, so each output sample names its frame.
    let samples: Vec<f32> = (0..2 * MAX_BLOCK)
        .flat_map(|frame| [0.0, frame as f32])
        .collect();
    push(&mut input, &samples);
    let out = render_frames(&mut engine, 2 * MAX_BLOCK);
    let expected: Vec<f32> = (0..2 * MAX_BLOCK)
        .flat_map(|frame| [frame as f32, 0.0])
        .collect();
    assert_eq!(out, expected);
}

#[test]
fn an_input_the_stream_has_not_delivered_is_silent() {
    let (mut engine, _handle, mut input) =
        engine_with_input(&monitored_set(1, MonitoringMode::In, false));
    push(&mut input, &[0.0, 0.5, 0.0, 0.5, 0.0, 0.5]);
    let out = render_frames(&mut engine, MAX_BLOCK);
    let mut expected = vec![0.0; MAX_BLOCK * 2];
    for frame in 0..3 {
        expected[frame * 2] = 0.5;
    }
    assert_eq!(out, expected);
    // Nothing is left for the next block, so it is silent too.
    let next = render_frames(&mut engine, MAX_BLOCK);
    assert!(next.iter().all(|&sample| sample == 0.0));
}

/// Two MIDI tracks, each with an Operator. Track 1 is armed, and track 2 is not.
fn two_midi_tracks() -> Project {
    let mut project = Project::new_default("Midi input");
    project.tracks[0].is_armed = true;
    project.tracks[1].device_chain.push(Device::new(
        DeviceId(103),
        "Operator 2",
        DeviceType::Operator,
    ));
    project
}

#[test]
fn a_midi_note_on_plays_on_its_frame_in_an_armed_midi_track_and_not_in_an_unarmed_one() {
    let project = two_midi_tracks();
    let collector = basedrop::Collector::new();
    let mut compiler = GraphCompiler::new(44100.0, collector.handle());
    // The expected output: the same Set, with the note pushed into track 1 directly at frame 441.
    let mut expected = compiler.compile(&project);
    let frequency = expected.note_frequencies()[60];
    expected
        .node_mut(TrackId(1))
        .expect("track 1")
        .events
        .push(NoteEvent::On {
            offset: 441,
            pitch: 60,
            velocity: 100,
            freq_hz: frequency,
        });
    let ctx = context();
    expected.render(&ctx);
    let (left, right) = expected.output(MAX_BLOCK);
    let expected: Vec<f32> = left.iter().zip(right).flat_map(|(&l, &r)| [l, r]).collect();

    let (mut engine, _handle) =
        Engine::start(compiler.compile(&project), 44100.0, collector).expect("start the engine");
    let (mut producer, consumer) = rtrb::RingBuffer::new(16);
    engine.set_midi_input(vec![consumer], HostClock::start());
    // The note arrived 10 ms into the previous buffer period, which is frame 441 of this buffer.
    let block_start = 1_000_000;
    let time = block_start - period_us(MAX_BLOCK, 44100.0) + 10_000;
    producer
        .push(MidiMessage::new(time, 0, &[0x90, 60, 100]).expect("a note on"))
        .expect("the queue has room");
    let mut out = vec![0.0f32; MAX_BLOCK * 2];
    checked(|| engine.process_at(&mut out, 2, block_start));

    assert!(out[..441 * 2].iter().all(|&sample| sample == 0.0));
    assert!(out.iter().any(|&sample| sample != 0.0));
    assert_eq!(out, expected);
}
