//! The transport and launch model end to end (M07): sample-accurate quantized launches, Follow Action
//! determinism, scenes, the Arrangement Loop, Back to Arrangement, count-in, tempo changes, and graph
//! swaps. Every callback runs inside the allocation check. Note-on frames are read from the MIDI
//! output queue of a track routed to an external port, which stamps each note with its engine frame.

use livecraft_engine::bouncer::{OfflineBouncer, RenderOptions};
use livecraft_engine::compiler::GraphCompiler;
use livecraft_engine::executor::{Engine, EngineCommand, EngineHandle};
use livecraft_engine::midi::MidiOutEvent;
use livecraft_engine::rt::checked;
use livecraft_engine::transport::TransportSettings;
use livecraft_model::ids::{SceneId, TrackId};
use livecraft_model::model::{
    ArrangementClip, Clip, ClipPayload, FollowAction, FollowActions, Metronome, MidiNote,
    MidiOutput, Project, Quantization,
};
use livecraft_time::{Beats, beats_per_sample};

#[cfg(debug_assertions)]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

const RATE: f32 = 44100.0;
const BUFFER: usize = 256;

/// A MIDI clip of `length` beats with one note per entry of `notes` (pitch, start beat).
fn clip(id: u64, length: f64, notes: &[(u8, f64)]) -> Clip {
    let mut clip = Clip::new_midi(id, "Clip", length);
    if let ClipPayload::Midi(ref mut data) = clip.payload {
        for &(pitch, start) in notes {
            data.notes.push(MidiNote::new(pitch, start, 0.25, 100));
        }
    }
    clip
}

/// The default Set with track 1 routed to MIDI output port 0, so its notes reach the output queue.
fn routed_set() -> Project {
    let mut project = Project::new_default("Launch");
    project.tracks[0].routing.midi_to = MidiOutput::External {
        port: 0,
        channel: 0,
    };
    project
}

struct Rig {
    engine: Engine,
    handle: EngineHandle,
    notes: rtrb::Consumer<MidiOutEvent>,
    out: Vec<f32>,
}

impl Rig {
    fn new(project: &Project) -> Self {
        let collector = basedrop::Collector::new();
        let mut compiler = GraphCompiler::new(RATE, collector.handle());
        let (mut engine, handle) =
            Engine::start(compiler.compile(project), RATE, collector).expect("start the engine");
        let (producer, notes) = rtrb::RingBuffer::new(4096);
        let (anchor, _anchors) = triple_buffer::triple_buffer(&None);
        engine.set_midi_output(producer, anchor);
        Self {
            engine,
            handle,
            notes,
            out: vec![0.0; BUFFER * 2],
        }
    }

    fn send(&mut self, command: EngineCommand) {
        self.handle.send(command).expect("room in the queue");
    }

    /// Renders `buffers` driver buffers inside the allocation check.
    fn run(&mut self, buffers: usize) {
        for _ in 0..buffers {
            let (engine, out) = (&mut self.engine, &mut self.out);
            checked(|| engine.process_at(out, 2, 0));
        }
    }

    /// The engine frames of the note-ons sent since the last call, with their pitches.
    fn note_ons(&mut self) -> Vec<(u64, u8)> {
        let mut found = Vec::new();
        while let Ok(event) = self.notes.pop() {
            if event.bytes[0] & 0xF0 == 0x90 {
                found.push((event.frame, event.bytes[1]));
            }
        }
        found
    }

    fn transport(&mut self) -> livecraft_engine::snapshot::Transport {
        self.handle.snapshot().transport
    }
}

/// The first frame whose song position reaches `beat`, at a constant tempo from frame 0 at beat 0.
fn frame_of(beat: f64, bpm: f32) -> u64 {
    let dt = beats_per_sample(f64::from(bpm), f64::from(RATE));
    let mut frame = (beat / dt).floor() as u64;
    while (frame as f64) * dt < beat {
        frame += 1;
    }
    while frame > 0 && ((frame - 1) as f64) * dt >= beat {
        frame -= 1;
    }
    frame
}

