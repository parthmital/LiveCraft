use crate::command::*;
use crate::ids::*;
use crate::model::*;

#[test]
fn test_project_creation() {
    let proj = Project::new_default("Test Project");
    assert_eq!(proj.tracks.len(), 4);
    assert_eq!(proj.return_tracks.len(), 2);
    assert_eq!(proj.scenes.len(), 8);
    assert_eq!(proj.transport.bpm(), 120.0);
}

#[test]
fn test_device_defaults() {
    let eq = Device::new(DeviceId(1), "EQ", DeviceType::EqEight);
    assert_eq!(eq.get_param("band_1_enabled"), 1.0);
    assert!(eq.get_param("band_8_freq") > 1000.0);

    let comp = Device::new(DeviceId(2), "Comp", DeviceType::Compressor);
    assert_eq!(comp.get_param("threshold_db"), -18.0);
    assert_eq!(comp.get_param("ratio"), 4.0);

    let delay = Device::new(DeviceId(3), "Delay", DeviceType::Delay);
    assert_eq!(delay.get_param("ping_pong"), 1.0);

    let op = Device::new(DeviceId(4), "Operator", DeviceType::Operator);
    assert_eq!(op.get_param("osc1_level"), 1.0);
    assert_eq!(op.get_param("filter_cutoff"), 2400.0);
}

#[test]
fn test_clip_slot_and_payload_manipulation() {
    let mut track = Track::new(TrackId(1), "Synth Lead", TrackType::Midi, 4);
    assert_eq!(track.clip_slots.len(), 4);

    let mut clip = Clip::new_midi(10, "Melody", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 100));
        mdata.notes.push(MidiNote::new(64, 1.0, 1.0, 100));
        mdata.notes.push(MidiNote::new(67, 2.0, 2.0, 110));
    }

    track.clip_slots[0].clip = Some(clip);
    assert!(track.clip_slots[0].clip.is_some());

    let retrieved = track.clip_slots[0].clip.as_ref().unwrap();
    assert_eq!(retrieved.name, "Melody");
    if let ClipPayload::Midi(ref m) = retrieved.payload {
        assert_eq!(m.notes.len(), 3);
        assert_eq!(m.notes[2].pitch, 67);
    }

    // Test ArrangementClip non-destructive editing (split, duplicate, trim)
    let arr_clip = ArrangementClip::new(
        ClipId(100),
        track.clip_slots[0].clip.clone().unwrap(),
        4.0,
        4.0,
    );

    // Split at beat 6.0 (2 beats into the 4-beat clip)
    let (first, second) = arr_clip.split_at(6.0, ClipId(101)).expect("Split failed");
    assert_eq!(first.start_beat, 4.0);
    assert_eq!(first.length_beats, 2.0);
    assert_eq!(second.start_beat, 6.0);
    assert_eq!(second.length_beats, 2.0);

    if let ClipPayload::Midi(ref m1) = first.clip.payload {
        // Notes 60 (0..1) and 64 (1..2) should be in first
        assert_eq!(m1.notes.len(), 2);
        assert_eq!(m1.notes[0].pitch, 60);
        assert_eq!(m1.notes[1].pitch, 64);
    }
    if let ClipPayload::Midi(ref m2) = second.clip.payload {
        // Note 67 was originally at 2.0..4.0, which corresponds to 0.0..2.0 in second part
        assert_eq!(m2.notes.len(), 1);
        assert_eq!(m2.notes[0].pitch, 67);
        assert_eq!(m2.notes[0].start_beat, 0.0);
        assert_eq!(m2.notes[0].duration_beats, 2.0);
    }

    // Test duplicate
    let dup = arr_clip.duplicate(ClipId(102), 8.0);
    assert_eq!(dup.id, ClipId(102));
    assert_eq!(dup.start_beat, 12.0);
    assert_eq!(dup.length_beats, 4.0);

    // Test trim_start and trim_end
    let mut trim_clip = arr_clip.clone();
    trim_clip.trim_start(1.0);
    assert_eq!(trim_clip.start_beat, 5.0);
    assert_eq!(trim_clip.length_beats, 3.0);

    trim_clip.trim_end(-1.0);
    assert_eq!(trim_clip.length_beats, 2.0);
}

