//! Track commands across the Set (M04 item 4): creating, deleting with exact undo, naming, color,
//! order, and the mixer settings.

use livecraft_model::edit::EditError;
use livecraft_model::ids::{IdAllocator, TrackId};
use livecraft_model::model::{MonitoringMode, Project, TrackType};
use livecraft_session::commands::track::{
    CreateAudioTrack, CreateGroupTrack, CreateReturnTrack, DeleteTrack, RenameTrack, ReorderTrack,
    SetTrackArmed, SetTrackColor, SetTrackMonitoring, SetTrackMuted, SetTrackPan, SetTrackSend,
    SetTrackSoloed,
};
use livecraft_session::{Command, CommandError, Operation, UndoHistory};
use std::time::{Duration, Instant};

/// Runs one command at `at`, so a test controls which commands merge into one gesture.
fn run_at<O: Operation>(
    history: &mut UndoHistory,
    project: &mut Project,
    ids: &mut IdAllocator,
    operation: O,
    at: Instant,
) -> Result<(), CommandError> {
    let command: Box<dyn Command> = Box::new(operation);
    history.execute(command, project, ids, at)
}

fn track_order(project: &Project) -> Vec<u64> {
    project.tracks.iter().map(|track| track.id.0).collect()
}

#[test]
fn a_return_track_gets_a_send_on_every_track_and_undo_removes_it() {
    let mut project = Project::new_default("Return");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let now = Instant::now();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        CreateReturnTrack::new("C-Echo"),
        now,
    )
    .expect("create return");
    let created = project.return_tracks[2].id;
    assert_eq!(project.return_tracks.len(), 3);
    assert!(project.all_tracks().all(|track| track.sends.len() == 3));
    assert!(project.validate().is_empty());

    assert!(history.undo(&mut project));
    assert_eq!(project.return_tracks.len(), 2);
    assert!(project.all_tracks().all(|track| track.sends.len() == 2));
    assert!(project.track(created).is_none());

    assert!(history.redo(&mut project, &mut ids).expect("redo"));
    assert_eq!(project.return_tracks[2].id, created);
    assert!(project.validate().is_empty());
}

#[test]
fn a_group_track_holds_no_clip_slots() {
    let mut project = Project::new_default("Group");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        CreateGroupTrack::new("Drums"),
        Instant::now(),
    )
    .expect("create group");
    let group = project.tracks.last().expect("group track");
    assert_eq!(group.track_type, TrackType::Group);
    assert!(group.clip_slots.is_empty());
    assert!(project.validate().is_empty());
}

#[test]
fn created_audio_tracks_are_named_and_undone() {
    let mut project = Project::new_default("Audio");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        CreateAudioTrack::new("5-Audio"),
        Instant::now(),
    )
    .expect("create audio");
    assert_eq!(
        project.tracks.last().map(|t| t.name.as_str()),
        Some("5-Audio")
    );
    assert!(history.undo(&mut project));
    assert_eq!(project.tracks.len(), 4);
}

#[test]
fn deleting_a_return_restores_its_send_values_and_position_on_undo() {
    let mut project = Project::new_default("Delete return");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let now = Instant::now();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTrackSend::new(TrackId(1), TrackId(5), 0.5),
        now,
    )
    .expect("send to A-Reverb");
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        DeleteTrack::new(TrackId(5)),
        now,
    )
    .expect("delete A-Reverb");
    assert_eq!(project.return_tracks.len(), 1);
    let sends = |project: &Project| project.track(TrackId(1)).map(|t| t.sends.clone());
    assert_eq!(sends(&project), Some(vec![0.0]));
    assert!(project.validate().is_empty());

    assert!(history.undo(&mut project));
    assert_eq!(project.return_tracks[0].id, TrackId(5));
    assert_eq!(sends(&project), Some(vec![0.5, 0.0]));
    assert!(project.validate().is_empty());
}

#[test]
fn deleting_a_group_moves_its_children_up_and_undo_puts_them_back() {
    let mut project = Project::new_default("Delete group");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let now = Instant::now();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        CreateGroupTrack::new("Drums"),
        now,
    )
    .expect("create group");
    let group = project.tracks.last().map(|t| t.id).expect("group");
    project
        .set_parent(TrackId(1), Some(group))
        .expect("put track 1 in the group");

    run_at(
        &mut history,
        &mut project,
        &mut ids,
        DeleteTrack::new(group),
        now,
    )
    .expect("delete group");
    assert_eq!(project.track(TrackId(1)).and_then(|t| t.parent), None);
    assert!(project.validate().is_empty());

    assert!(history.undo(&mut project));
    assert_eq!(
        project.track(TrackId(1)).and_then(|t| t.parent),
        Some(group)
    );
    assert!(project.validate().is_empty());
}

