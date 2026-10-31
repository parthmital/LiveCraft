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

#[test]
fn test_group_tracks_nest_and_hold_no_clips() {
    let mut proj = Project::new_default("Groups");
    assert_eq!(proj.main_track.track_type, TrackType::Main);

    // Group tracks get no clip slots, whatever the caller asks for (manual 18.3, page 400).
    let outer = Track::new(TrackId(20), "Drums", TrackType::Group, 8);
    let mut inner = Track::new(TrackId(21), "Kit", TrackType::Group, 8);
    let mut kick = Track::new(TrackId(22), "Kick", TrackType::Audio, 8);
    assert!(outer.clip_slots.is_empty());
    assert!(inner.clip_slots.is_empty());

    // A group nested in a group (manual 18.3, page 400; soloing a nested group, page 402).
    inner.parent = Some(outer.id);
    kick.parent = Some(inner.id);
    proj.tracks.push(outer);
    proj.tracks.push(inner);
    proj.tracks.push(kick);

    assert_eq!(
        proj.track(TrackId(21)).and_then(|t| t.parent),
        Some(TrackId(20))
    );
    assert_eq!(
        proj.track(TrackId(22)).and_then(|t| t.parent),
        Some(TrackId(21))
    );
    assert_eq!(proj.track(TrackId(20)).and_then(|t| t.parent), None);
}

#[test]
fn test_routing_defaults_to_main_and_round_trips_track_targets() {
    let plain = Track::new(TrackId(31), "Plain", TrackType::Audio, 0);
    assert!(plain.routing.is_default());
    assert_eq!(plain.routing.audio_to, AudioOutput::Main);
    let json = serde_json::to_string(&plain).expect("serialise track");
    assert!(!json.contains("routing"), "default routing is not exported");

    let mut synth = Track::new(TrackId(30), "Synth", TrackType::Midi, 0);
    synth.routing.audio_to = AudioOutput::Track(TrackId(20));
    synth.routing.audio_from = AudioInput::Track {
        track: TrackId(5),
        tap: TapPoint::PostFx,
    };
    let json = serde_json::to_string(&synth).expect("serialise track");
    let back: Track = serde_json::from_str(&json).expect("deserialise track");
    assert_eq!(back.routing, synth.routing);
}

#[test]
fn test_device_parameters_are_stored_by_id_and_addressed_by_name() {
    let mut eq = Device::new(DeviceId(1), "EQ", DeviceType::EqEight);
    assert_eq!(eq.parameters.iter().count(), 32);
    assert_eq!(eq.get_param("band_8_freq"), 12800.0);
    // Band 8 starts at index 28 of the EQ Eight table; its frequency is the first entry of that band.
    assert_eq!(
        param_id(param_table(&DeviceType::EqEight), "band_8_freq"),
        Some(ParamId(28))
    );

    eq.set_param("band_2_gain", 3.5);
    assert_eq!(eq.parameters.get(ParamId(5)), Some(3.5));
    assert_eq!(eq.param("band_2_gain"), Some(3.5));

    // Names outside the device kind's table are ignored, and read as 0.0.
    eq.set_param("no_such_param", 9.0);
    assert_eq!(eq.parameters.iter().count(), 32);
    assert_eq!(eq.get_param("no_such_param"), 0.0);
}

#[test]
fn test_device_json_writes_names_and_leaves_out_unset_parameters() {
    let op = Device::new(DeviceId(4), "Operator", DeviceType::Operator);
    let json = serde_json::to_value(&op).expect("serialise device");
    assert_eq!(json["parameters"]["osc1_level"], 1.0);
    assert!(json["parameters"].get("algorithm").is_none());

    let back: Device = serde_json::from_value(json).expect("deserialise device");
    assert_eq!(back.get_param("filter_cutoff"), 2400.0);
    assert_eq!(back.param("algorithm"), None);
}
