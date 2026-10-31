//! Neutral operations change no sample (manual 39.2), so the output stays bit-identical. Two renders of
//! one Set are identical too (M05 item 11). Panning is not on the list: manual 39.2 leaves it out and
//! page 397 calls it a non-neutral operation, so the centre pan is not tested for bit-exactness.

use basedrop::{Collector, Shared};
use livecraft_engine::audio_clip::DecodedAudio;
use livecraft_engine::clip::{ArrangedClip, PlayableClip, PlayableSource};
use livecraft_engine::compiled::{CompiledGraph, MAX_BLOCK};
use livecraft_engine::compiler::GraphCompiler;
use livecraft_engine::executor::{Engine, EngineHandle};
use livecraft_model::ids::{DeviceId, TrackId};
use livecraft_model::model::{Device, DeviceType, Project};

const RATE: f32 = 44100.0;
const FRAMES: usize = 4 * MAX_BLOCK;

/// Starts the Set with a constant 0.5 input on each listed track, an audio clip of four beats from beat
/// 0 that plays from beat 0. The clip is the input of the track's devices.
fn start(project: &Project, tracks: &[TrackId]) -> (Engine, EngineHandle) {
    let collector = Collector::new();
    let mut compiler = GraphCompiler::new(RATE, collector.handle());
    let mut graph: CompiledGraph = compiler.compile(project);
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
    let (engine, mut handle) = Engine::start(graph, RATE, collector).expect("start");
    handle
        .send(livecraft_engine::executor::EngineCommand::SetPlaying(true))
        .expect("send");
    (engine, handle)
}

/// The left and right output of `FRAMES` frames, as bit patterns.
fn render(project: &Project, tracks: &[TrackId]) -> (Vec<u32>, Vec<u32>) {
    let (mut engine, _handle) = start(project, tracks);
    let mut out = vec![0.0f32; MAX_BLOCK * 2];
    let mut left = Vec::with_capacity(FRAMES);
    let mut right = Vec::with_capacity(FRAMES);
    while left.len() < FRAMES {
        engine.process(&mut out, 2);
        for frame in out.as_chunks::<2>().0 {
            left.push(frame[0].to_bits());
            right.push(frame[1].to_bits());
        }
    }
    (left, right)
}

fn values(bits: &[u32]) -> Vec<f32> {
    bits.iter().map(|&bit| f32::from_bits(bit)).collect()
}

#[test]
fn two_tracks_at_unity_gain_sum_bit_exactly() {
    let project = Project::new_default("Unity");
    let (first, _) = render(&project, &[TrackId(1)]);
    let (second, _) = render(&project, &[TrackId(2)]);
    let (both, _) = render(&project, &[TrackId(1), TrackId(2)]);
    let expected: Vec<u32> = values(&first)
        .iter()
        .zip(values(&second))
        .map(|(a, b)| (a + b).to_bits())
        .collect();
    assert_eq!(both, expected);
    assert!(values(&first).iter().any(|&sample| sample != 0.0));
}

#[test]
fn an_empty_device_chain_passes_the_clip_through_at_unity_gain() {
    let mut project = Project::new_default("Empty");
    project.tracks[0].device_chain.clear();
    let (left, right) = render(&project, &[TrackId(1)]);
    // A 0.5 input at 0 dB, then the centre pan gain that the track's mixer applies to its left channel.
    let pan_left = std::f32::consts::FRAC_PI_4.cos();
    let expected = (0.5f32 * 1.0 * pan_left).to_bits();
    assert!(left.iter().all(|&bit| bit == expected));
    assert!(right.iter().any(|&bit| bit != 0));
}

#[test]
fn a_disabled_device_leaves_the_signal_bit_identical() {
    let plain = Project::new_default("Plain");
    let mut bypassed = plain.clone();
    let mut utility = Device::new(DeviceId(500), "Utility", DeviceType::Utility);
    utility.is_enabled = false;
    bypassed.tracks[0].device_chain.push(utility);
    assert_eq!(
        render(&bypassed, &[TrackId(1)]),
        render(&plain, &[TrackId(1)])
    );
}

#[test]
fn a_send_at_minus_infinity_adds_nothing_to_main() {
    let with_returns = Project::new_default("Returns");
    let mut without_returns = with_returns.clone();
    without_returns.return_tracks.clear();
    // Every send of track 1 sits at 0, which is -inf dB.
    let mut sending = with_returns.clone();
    sending.tracks[0].sends = vec![0.0; sending.return_tracks.len()];
    assert_eq!(
        render(&sending, &[TrackId(1)]),
        render(&without_returns, &[TrackId(1)])
    );
}

#[test]
fn two_renders_of_one_set_are_identical() {
    let project = Project::new_default("Repeat");
    assert_eq!(
        render(&project, &[TrackId(1), TrackId(2)]),
        render(&project, &[TrackId(1), TrackId(2)])
    );
}