#[test]
fn test_command_undo_redo() {
    let mut proj = Project::new_default("UndoTest");
    let mut history = CommandHistory::default();

    assert_eq!(proj.transport.bpm(), 120.0);

    let action = Box::new(SetTempoAction {
        old_tempo: 120.0,
        new_tempo: 140.0,
    });
    history.execute(action, &mut proj);
    assert_eq!(proj.transport.bpm(), 140.0);

    assert!(history.undo(&mut proj));
    assert_eq!(proj.transport.bpm(), 120.0);

    assert!(history.redo(&mut proj));
    assert_eq!(proj.transport.bpm(), 140.0);

    // Test AutomationLane and undo/redo of automation points
    let vol_lane = AutomationLane::new(LaneId(1), AutomationTarget::TrackVolume);
    let pan_lane = AutomationLane::new(LaneId(2), AutomationTarget::TrackPan);
    proj.tracks[0].automation_lanes.push(vol_lane);
    proj.tracks[0].automation_lanes.push(pan_lane);
    assert_eq!(proj.tracks[0].automation_lanes.len(), 2);

    // Initially empty lane evaluate_at returns None
    assert_eq!(proj.tracks[0].automation_lanes[0].evaluate_at(1.0), None);

    // Add first point at beat 0.0, val 0.0
    let p1 = AutomationPoint::new(0.0, 0.0, 0.0);
    let add_p1 = Box::new(AddAutomationPointAction::new(TrackId(1), LaneId(1), p1));
    history.execute(add_p1, &mut proj);
    assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 1);
    assert_eq!(
        proj.tracks[0].automation_lanes[0].evaluate_at(2.0),
        Some(0.0)
    );

    // Add second point at beat 4.0, val 1.0
    let p2 = AutomationPoint::new(4.0, 1.0, 0.0);
    let add_p2 = Box::new(AddAutomationPointAction::new(TrackId(1), LaneId(1), p2));
    history.execute(add_p2, &mut proj);
    assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 2);

    // Evaluate at boundary points, before start, after end, and midpoint
    assert!(
        (proj.tracks[0].automation_lanes[0]
            .evaluate_at(-1.0)
            .unwrap()
            - 0.0)
            .abs()
            < 1e-5
    );
    assert!((proj.tracks[0].automation_lanes[0].evaluate_at(0.0).unwrap() - 0.0).abs() < 1e-5);
    assert!((proj.tracks[0].automation_lanes[0].evaluate_at(2.0).unwrap() - 0.5).abs() < 1e-5);
    assert!((proj.tracks[0].automation_lanes[0].evaluate_at(4.0).unwrap() - 1.0).abs() < 1e-5);
    assert!((proj.tracks[0].automation_lanes[0].evaluate_at(5.0).unwrap() - 1.0).abs() < 1e-5);

    // Undo adding second point
    assert!(history.undo(&mut proj));
    assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 1);
    assert_eq!(
        proj.tracks[0].automation_lanes[0].evaluate_at(2.0),
        Some(0.0)
    );

    // Redo adding second point
    assert!(history.redo(&mut proj));
    assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 2);
    assert!((proj.tracks[0].automation_lanes[0].evaluate_at(2.0).unwrap() - 0.5).abs() < 1e-5);

    // Remove point at index 0
    let rem_p = Box::new(RemoveAutomationPointAction::new(TrackId(1), LaneId(1), 0));
    history.execute(rem_p, &mut proj);
    assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 1);
    assert_eq!(proj.tracks[0].automation_lanes[0].points[0].beat, 4.0);

    // Undo removal
    assert!(history.undo(&mut proj));
    assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 2);
    assert_eq!(proj.tracks[0].automation_lanes[0].points[0].beat, 0.0);

    // Redo removal
    assert!(history.redo(&mut proj));
    assert_eq!(proj.tracks[0].automation_lanes[0].points.len(), 1);
}

