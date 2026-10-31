//! Commands and the undo history across modules: the two M01 undo tests, ported (M04 item 3), the
//! behaviour of each registered command, and the registry round trip for every command.

use livecraft_model::automation::{AutomationLane, AutomationPoint, AutomationTarget};
use livecraft_model::edit::EditError;
use livecraft_model::ids::{LaneId, TrackId};
use livecraft_model::model::Project;
use livecraft_session::commands;
use livecraft_session::commands::automation::{
    AddAutomationPoint, MoveAutomationPoint, RemoveAutomationPoint,
};
use livecraft_session::commands::track::{CreateAudioTrack, SetTrackVolume};
use livecraft_session::commands::transport::SetTempo;
use livecraft_session::{CommandError, UndoHistory};
use serde_json::json;
use std::time::{Duration, Instant};

/// Points of the first lane of track 1, as (beat, value) pairs.
fn lane_points(project: &Project) -> Vec<(f64, f32)> {
    project.tracks[0].automation_lanes[0]
        .points
        .iter()
        .map(|point| (point.beat, point.value))
        .collect()
}

fn with_lanes() -> Project {
    let mut project = Project::new_default("Automation");
    project.tracks[0].automation_lanes.push(AutomationLane::new(
        LaneId(1),
        AutomationTarget::TrackVolume,
    ));
    project.tracks[0]
        .automation_lanes
        .push(AutomationLane::new(LaneId(2), AutomationTarget::TrackPan));
    project
}

/// The M01 test `test_command_undo_redo`, ported: tempo and automation points undo and redo.
#[test]
fn tempo_and_automation_points_undo_and_redo() {
    let mut project = with_lanes();
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let t0 = Instant::now();
    let at = |secs: u64| t0 + Duration::from_secs(secs);

    assert_eq!(project.transport.bpm(), 120.0);
    history
        .execute(
            Box::new(SetTempo::new(140.0)),
            &mut project,
            &mut ids,
            at(0),
        )
        .expect("set tempo");
    assert_eq!(project.transport.bpm(), 140.0);
    assert!(history.undo(&mut project));
    assert_eq!(project.transport.bpm(), 120.0);
    assert!(history.redo(&mut project, &mut ids).expect("redo tempo"));
    assert_eq!(project.transport.bpm(), 140.0);

    assert_eq!(project.tracks[0].automation_lanes[0].evaluate_at(1.0), None);
    let add = |beat: f64, value: f32| {
        Box::new(AddAutomationPoint::new(
            TrackId(1),
            LaneId(1),
            AutomationPoint::new(beat, value, 0.0),
        ))
    };
    history
        .execute(add(0.0, 0.0), &mut project, &mut ids, at(1))
        .expect("add first point");
    history
        .execute(add(4.0, 1.0), &mut project, &mut ids, at(2))
        .expect("add second point");
    assert_eq!(lane_points(&project), [(0.0, 0.0), (4.0, 1.0)]);
    let halfway = project.tracks[0].automation_lanes[0]
        .evaluate_at(2.0)
        .expect("a value between the points");
    assert!((halfway - 0.5).abs() < 1e-5);

    assert!(history.undo(&mut project));
    assert_eq!(lane_points(&project), [(0.0, 0.0)]);
    assert!(history.redo(&mut project, &mut ids).expect("redo add"));
    assert_eq!(lane_points(&project).len(), 2);

    history
        .execute(
            Box::new(RemoveAutomationPoint::new(TrackId(1), LaneId(1), 0)),
            &mut project,
            &mut ids,
            at(3),
        )
        .expect("remove first point");
    assert_eq!(lane_points(&project), [(4.0, 1.0)]);
    assert!(history.undo(&mut project));
    assert_eq!(lane_points(&project), [(0.0, 0.0), (4.0, 1.0)]);
    assert!(history.redo(&mut project, &mut ids).expect("redo remove"));
    assert_eq!(lane_points(&project), [(4.0, 1.0)]);
}

/// The M01 test `test_command_history_capacity`, ported: a limit of five keeps five undo steps.
#[test]
fn the_history_keeps_only_as_many_steps_as_its_limit() {
    let mut project = Project::new_default("HistoryCapTest");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::with_limit(5);
    let t0 = Instant::now();

    for step in 1..=10u64 {
        // One second apart, so no two tempo changes merge into one gesture.
        history
            .execute(
                Box::new(SetTempo::new(120.0 + step as f32)),
                &mut project,
                &mut ids,
                t0 + Duration::from_secs(step),
            )
            .expect("set tempo");
    }
    assert_eq!(project.transport.bpm(), 130.0);
    let mut undos = 0;
    while history.undo(&mut project) {
        undos += 1;
    }
    assert_eq!(undos, 5);
    assert_eq!(project.transport.bpm(), 125.0);
}

