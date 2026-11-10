use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project, Track};
use livecraft_time::Beats;

pub fn assert_approx_f32(a: f32, b: f32, tol: f32, ctx: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{ctx}: expected {a}, got {b} (diff {})",
        (a - b).abs()
    );
}

pub fn assert_approx_f64(a: f64, b: f64, tol: f64, ctx: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{ctx}: expected {a}, got {b} (diff {})",
        (a - b).abs()
    );
}

pub fn assert_clips_equivalent(a: &Clip, b: &Clip) {
    assert_eq!(a.id, b.id, "clip id");
    assert_eq!(a.name, b.name, "clip name");
    assert_eq!(a.color, b.color, "clip color");
    assert_approx_f64(a.length_beats, b.length_beats, 1e-6, "clip length");
    assert_approx_f64(a.loop_start_beat, b.loop_start_beat, 1e-6, "loop start");
    assert_approx_f64(a.loop_end_beat, b.loop_end_beat, 1e-6, "loop end");
    assert_eq!(a.is_looping, b.is_looping, "is_looping");
    assert_approx_f64(
        a.start_marker_beat,
        b.start_marker_beat,
        1e-6,
        "start marker",
    );
    assert_approx_f64(a.end_marker_beat, b.end_marker_beat, 1e-6, "end marker");
    assert_eq!(a.launch.mode, b.launch.mode);
    assert_eq!(a.launch.quantization, b.launch.quantization);
    assert_eq!(a.launch.legato, b.launch.legato);
    assert_approx_f32(
        a.launch.velocity_amount,
        b.launch.velocity_amount,
        1e-4,
        "vel amt",
    );
    assert_eq!(a.follow.enabled, b.follow.enabled);
    assert_eq!(a.follow.action_a, b.follow.action_a);
    assert_eq!(a.follow.action_b, b.follow.action_b);
    assert_approx_f32(a.follow.chance_a, b.follow.chance_a, 1e-4, "chance a");
    assert_approx_f32(a.follow.chance_b, b.follow.chance_b, 1e-4, "chance b");
    assert_eq!(a.follow.linked, b.follow.linked);
    assert_eq!(a.follow.multiplier, b.follow.multiplier);
    assert_approx_f64(
        a.follow.time_beats,
        b.follow.time_beats,
        1e-6,
        "follow time",
    );

    match (&a.payload, &b.payload) {
        (ClipPayload::Midi(ma), ClipPayload::Midi(mb)) => {
            let mut na = ma.notes.clone();
            let mut nb = mb.notes.clone();
            let key = |n: &MidiNote| (n.pitch, (n.start_beat * 1e6) as i64);
            na.sort_by_key(key);
            nb.sort_by_key(key);
            assert_eq!(na.len(), nb.len(), "note count");
            for (x, y) in na.iter().zip(&nb) {
                assert_eq!(x.pitch, y.pitch);
                assert_approx_f64(x.start_beat, y.start_beat, 1e-6, "note start");
                assert_approx_f64(x.duration_beats, y.duration_beats, 1e-6, "note dur");
                assert_approx_f32(x.velocity, y.velocity, 1e-4, "note vel");
                assert_approx_f32(x.release_velocity, y.release_velocity, 1e-4, "note rel vel");
                assert_approx_f32(x.probability, y.probability, 1e-4, "note prob");
                assert_approx_f32(
                    x.velocity_deviation,
                    y.velocity_deviation,
                    1e-4,
                    "note vel dev",
                );
                assert_eq!(x.mute, y.mute);
            }
        }
        (ClipPayload::Audio(aa), ClipPayload::Audio(ab)) => {
            assert_eq!(aa.file_path, ab.file_path);
            assert_eq!(aa.relative_path, ab.relative_path);
            assert_eq!(aa.sample_rate, ab.sample_rate);
            assert_eq!(aa.channels, ab.channels);
            assert_eq!(aa.warp_mode, ab.warp_mode);
            assert_eq!(aa.is_warped, ab.is_warped);
            assert_approx_f64(aa.original_bpm, ab.original_bpm, 1e-6, "orig bpm");
            assert_approx_f32(aa.gain_db, ab.gain_db, 1e-3, "gain db");
            assert_eq!(aa.pitch_cents, ab.pitch_cents);
            assert_eq!(aa.warp_markers.len(), ab.warp_markers.len());
            for (x, y) in aa.warp_markers.iter().zip(&ab.warp_markers) {
                assert_approx_f64(x.sample_time_sec, y.sample_time_sec, 1e-6, "wm sec");
                assert_approx_f64(x.beat_time, y.beat_time, 1e-6, "wm beat");
            }
        }
        _ => panic!("mismatched clip payload types"),
    }
}

