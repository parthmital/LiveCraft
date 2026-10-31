//! Simulated playback of a 16-track Set with a device change every 100 ms, with every driver callback
//! inside the allocation check (M05 item 11, QUALITY-GATES audio timing). Any allocation or free aborts
//! the test, so a finished run has zero violations.
//!
//! The short variant runs in every test pass. The 10-minute variant is ignored in debug builds, where it
//! would take hours. Run it in release with the checks on:
//!   cargo test --release -p livecraft-engine --no-default-features --features rt-checks --test long_playback -- --ignored

use basedrop::Collector;
use livecraft_engine::compiler::GraphCompiler;
use livecraft_engine::executor::{Engine, EngineCommand};
use livecraft_engine::rt::checked;
use livecraft_model::ids::{DeviceId, TrackId};
use livecraft_model::model::{
    Clip, ClipPayload, Device, DeviceType, MidiNote, Project, Track, TrackType,
};
use livecraft_model::param::ParamId;

/// The allocator that makes `checked` detect allocations. It exists whenever the checks are compiled in.
#[cfg(any(debug_assertions, not(feature = "disable-release-checks")))]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

const RATE: f32 = 44100.0;
/// The driver buffer, in frames.
const BUFFER: usize = 512;
/// A device change every 100 ms.
const CHANGE_FRAMES: usize = 4410;
const TRACKS: u64 = 16;

/// Sixteen MIDI tracks, each with a looping four-beat clip and an Operator and a Utility.
fn sixteen_track_set() -> Project {
    let mut project = Project::new_default("Sixteen");
    project.tracks.clear();
    for index in 0..TRACKS {
        let mut track = Track::new(
            TrackId(100 + index),
            format!("Track {}", index + 1),
            TrackType::Midi,
            1,
        );
        let mut clip = Clip::new_midi(1000 + index, "Loop", 4.0);
        if let ClipPayload::Midi(ref mut data) = clip.payload {
            for step in 0..4u8 {
                let pitch = 48 + ((index as u8 * 3 + step * 5) % 24);
                data.notes
                    .push(MidiNote::new(pitch, f64::from(step), 1.0, 100));
            }
        }
        track.clip_slots[0].clip = Some(clip);
        track.device_chain.push(Device::new(
            DeviceId(10_000 + 2 * index),
            "Operator",
            DeviceType::Operator,
        ));
        track.device_chain.push(Device::new(
            DeviceId(10_001 + 2 * index),
            "Utility",
            DeviceType::Utility,
        ));
        project.tracks.push(track);
    }
    project
}

/// Plays `frames` frames of the Set, changing one track's Utility gain every 100 ms with a sample offset
/// that varies, and returns the number of driver callbacks it rendered.
fn play(frames: usize) -> usize {
    let project = sixteen_track_set();
    let collector = Collector::new();
    let mut compiler = GraphCompiler::new(RATE, collector.handle());
    let graph = compiler.compile(&project);
    let (mut engine, mut handle) = Engine::start(graph, RATE, collector).expect("start");
    for index in 0..TRACKS {
        handle
            .send(EngineCommand::TriggerClip {
                track: TrackId(100 + index),
                slot: 0,
            })
            .expect("send");
    }
    handle.send(EngineCommand::SetPlaying(true)).expect("send");

    let mut out = vec![0.0f32; BUFFER * 2];
    let mut rendered = 0;
    let mut callbacks = 0;
    let mut next_change = 0;
    let mut change = 0u64;
    let mut energy = 0.0f64;
    while rendered < frames {
        if rendered >= next_change {
            let slot = change % TRACKS;
            handle
                .send(EngineCommand::SetDeviceParam {
                    track: TrackId(100 + slot),
                    device: DeviceId(10_001 + 2 * slot),
                    param: ParamId(0),
                    value: -20.0 + (change % 12) as f32 * 2.0,
                    offset: ((change * 37) % BUFFER as u64) as u32,
                })
                .expect("send");
            next_change += CHANGE_FRAMES;
            change += 1;
        }
        let block = BUFFER.min(frames - rendered);
        let samples = &mut out[..block * 2];
        checked(|| engine.process(samples, 2));
        energy += samples.iter().map(|&s| f64::from(s * s)).sum::<f64>();
        rendered += block;
        callbacks += 1;
        // The session reads the snapshot between callbacks, so it does not run inside the check.
        let _ = handle.snapshot().node_count;
    }
    assert!(energy > 0.0, "the Set should be audible");
    callbacks
}

#[test]
fn two_seconds_of_playback_allocate_nothing() {
    assert_eq!(play(2 * 44100), (2 * 44100usize).div_ceil(BUFFER));
}

#[test]
#[ignore = "10 minutes of audio; run in release with the checks on (see the module docs)"]
fn ten_minutes_of_playback_allocate_nothing() {
    let callbacks = play(10 * 60 * 44100);
    println!("rendered 10 minutes in {callbacks} driver callbacks");
}