#[test]
fn a_fader_drag_is_one_undo_step() {
    let mut project = Project::new_default("Fader");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let t0 = Instant::now();
    for (offset_ms, volume) in [(0, -1.0), (100, -2.0), (200, -3.0)] {
        history
            .execute(
                Box::new(SetTrackVolume::new(TrackId(1), volume)),
                &mut project,
                &mut ids,
                t0 + Duration::from_millis(offset_ms),
            )
            .expect("set volume");
    }
    assert_eq!(project.track(TrackId(1)).map(|t| t.volume_db), Some(-3.0));
    assert!(history.undo(&mut project));
    assert!(!history.can_undo(), "the drag is one step");
    assert_eq!(project.track(TrackId(1)).map(|t| t.volume_db), Some(0.0));
}

#[test]
fn a_created_track_keeps_its_id_through_undo_and_redo() {
    let mut project = Project::new_default("Create");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    history
        .execute(
            Box::new(CreateAudioTrack::new("5-Audio")),
            &mut project,
            &mut ids,
            Instant::now(),
        )
        .expect("create track");
    let created = project
        .tracks
        .last()
        .map(|track| track.id)
        .expect("new track");
    assert_eq!(project.tracks.len(), 5);

    assert!(history.undo(&mut project));
    assert!(project.track(created).is_none());
    assert!(history.redo(&mut project, &mut ids).expect("redo create"));
    assert_eq!(
        project.track(created).map(|t| t.name.as_str()),
        Some("5-Audio")
    );

    // A later command that names the track finds it after the redo.
    history
        .execute(
            Box::new(SetTrackVolume::new(created, -6.0)),
            &mut project,
            &mut ids,
            Instant::now(),
        )
        .expect("set volume of the redone track");
    assert_eq!(project.track(created).map(|t| t.volume_db), Some(-6.0));
}

#[test]
fn removing_and_moving_points_undo_to_the_exact_previous_order() {
    let mut project = with_lanes();
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let t0 = Instant::now();
    for beat in [0.0, 4.0] {
        history
            .execute(
                Box::new(AddAutomationPoint::new(
                    TrackId(1),
                    LaneId(1),
                    AutomationPoint::new(beat, beat as f32, 0.0),
                )),
                &mut project,
                &mut ids,
                t0,
            )
            .expect("add point");
    }

    // Moving the first point past the second re-sorts the lane.
    history
        .execute(
            Box::new(MoveAutomationPoint::new(
                TrackId(1),
                LaneId(1),
                0,
                AutomationPoint::new(8.0, 0.5, 0.0),
            )),
            &mut project,
            &mut ids,
            t0,
        )
        .expect("move point");
    assert_eq!(lane_points(&project), [(4.0, 4.0), (8.0, 0.5)]);
    assert!(history.undo(&mut project));
    assert_eq!(lane_points(&project), [(0.0, 0.0), (4.0, 4.0)]);
    assert!(history.redo(&mut project, &mut ids).expect("redo move"));
    assert_eq!(lane_points(&project), [(4.0, 4.0), (8.0, 0.5)]);
}

#[test]
fn a_command_on_a_missing_target_fails_and_changes_nothing() {
    let mut project = Project::new_default("Missing");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let result = history.execute(
        Box::new(SetTrackVolume::new(TrackId(99), -6.0)),
        &mut project,
        &mut ids,
        Instant::now(),
    );
    assert_eq!(
        result,
        Err(CommandError::Edit(EditError::NoSuchTrack(TrackId(99))))
    );
    assert!(!history.can_undo());
}

#[test]
fn a_point_on_a_missing_lane_is_an_error() {
    let mut project = Project::new_default("Lane");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let result = history.execute(
        Box::new(RemoveAutomationPoint::new(TrackId(1), LaneId(7), 0)),
        &mut project,
        &mut ids,
        Instant::now(),
    );
    assert_eq!(
        result,
        Err(CommandError::NoSuchLane {
            track: TrackId(1),
            lane: LaneId(7),
        })
    );
}

#[test]
fn tempo_outside_the_allowed_range_and_nan_are_refused() {
    let mut project = Project::new_default("Tempo");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    for bpm in [f32::NAN, 1.0, 1000.0] {
        let error = history
            .execute(
                Box::new(SetTempo::new(bpm)),
                &mut project,
                &mut ids,
                Instant::now(),
            )
            .err();
        assert!(
            matches!(error, Some(CommandError::OutOfRange { what: "tempo", .. })),
            "tempo {bpm} should be refused, got {error:?}"
        );
    }
    assert_eq!(project.transport.bpm(), 120.0);
}