pub fn assert_tracks_equivalent(a: &Track, b: &Track) {
    assert_eq!(a.id, b.id, "track id");
    assert_eq!(a.name, b.name, "track name");
    assert_eq!(a.track_type, b.track_type, "track type");
    assert_eq!(a.parent, b.parent, "track parent");
    assert_eq!(a.routing, b.routing, "track routing");
    assert_eq!(a.color, b.color, "track color");
    assert_approx_f32(a.volume_db, b.volume_db, 1e-3, "track volume");
    assert_approx_f32(a.pan, b.pan, 1e-4, "track pan");
    assert_eq!(a.is_muted, b.is_muted, "track muted");
    assert_eq!(a.is_soloed, b.is_soloed, "track soloed");
    assert_eq!(a.is_armed, b.is_armed, "track armed");
    assert_eq!(a.monitoring, b.monitoring, "track monitoring");
    assert_eq!(a.sends.len(), b.sends.len(), "track sends len");
    for (i, (&x, &y)) in a.sends.iter().zip(&b.sends).enumerate() {
        assert_approx_f32(x, y, 1e-4, &format!("send[{i}]"));
    }
    assert_eq!(a.clip_slots.len(), b.clip_slots.len(), "slots len");
    for (sa, sb) in a.clip_slots.iter().zip(&b.clip_slots) {
        assert_eq!(sa.id, sb.id);
        assert_eq!(sa.has_stop_button, sb.has_stop_button);
        match (&sa.clip, &sb.clip) {
            (None, None) => {}
            (Some(ca), Some(cb)) => assert_clips_equivalent(ca, cb),
            _ => panic!("slot clip presence mismatch"),
        }
    }
    assert_eq!(a.arrangement_clips.len(), b.arrangement_clips.len());
    for (aa, ab) in a.arrangement_clips.iter().zip(&b.arrangement_clips) {
        assert_eq!(aa.id, ab.id);
        assert_approx_f64(aa.start_beat, ab.start_beat, 1e-6, "arr start");
        assert_approx_f64(aa.length_beats, ab.length_beats, 1e-6, "arr len");
        assert_clips_equivalent(&aa.clip, &ab.clip);
    }
    assert_eq!(a.device_chain.len(), b.device_chain.len(), "devices len");
    for (da, db) in a.device_chain.iter().zip(&b.device_chain) {
        assert_eq!(da.id, db.id);
        assert_eq!(da.name, db.name);
        assert_eq!(da.device_type, db.device_type);
        assert_eq!(da.is_enabled, db.is_enabled);
        let pa: Vec<_> = da.named_params().collect();
        let pb: Vec<_> = db.named_params().collect();
        assert_eq!(pa.len(), pb.len(), "param count for {:?}", da.device_type);
        for ((ka, va), (kb, vb)) in pa.iter().zip(&pb) {
            assert_eq!(ka, kb);
            assert_approx_f32(*va, *vb, 1e-4, ka);
        }
    }
    assert_eq!(
        a.automation_lanes.len(),
        b.automation_lanes.len(),
        "lanes len"
    );
    for (la, lb) in a.automation_lanes.iter().zip(&b.automation_lanes) {
        assert_eq!(la.id, lb.id);
        assert_eq!(la.target, lb.target);
        assert_eq!(la.is_enabled, lb.is_enabled);
        assert_eq!(la.points.len(), lb.points.len());
        for (pa, pb) in la.points.iter().zip(&lb.points) {
            assert_approx_f64(pa.beat, pb.beat, 1e-6, "auto beat");
            assert_approx_f32(pa.value, pb.value, 1e-4, "auto val");
            assert_approx_f32(pa.curve, pb.curve, 1e-3, "auto curve");
        }
    }
}

