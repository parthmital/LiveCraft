//! The audio path renders a block without allocating (M05 items 5 and 6). The render runs inside
//! the allocation check, which aborts on any allocation or free.

use livecraft_devices::{NoteEvent, ProcessContext};
use livecraft_engine::compiled::MAX_BLOCK;
use livecraft_engine::compiler::GraphCompiler;
use livecraft_engine::executor::{Engine, EngineCommand};
use livecraft_engine::rt::checked;
use livecraft_model::ids::TrackId;
use livecraft_model::model::Project;

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