#[test]
fn a_clip_launched_at_one_bar_starts_on_the_sample_of_the_next_bar_line_at_ten_tempos() {
    // "The Clip Quantization chooser lets you adjust an onset timing correction for clip triggering."
    // (manual 16.4, page 363)
    let mut seed: u64 = 0x5eed;
    for _ in 0..10 {
        seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let bpm = 60.0 + (seed >> 33) as f32 % 140.0 + ((seed >> 20) & 0xff) as f32 / 256.0;
        let mut project = routed_set();
        project.transport.set_bpm(bpm);
        let mut launched = clip(500, 4.0, &[(60, 0.0)]);
        launched.launch.quantization = Quantization::Bars(1);
        project.tracks[0].clip_slots[0].clip = Some(launched);
        let mut rig = Rig::new(&project);
        rig.send(EngineCommand::SetPlaying(true));
        // Play for a while, then launch between bar lines.
        let buffers = 1 + (seed >> 40) as usize % 300;
        rig.run(buffers);
        let song = rig.transport().playhead_beats;
        let next_bar = ((song / 4.0).floor() + 1.0) * 4.0;
        rig.send(EngineCommand::TriggerClip {
            track: TrackId(1),
            slot: 0,
        });
        let bar_frames = (4.0 * 60.0 / f64::from(bpm) * f64::from(RATE)) as usize;
        rig.run(bar_frames / BUFFER + 2);
        let ons = rig.note_ons();
        assert_eq!(
            ons.first().map(|&(frame, _)| frame),
            Some(frame_of(next_bar, bpm)),
            "tempo {bpm}"
        );
    }
}

#[test]
fn a_clip_with_no_quantization_starts_at_the_next_callback() {
    let mut project = routed_set();
    let mut launched = clip(500, 4.0, &[(60, 0.0)]);
    launched.launch.quantization = Quantization::None;
    project.tracks[0].clip_slots[0].clip = Some(launched);
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::SetPlaying(true));
    rig.run(3);
    rig.send(EngineCommand::TriggerClip {
        track: TrackId(1),
        slot: 0,
    });
    rig.run(1);
    assert_eq!(rig.note_ons(), vec![(3 * BUFFER as u64, 60)]);
}

#[test]
fn a_launch_while_stopped_starts_the_transport_and_the_clip_at_once() {
    let mut project = routed_set();
    project.tracks[0].clip_slots[0].clip = Some(clip(500, 4.0, &[(60, 0.0)]));
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::TriggerClip {
        track: TrackId(1),
        slot: 0,
    });
    rig.run(1);
    assert!(rig.transport().playing);
    assert_eq!(rig.note_ons(), vec![(0, 60)]);
}

/// The time an engine needs to render `seconds` of audio, in buffers.
fn buffers_for(seconds: f64) -> usize {
    (seconds * f64::from(RATE) / BUFFER as f64).ceil() as usize
}

fn follow_set(seed: u64) -> Project {
    let mut project = routed_set();
    project.transport.follow_seed = seed;
    for (row, pitch) in [(0usize, 60u8), (1, 62), (2, 64)] {
        let mut clip = clip(500 + row as u64, 1.0, &[(pitch, 0.0)]);
        clip.follow = FollowActions {
            enabled: true,
            action_a: FollowAction::Any,
            action_b: FollowAction::Next,
            chance_a: 0.7,
            chance_b: 0.3,
            ..FollowActions::default()
        };
        project.tracks[0].clip_slots[row].clip = Some(clip);
    }
    project
}

fn follow_pitches(seed: u64) -> Vec<(u64, u8)> {
    let mut rig = Rig::new(&follow_set(seed));
    rig.send(EngineCommand::TriggerClip {
        track: TrackId(1),
        slot: 0,
    });
    rig.run(buffers_for(10.0));
    rig.note_ons()
}

#[test]
fn follow_actions_play_the_same_way_for_the_same_seed() {
    let first = follow_pitches(11);
    let second = follow_pitches(11);
    assert!(first.len() > 15, "{first:?}");
    assert_eq!(first, second);
    assert_ne!(first, follow_pitches(12));
    // A linked Follow Action of a one-beat clip comes at the end of each beat, so a note starts on every
    // beat: 22050 frames apart at 120 BPM.
    for pair in first.windows(2) {
        assert_eq!(pair[1].0 - pair[0].0, 22050);
    }
}