pub fn assert_projects_equivalent(a: &Project, b: &Project) {
    for beat in [0.0, 1.5, 4.0, 8.0, 16.0, 32.0] {
        assert_approx_f64(
            a.transport.tempo.bpm_at(Beats(beat)),
            b.transport.tempo.bpm_at(Beats(beat)),
            1e-5,
            "tempo bpm_at",
        );
    }
    for bar in -2..=32 {
        assert_approx_f64(
            a.transport.time_signature.bar_start(bar).0,
            b.transport.time_signature.bar_start(bar).0,
            1e-9,
            "bar_start",
        );
        assert_eq!(
            a.transport
                .time_signature
                .signature_at(a.transport.time_signature.bar_start(bar)),
            b.transport
                .time_signature
                .signature_at(b.transport.time_signature.bar_start(bar)),
        );
    }
    assert_eq!(a.transport.loop_enabled, b.transport.loop_enabled);
    assert_approx_f64(
        a.transport.loop_start_beats,
        b.transport.loop_start_beats,
        1e-6,
        "loop start",
    );
    assert_approx_f64(
        a.transport.loop_length_beats,
        b.transport.loop_length_beats,
        1e-6,
        "loop len",
    );
    assert_approx_f64(
        a.transport.playhead_beats,
        b.transport.playhead_beats,
        1e-6,
        "playhead",
    );
    assert_eq!(a.transport.punch_in, b.transport.punch_in);
    assert_eq!(a.transport.punch_out, b.transport.punch_out);
    assert_eq!(
        a.transport.global_quantization,
        b.transport.global_quantization
    );
    assert_eq!(a.transport.metronome_enabled, b.transport.metronome_enabled);
    assert_eq!(
        a.transport.follow_actions_enabled,
        b.transport.follow_actions_enabled
    );
    assert_eq!(a.transport.follow_seed, b.transport.follow_seed);
    assert_eq!(
        a.transport.metronome.count_in_bars,
        b.transport.metronome.count_in_bars
    );
    assert_eq!(a.transport.metronome.rhythm, b.transport.metronome.rhythm);
    assert_eq!(a.transport.metronome.sound, b.transport.metronome.sound);
    assert_eq!(
        a.transport.metronome.only_while_recording,
        b.transport.metronome.only_while_recording
    );
    assert_approx_f32(
        a.transport.metronome.volume_db,
        b.transport.metronome.volume_db,
        1e-4,
        "metronome vol",
    );
    assert_eq!(a.scale, b.scale);
    assert_eq!(a.tuning, b.tuning);

    assert_eq!(a.scenes.len(), b.scenes.len());
    for (sa, sb) in a.scenes.iter().zip(&b.scenes) {
        assert_eq!(sa.id, sb.id);
        assert_eq!(sa.name, sb.name);
        assert_eq!(sa.color, sb.color);
        assert_eq!(sa.time_signature, sb.time_signature);
        match (sa.tempo, sb.tempo) {
            (None, None) => {}
            (Some(ta), Some(tb)) => assert_approx_f32(ta, tb, 1e-4, "scene tempo"),
            _ => panic!("scene tempo mismatch"),
        }
    }

    assert_eq!(a.tracks.len(), b.tracks.len());
    for (ta, tb) in a.tracks.iter().zip(&b.tracks) {
        assert_tracks_equivalent(ta, tb);
    }
    assert_eq!(a.return_tracks.len(), b.return_tracks.len());
    for (ra, rb) in a.return_tracks.iter().zip(&b.return_tracks) {
        assert_tracks_equivalent(ra, rb);
    }
    assert_tracks_equivalent(&a.main_track, &b.main_track);
}