#[test]
fn test_command_history_capacity() {
    let mut proj = Project::new_default("HistoryCapTest");
    let mut history = CommandHistory::with_capacity(5);

    for i in 1..=10 {
        let action = Box::new(SetTempoAction {
            old_tempo: 120.0,
            new_tempo: 120.0 + i as f32,
        });
        history.execute(action, &mut proj);
    }

    assert_eq!(proj.transport.bpm(), 130.0);
    // Can only undo 5 times because max_history is 5
    let mut undos = 0;
    while history.undo(&mut proj) {
        undos += 1;
    }
    assert_eq!(undos, 5);
    assert_eq!(proj.transport.bpm(), 125.0);
}

#[test]
fn test_scale_system_and_snapping() {
    // C Major: C(60), D(62), E(64), F(65), G(67), A(69), B(71)
    let c_maj = ScaleConfig::new(0, ScaleType::Major);
    assert!(c_maj.is_in_scale(60)); // C4
    assert!(!c_maj.is_in_scale(61)); // C#4
    assert!(c_maj.is_in_scale(62)); // D4
    assert_eq!(c_maj.snap_to_scale(61), 60); // snaps to C or D

    // A Minor: A(57), B(59), C(60), D(62), E(64), F(65), G(67)
    let a_min = ScaleConfig::new(9, ScaleType::Minor);
    assert!(a_min.is_in_scale(60)); // C
    assert!(a_min.is_in_scale(57)); // A
    assert!(!a_min.is_in_scale(58)); // A#
}

#[test]
fn test_midi_transformations() {
    let mut data = MidiClipData {
        notes: vec![
            MidiNote::new(60, 0.24, 0.98, 100),
            MidiNote::new(64, 0.26, 1.05, 100),
            MidiNote::new(67, 1.10, 0.45, 100),
        ],
    };

    // Quantize to 16th notes (0.25)
    data.quantize(0.25);
    assert_eq!(data.notes[0].start_beat, 0.25);
    assert_eq!(data.notes[1].start_beat, 0.25);
    assert_eq!(data.notes[2].start_beat, 1.0);

    // Transpose up 2 semitones
    data.transpose(2, None);
    assert_eq!(data.notes[0].pitch, 62);
    assert_eq!(data.notes[1].pitch, 66);
    assert_eq!(data.notes[2].pitch, 69);

    // Strum chord at 0.25 beats
    data.strum(0.1, true);
    assert!(data.notes[1].start_beat > data.notes[0].start_beat);

    // Velocity ramp from 40 to 120
    data.velocity_ramp(40, 120);
    assert_eq!(data.notes[0].velocity, 40.0);
    assert_eq!(data.notes[2].velocity, 120.0);

    // Test Chop into 2 subdivisions
    let mut chop_data = MidiClipData {
        notes: vec![MidiNote::new(60, 0.0, 1.0, 100)],
    };
    chop_data.chop(2);
    assert_eq!(chop_data.notes.len(), 2);
    assert_eq!(chop_data.notes[0].duration_beats, 0.5);
    assert_eq!(chop_data.notes[1].start_beat, 0.5);

    // Test Span (legato)
    let mut span_data = MidiClipData {
        notes: vec![
            MidiNote::new(60, 0.0, 0.5, 100),
            MidiNote::new(62, 1.0, 0.5, 100),
        ],
    };
    span_data.span();
    assert_eq!(span_data.notes[0].duration_beats, 1.0);

    // Test Euclidean rhythm (3 pulses over 8 steps)
    let mut euc_data = MidiClipData { notes: Vec::new() };
    euc_data.generate_euclidean(3, 8, 36, 4.0, 100);
    assert_eq!(euc_data.notes.len(), 3);

    // Test Stacks (Triad: 0, 4, 7)
    let mut stack_data = MidiClipData { notes: Vec::new() };
    stack_data.generate_stacks(&[0, 4, 7], 60, 2.0, 110, None);
    assert_eq!(stack_data.notes.len(), 3);
    assert_eq!(stack_data.notes[0].pitch, 60);
    assert_eq!(stack_data.notes[1].pitch, 64);
    assert_eq!(stack_data.notes[2].pitch, 67);
}