#[test]
fn a_follow_action_render_is_byte_identical_for_a_stored_seed() {
    let project = follow_set(7);
    let folder = std::env::temp_dir().join("livecraft_follow_render");
    std::fs::create_dir_all(&folder).expect("create the folder");
    let paths = [folder.join("a.wav"), folder.join("b.wav")];
    for path in &paths {
        let options = RenderOptions {
            sample_rate: 44100,
            duration_bars: 4,
            bits_per_sample: 24,
            ..RenderOptions::default()
        };
        OfflineBouncer::render_project(&project, path, options).expect("render");
    }
    let first = std::fs::read(&paths[0]).expect("read");
    let second = std::fs::read(&paths[1]).expect("read");
    let _ = std::fs::remove_dir_all(folder);
    assert!(first == second, "the renders differ");
}

#[test]
fn follow_actions_disabled_globally_keep_the_clip_playing() {
    let mut rig = Rig::new(&follow_set(3));
    rig.send(EngineCommand::SetTransport(TransportSettings {
        follow_actions_enabled: false,
        ..TransportSettings::default()
    }));
    rig.send(EngineCommand::TriggerClip {
        track: TrackId(1),
        slot: 0,
    });
    rig.run(buffers_for(3.0));
    assert!(rig.note_ons().iter().all(|&(_, pitch)| pitch == 60));
}

#[test]
fn a_scene_launches_its_row_and_sets_its_tempo_at_the_launch_time() {
    let mut project = routed_set();
    project.tracks[0].clip_slots[1].clip = Some(clip(500, 4.0, &[(67, 0.0)]));
    project.scenes[1].tempo = Some(90.0);
    let scene: SceneId = project.scenes[1].id;
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::SetPlaying(true));
    rig.run(10);
    rig.send(EngineCommand::TriggerScene { scene });
    rig.run(1);
    assert_eq!(
        rig.transport().bpm,
        120.0,
        "the tempo waits for the bar line"
    );
    rig.run(buffers_for(2.1));
    assert_eq!(rig.transport().bpm, 90.0);
    assert_eq!(rig.note_ons().first(), Some(&(frame_of(4.0, 120.0), 67)));
}

#[test]
fn the_arrangement_loop_jumps_back_to_its_start_on_the_sample_of_its_end() {
    // "To repeatedly play a section of the Arrangement, activate the Arrangement Loop" (manual 6.6).
    let mut project = routed_set();
    let looped = clip(600, 1.0, &[(72, 0.0)]);
    project.tracks[0]
        .arrangement_clips
        .push(ArrangementClip::new(601.into(), looped, 0.0, 1.0));
    project.transport.loop_enabled = true;
    project.transport.loop_start_beats = 0.0;
    project.transport.loop_length_beats = 2.0;
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::SetPlaying(true));
    rig.run(buffers_for(2.5));
    let ons = rig.note_ons();
    assert_eq!(ons[0], (0, 72));
    assert_eq!(ons[1], (frame_of(2.0, 120.0), 72));
    assert!(rig.transport().playhead_beats < 2.0);
}

#[test]
fn a_launched_clip_takes_the_track_out_of_the_arrangement_until_back_to_arrangement() {
    // "The Arrangement clips in the track where the Session clip was launched will not resume playback
    // until you manually restart it using the Back to Arrangement button." (manual 3.7, page 42)
    let mut project = routed_set();
    let long = clip(600, 64.0, &[(72, 4.0), (72, 8.0)]);
    project.tracks[0]
        .arrangement_clips
        .push(ArrangementClip::new(601.into(), long, 0.0, 64.0));
    let mut session_clip = clip(500, 1.0, &[(60, 0.0)]);
    session_clip.launch.quantization = Quantization::None;
    project.tracks[0].clip_slots[0].clip = Some(session_clip);
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::SetPlaying(true));
    rig.send(EngineCommand::TriggerClip {
        track: TrackId(1),
        slot: 0,
    });
    rig.run(1);
    rig.send(EngineCommand::StopClip { track: TrackId(1) });
    rig.run(buffers_for(2.5));
    assert!(rig.transport().back_to_arrangement);
    assert!(rig.note_ons().iter().all(|&(_, pitch)| pitch == 60));
    rig.send(EngineCommand::BackToArrangement);
    rig.run(buffers_for(2.0));
    assert!(!rig.transport().back_to_arrangement);
    assert_eq!(
        rig.note_ons().first(),
        Some(&(frame_of(8.0, 120.0), 72)),
        "the Arrangement plays again"
    );
}