#[test]
fn the_main_track_cannot_be_deleted() {
    let mut project = Project::new_default("Main");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let result = run_at(
        &mut history,
        &mut project,
        &mut ids,
        DeleteTrack::new(TrackId(7)),
        Instant::now(),
    );
    assert_eq!(result, Err(CommandError::Edit(EditError::MainTrackFixed)));
    assert!(!history.can_undo());
}

#[test]
fn renaming_coloring_and_toggling_track_flags_undo_to_the_previous_values() {
    let mut project = Project::new_default("Flags");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let now = Instant::now();
    let track = |project: &Project| project.track(TrackId(1)).expect("track 1").clone();
    let original = track(&project);

    run_at(
        &mut history,
        &mut project,
        &mut ids,
        RenameTrack::new(TrackId(1), "Lead".to_string()),
        now,
    )
    .expect("rename");
    assert_eq!(track(&project).name, "Lead");
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTrackColor::new(TrackId(1), "#ff0000"),
        now,
    )
    .expect("color");
    assert_eq!(track(&project).color, "#ff0000");
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTrackArmed::new(TrackId(1), true),
        now,
    )
    .expect("arm");
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTrackSoloed::new(TrackId(1), true),
        now,
    )
    .expect("solo");
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTrackMuted::new(TrackId(1), true),
        now,
    )
    .expect("mute");
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTrackMonitoring::new(TrackId(1), MonitoringMode::In),
        now,
    )
    .expect("monitor");
    let after = track(&project);
    assert!(after.is_armed && after.is_soloed && after.is_muted);
    assert_eq!(after.monitoring, MonitoringMode::In);

    for _ in 0..6 {
        assert!(history.undo(&mut project));
    }
    let before = track(&project);
    assert_eq!(before.name, original.name);
    assert_eq!(before.color, original.color);
    assert_eq!(before.is_armed, original.is_armed);
    assert_eq!(before.is_soloed, original.is_soloed);
    assert_eq!(before.is_muted, original.is_muted);
    assert_eq!(before.monitoring, original.monitoring);
}

#[test]
fn a_color_must_be_a_hex_color() {
    let mut project = Project::new_default("Color");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    for color in ["red", "#ff00", "#ff00zz", "ff0000"] {
        let error = run_at(
            &mut history,
            &mut project,
            &mut ids,
            SetTrackColor::new(TrackId(1), color),
            Instant::now(),
        )
        .err();
        assert_eq!(
            error,
            Some(CommandError::BadColor {
                value: color.to_string()
            })
        );
    }
    assert!(!history.can_undo());
}

#[test]
fn reordering_moves_a_track_and_undo_moves_it_back() {
    let mut project = Project::new_default("Order");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    assert_eq!(track_order(&project), [1, 2, 3, 4]);
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        ReorderTrack::new(TrackId(1), 2),
        Instant::now(),
    )
    .expect("move track 1 to position 2");
    assert_eq!(track_order(&project), [2, 3, 1, 4]);
    assert!(history.undo(&mut project));
    assert_eq!(track_order(&project), [1, 2, 3, 4]);
}

#[test]
fn the_main_track_does_not_move() {
    let mut project = Project::new_default("Main order");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let result = run_at(
        &mut history,
        &mut project,
        &mut ids,
        ReorderTrack::new(TrackId(7), 0),
        Instant::now(),
    );
    assert_eq!(result, Err(CommandError::Edit(EditError::MainTrackFixed)));
}

#[test]
fn pan_and_send_values_are_range_checked_and_a_drag_is_one_step() {
    let mut project = Project::new_default("Mixer");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let out_of_range = run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTrackPan::new(TrackId(1), 1.5),
        Instant::now(),
    )
    .err();
    assert!(matches!(
        out_of_range,
        Some(CommandError::OutOfRange {
            what: "track pan",
            ..
        })
    ));

    let not_a_return = run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTrackSend::new(TrackId(1), TrackId(2), 0.5),
        Instant::now(),
    )
    .err();
    assert_eq!(
        not_a_return,
        Some(CommandError::Edit(EditError::NoSuchTrack(TrackId(2))))
    );

    let t0 = Instant::now();
    for (offset_ms, level) in [(0, 0.2), (150, 0.4), (300, 0.6)] {
        run_at(
            &mut history,
            &mut project,
            &mut ids,
            SetTrackSend::new(TrackId(1), TrackId(5), level),
            t0 + Duration::from_millis(offset_ms),
        )
        .expect("send drag");
    }
    let sends = |project: &Project| project.track(TrackId(1)).map(|t| t.sends[0]);
    assert_eq!(sends(&project), Some(0.6));
    assert!(history.undo(&mut project));
    assert!(!history.can_undo(), "the send drag is one step");
    assert_eq!(sends(&project), Some(0.0));
}
