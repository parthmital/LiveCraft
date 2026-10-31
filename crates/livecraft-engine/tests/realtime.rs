//! The audio path renders a block without allocating (M05 items 5 and 6). The render runs inside
//! the allocation check, which aborts on any allocation or free.

use livecraft_devices::{NoteEvent, ProcessContext};
use livecraft_engine::compiled::MAX_BLOCK;
use livecraft_engine::compiler::GraphCompiler;
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
    let mut graph = GraphCompiler::new(44100.0).compile(&Project::new_default("Realtime"));
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