#[test]
fn a_count_in_plays_the_metronome_for_its_bars_before_the_start() {
    // "The count-in runs from negative bars-beats-sixteenths (beginning at -2.1.1., for example, with a
    // Count-In setting of 2 bars) up to 1.1.1., at which point recording commences." (manual 19.6)
    let mut project = Project::new_default("Count-in");
    project.transport.metronome = Metronome {
        count_in_bars: 1,
        ..Metronome::default()
    };
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::SetRecording(true));
    rig.send(EngineCommand::SetPlaying(true));
    rig.run(1);
    let transport = rig.transport();
    assert!(transport.counting_in);
    assert!(transport.playhead_beats < -3.9);
    assert!(
        rig.out.iter().any(|&sample| sample != 0.0),
        "the count-in ticks"
    );
    rig.run(buffers_for(2.05));
    let transport = rig.transport();
    assert!(!transport.counting_in);
    assert!(transport.playhead_beats >= 0.0);
}

#[test]
fn the_metronome_is_silent_when_only_while_recording_and_not_recording() {
    let mut project = Project::new_default("Metronome");
    project.transport.metronome_enabled = true;
    project.transport.metronome.only_while_recording = true;
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::SetPlaying(true));
    rig.run(4);
    assert!(rig.out.iter().all(|&sample| sample == 0.0));
    rig.send(EngineCommand::SetRecording(true));
    rig.run(buffers_for(0.6));
    let mut heard = false;
    for _ in 0..buffers_for(0.6) {
        rig.run(1);
        heard |= rig.out.iter().any(|&sample| sample != 0.0);
    }
    assert!(heard);
}

#[test]
fn the_tempo_follows_the_sets_tempo_changes_from_their_beat() {
    let mut project = Project::new_default("Tempo map");
    project.transport.tempo.insert(Beats(4.0), 150.0, false);
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::SetPlaying(true));
    rig.run(buffers_for(1.0));
    assert_eq!(rig.transport().bpm, 120.0);
    rig.run(buffers_for(1.2));
    assert_eq!(rig.transport().bpm, 150.0);
}

#[test]
fn a_graph_swap_keeps_the_launched_clip_playing() {
    let mut project = routed_set();
    project.tracks[0].clip_slots[0].clip = Some(clip(500, 1.0, &[(60, 0.0)]));
    let collector = basedrop::Collector::new();
    let mut compiler = GraphCompiler::new(RATE, collector.handle());
    let (mut engine, mut handle) =
        Engine::start(compiler.compile(&project), RATE, collector).expect("start");
    handle
        .send(EngineCommand::TriggerClip {
            track: TrackId(1),
            slot: 0,
        })
        .expect("send");
    let mut out = vec![0.0f32; BUFFER * 2];
    checked(|| engine.process(&mut out, 2));
    handle
        .submit_graph(compiler.compile(&project))
        .expect("submit");
    for _ in 0..4 {
        checked(|| engine.process(&mut out, 2));
    }
    assert_eq!(handle.snapshot().nodes[0].clip.active_slot, Some(0));
}

#[test]
fn stopping_the_transport_stops_the_session_clips() {
    let mut project = routed_set();
    project.tracks[0].clip_slots[0].clip = Some(clip(500, 1.0, &[(60, 0.0)]));
    let mut rig = Rig::new(&project);
    rig.send(EngineCommand::TriggerClip {
        track: TrackId(1),
        slot: 0,
    });
    rig.run(2);
    rig.send(EngineCommand::SetPlaying(false));
    rig.run(1);
    assert_eq!(rig.handle.snapshot().nodes[0].clip.active_slot, None);
}

#[test]
fn a_jump_waits_for_the_next_global_quantization_step() {
    // "Locators are quantized according to the global launch quantization value set in the Control Bar."
    // (manual 6.4, page 171)
    let mut rig = Rig::new(&Project::new_default("Jump"));
    rig.send(EngineCommand::SetPlaying(true));
    rig.run(buffers_for(0.5));
    rig.send(EngineCommand::JumpTo(32.0));
    rig.run(1);
    assert!(rig.transport().playhead_beats < 4.0);
    rig.run(buffers_for(1.6));
    let at = rig.transport().playhead_beats;
    assert!((32.0..33.0).contains(&at), "{at}");
}