#[test]
fn test_tuning_system() {
    let default_tuning = TuningSystem::default();
    let f69 = default_tuning.note_to_freq(69);
    assert!((f69 - 440.0).abs() < 1e-4);
    let f81 = default_tuning.note_to_freq(81);
    assert!((f81 - 880.0).abs() < 1e-4);

    let scl_text = "! pentatonic.scl\nSlendro 5-TET\n5\n240.0\n480.0\n720.0\n960.0\n1200.0\n";
    let slendro = TuningSystem::from_scala(scl_text).expect("Failed to parse scala");
    assert_eq!(slendro.degrees_cents.len(), 6);
    assert!((slendro.degrees_cents[1] - 240.0).abs() < 1e-4);

    // Microtonal calculation verification
    let s69 = slendro.note_to_freq(69);
    assert!((s69 - 440.0).abs() < 1e-4);

    let s70 = slendro.note_to_freq(70);
    let expected_s70 = 440.0 * 2.0f64.powf(240.0 / 1200.0);
    assert!((s70 - expected_s70).abs() < 1e-4);
    // Ensure it differs from standard 12-TET note 70 (466.16 Hz)
    assert!((s70 - default_tuning.note_to_freq(70)).abs() > 30.0);

    // 5 notes up in 5-TET is an octave (880.0 Hz)
    let s74 = slendro.note_to_freq(74);
    assert!((s74 - 880.0).abs() < 1e-4);
}

#[test]
fn test_note_name_follows_live_convention() {
    assert_eq!(note_name(60), "C3");
    assert_eq!(note_name(69), "A3");
    assert_eq!(note_name(0), "C-2");
    assert_eq!(note_name(127), "G8");
    assert_eq!(note_name(61), "C#3");
}

#[test]
fn test_track_lookup_covers_every_track_kind() {
    let mut proj = Project::new_default("Lookup");
    assert_eq!(
        proj.track(TrackId(1)).map(|t| t.track_type),
        Some(TrackType::Midi)
    );
    assert_eq!(
        proj.track(TrackId(5)).map(|t| t.track_type),
        Some(TrackType::Return)
    );
    assert!(proj.track(TrackId(7)).is_some());
    assert!(proj.track(TrackId(99)).is_none());

    proj.track_mut(TrackId(6)).expect("return track").name = "Echo".to_string();
    assert_eq!(proj.return_tracks[1].name, "Echo");
}

#[test]
fn test_id_allocator_starts_above_every_existing_id() {
    // The default Set's highest id is the Limiter device (301).
    let proj = Project::new_default("Seed");
    let mut ids = proj.id_allocator();
    assert_eq!(ids.track(), TrackId(302));
    assert_eq!(ids.device(), DeviceId(303));
}

#[test]
fn test_midi_note_defaults_match_live_properties() {
    let note = MidiNote::new(60, 0.0, 1.0, 100);
    assert_eq!(note.velocity, 100.0);
    assert_eq!(note.release_velocity, 64.0);
    assert_eq!(note.probability, 1.0);
    assert_eq!(note.velocity_deviation, 0.0);
    assert!(!note.mute);
}

#[test]
fn test_transport_tempo_and_signature_round_trip_through_json() {
    let mut proj = Project::new_default("Transport");
    proj.transport.set_bpm(140.0);
    proj.transport
        .set_time_signature(7, 8)
        .expect("7/8 is a valid signature");

    let json = serde_json::to_string(&proj).expect("serialise project");
    let back: Project = serde_json::from_str(&json).expect("deserialise project");
    assert_eq!(back.transport.bpm(), 140.0);
    assert_eq!(back.transport.time_signature(), (7, 8));
}

#[test]
fn test_transport_rejects_an_invalid_time_signature() {
    let mut proj = Project::new_default("Invalid");
    assert!(proj.transport.set_time_signature(4, 3).is_err());
    assert_eq!(proj.transport.time_signature(), (4, 4));
}