#[test]
fn every_registered_command_round_trips_its_arguments() {
    let registry = commands::registry();
    let samples = [
        (
            "automation.add_point",
            json!({ "track": 1, "lane": 1, "point": { "beat": 4.0, "value": 1.0, "curve": 0.0 } }),
        ),
        (
            "automation.move_point",
            json!({ "track": 1, "lane": 1, "index": 0, "point": { "beat": 2.0, "value": 0.5, "curve": 0.0 } }),
        ),
        (
            "automation.remove_point",
            json!({ "track": 1, "lane": 1, "index": 0 }),
        ),
        (
            "clip.create_audio",
            json!({ "track": 3, "slot": 0, "name": "Loop", "file_path": "loop.wav", "length_beats": 8.0 }),
        ),
        (
            "clip.create_midi",
            json!({ "track": 1, "slot": 0, "name": "Intro", "length_beats": 4.0 }),
        ),
        ("clip.delete", json!({ "clip": 9 })),
        (
            "clip.duplicate",
            json!({ "clip": 9, "track": 1, "slot": 1 }),
        ),
        ("clip.move", json!({ "clip": 9, "track": 1, "slot": 2 })),
        ("clip.rename", json!({ "clip": 9, "name": "Verse" })),
        (
            "clip.set_loop",
            json!({ "clip": 9, "loop_start_beat": 0.0, "loop_end_beat": 4.0, "is_looping": true }),
        ),
        (
            "clip.set_markers",
            json!({ "clip": 9, "start_marker_beat": 0.0, "end_marker_beat": 4.0 }),
        ),
        (
            "device.insert",
            json!({ "track": 3, "device_type": "Compressor", "name": "Compressor" }),
        ),
        (
            "device.move",
            json!({ "track": 3, "device": 102, "index": 0 }),
        ),
        ("device.remove", json!({ "track": 3, "device": 102 })),
        (
            "device.set_enabled",
            json!({ "track": 3, "device": 102, "enabled": false }),
        ),
        (
            "device.set_param",
            json!({ "track": 3, "device": 102, "param": "band_1_gain", "value": 3.0 }),
        ),
        (
            "note.add",
            json!({ "clip": 9, "note": { "pitch": 60, "start_beat": 0.0, "duration_beats": 1.0, "velocity": 100.0, "release_velocity": 64.0, "probability": 1.0, "velocity_deviation": 0.0, "mute": false } }),
        ),
        (
            "note.edit",
            json!({ "clip": 9, "index": 0, "note": { "pitch": 64, "start_beat": 0.0, "duration_beats": 2.0, "velocity": 90.0, "release_velocity": 64.0, "probability": 1.0, "velocity_deviation": 0.0, "mute": false } }),
        ),
        ("note.remove", json!({ "clip": 9, "index": 0 })),
        ("scene.create", json!({ "name": "9" })),
        ("scene.delete", json!({ "scene": 3 })),
        ("scene.duplicate", json!({ "scene": 3 })),
        ("track.create_audio", json!({ "name": "5-Audio" })),
        ("track.create_group", json!({ "name": "Drums" })),
        ("track.create_midi", json!({ "name": "6-MIDI" })),
        ("track.create_return", json!({ "name": "C-Echo" })),
        ("track.delete", json!({ "track": 3 })),
        ("track.rename", json!({ "track": 1, "name": "Lead" })),
        ("track.reorder", json!({ "track": 1, "index": 2 })),
        ("track.set_armed", json!({ "track": 1, "is_armed": true })),
        ("track.set_color", json!({ "track": 1, "color": "#ff4848" })),
        (
            "track.set_monitoring",
            json!({ "track": 1, "monitoring": "In" }),
        ),
        ("track.set_muted", json!({ "track": 1, "is_muted": true })),
        ("track.set_pan", json!({ "track": 1, "pan": -0.5 })),
        (
            "track.set_send",
            json!({ "track": 1, "return": 5, "level": 0.25 }),
        ),
        (
            "track.set_soloed",
            json!({ "track": 1, "is_soloed": false }),
        ),
        ("track.set_volume", json!({ "track": 1, "volume_db": -6.0 })),
        (
            "transport.set_loop",
            json!({ "loop_enabled": true, "loop_start_beats": 4.0, "loop_length_beats": 8.0 }),
        ),
        (
            "transport.set_metronome",
            json!({ "metronome_enabled": true }),
        ),
        ("transport.set_tempo", json!({ "bpm": 140.0 })),
        (
            "transport.set_time_signature",
            json!({ "numerator": 7, "denominator": 8 }),
        ),
    ];
    let names: Vec<&str> = registry.names().collect();
    let sampled: Vec<&str> = samples.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, sampled, "every registered command needs a sample");
    for (name, args) in samples {
        let command = registry.build(name, args.clone()).expect("build sample");
        assert_eq!(command.arguments(), args, "round trip of `{name}`");
    }
}
