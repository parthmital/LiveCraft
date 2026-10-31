//! Clip, clip slot, and note commands (M04 item 5).

use livecraft_model::automation::{AutomationLane, AutomationTarget};
use livecraft_model::ids::{ClipId, TrackId};
use livecraft_model::model::{ClipPayload, MidiNote, Project};
use livecraft_session::commands::clip::{
    CreateAudioClip, CreateMidiClip, DeleteClip, DuplicateClip, MoveClip, RenameClip, SetClipLoop,
    SetClipMarkers,
};
use livecraft_session::commands::note::{AddNote, EditNote, RemoveNote};
use livecraft_session::{Command, CommandError, Operation, UndoHistory};
use std::time::Instant;

fn run<O: Operation>(
    history: &mut UndoHistory,
    project: &mut Project,
    ids: &mut livecraft_model::ids::IdAllocator,
    operation: O,
) -> Result<(), CommandError> {
    let command: Box<dyn Command> = Box::new(operation);
    history.execute(command, project, ids, Instant::now())
}

/// The id of the clip in a track's slot.
fn clip_id(project: &Project, track: u64, slot: usize) -> Option<ClipId> {
    project
        .track(TrackId(track))
        .and_then(|track| track.clip_slots.get(slot))
        .and_then(|slot| slot.clip.as_ref())
        .map(|clip| clip.id)
}

fn note_count(project: &Project, clip: ClipId) -> Option<usize> {
    project.all_tracks().find_map(|track| {
        track.clip_slots.iter().find_map(|slot| {
            slot.clip
                .as_ref()
                .filter(|known| known.id == clip)
                .and_then(|known| match &known.payload {
                    ClipPayload::Midi(data) => Some(data.notes.len()),
                    ClipPayload::Audio(_) => None,
                })
        })
    })
}

#[test]
fn a_created_clip_keeps_its_id_through_undo_and_redo() {
    let mut project = Project::new_default("Clip");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let created = clip_id(&project, 1, 0).expect("clip in slot 0");
    assert!(project.validate().is_empty());

    assert!(history.undo(&mut project));
    assert_eq!(clip_id(&project, 1, 0), None);
    assert!(history.redo(&mut project, &mut ids).expect("redo"));
    assert_eq!(clip_id(&project, 1, 0), Some(created));
    assert!(project.validate().is_empty());
}

#[test]
fn a_clip_cannot_be_created_in_an_occupied_slot() {
    let mut project = Project::new_default("Occupied");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let error = run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Again", 4.0),
    )
    .err();
    assert_eq!(
        error,
        Some(CommandError::SlotOccupied {
            track: TrackId(1),
            slot: 0
        })
    );
}

#[test]
fn deleting_a_clip_and_undoing_it_restores_its_notes() {
    let mut project = Project::new_default("Delete clip");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let clip = clip_id(&project, 1, 0).expect("clip");
    run(
        &mut history,
        &mut project,
        &mut ids,
        AddNote::new(clip, MidiNote::new(60, 0.0, 1.0, 100)),
    )
    .expect("add note");

    run(&mut history, &mut project, &mut ids, DeleteClip::new(clip)).expect("delete clip");
    assert_eq!(clip_id(&project, 1, 0), None);

    assert!(history.undo(&mut project));
    assert_eq!(clip_id(&project, 1, 0), Some(clip));
    assert_eq!(note_count(&project, clip), Some(1));
    assert!(project.validate().is_empty());
}

#[test]
fn duplicating_a_clip_gives_it_new_ids_and_undo_removes_the_copy() {
    let mut project = Project::new_default("Duplicate");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let original = clip_id(&project, 1, 0).expect("clip");
    let lane = ids.lane();
    project.tracks[0].clip_slots[0]
        .clip
        .as_mut()
        .expect("clip")
        .automation_lanes
        .push(AutomationLane::new(lane, AutomationTarget::TrackVolume));

    run(
        &mut history,
        &mut project,
        &mut ids,
        DuplicateClip::new(original, TrackId(1), 1),
    )
    .expect("duplicate");
    let copy = clip_id(&project, 1, 1).expect("copy in slot 1");
    assert_ne!(copy, original);
    let copy_lane = project.tracks[0].clip_slots[1]
        .clip
        .as_ref()
        .and_then(|clip| clip.automation_lanes.first())
        .map(|lane| lane.id)
        .expect("copied lane");
    assert_ne!(copy_lane, lane, "lane ids stay unique in the Set");
    assert!(project.validate().is_empty());

    assert!(history.undo(&mut project));
    assert_eq!(clip_id(&project, 1, 1), None);
    assert!(project.validate().is_empty());
}

#[test]
fn moving_a_clip_keeps_its_id_and_undo_returns_it() {
    let mut project = Project::new_default("Move");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let clip = clip_id(&project, 1, 0).expect("clip");

    run(
        &mut history,
        &mut project,
        &mut ids,
        MoveClip::new(clip, TrackId(1), 3),
    )
    .expect("move to slot 3");
    assert_eq!(clip_id(&project, 1, 0), None);
    assert_eq!(clip_id(&project, 1, 3), Some(clip));

    assert!(history.undo(&mut project));
    assert_eq!(clip_id(&project, 1, 0), Some(clip));
    assert_eq!(clip_id(&project, 1, 3), None);
}

#[test]
fn a_clip_cannot_move_into_an_occupied_slot() {
    let mut project = Project::new_default("Move refusals");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 1, "Verse", 4.0),
    )
    .expect("create clip");
    let intro = clip_id(&project, 1, 0).expect("intro");
    let occupied = run(
        &mut history,
        &mut project,
        &mut ids,
        MoveClip::new(intro, TrackId(1), 1),
    )
    .err();
    assert_eq!(
        occupied,
        Some(CommandError::SlotOccupied {
            track: TrackId(1),
            slot: 1
        })
    );
    assert_eq!(clip_id(&project, 1, 0), Some(intro));
}

#[test]
fn rename_loop_and_markers_undo_to_their_previous_values() {
    let mut project = Project::new_default("Edit clip");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let clip = clip_id(&project, 1, 0).expect("clip");
    let current = |project: &Project| {
        project.tracks[0].clip_slots[0]
            .clip
            .as_ref()
            .map(|clip| (clip.name.clone(), clip.loop_end_beat, clip.end_marker_beat))
            .expect("clip")
    };
    let before = current(&project);

    run(
        &mut history,
        &mut project,
        &mut ids,
        RenameClip::new(clip, "Verse"),
    )
    .expect("rename");
    run(
        &mut history,
        &mut project,
        &mut ids,
        SetClipLoop::new(clip, 0.0, 2.0, true),
    )
    .expect("loop");
    run(
        &mut history,
        &mut project,
        &mut ids,
        SetClipMarkers::new(clip, 0.0, 2.0),
    )
    .expect("markers");
    assert_eq!(current(&project), ("Verse".to_string(), 2.0, 2.0));

    for _ in 0..3 {
        assert!(history.undo(&mut project));
    }
    assert_eq!(current(&project), before);
}

#[test]
fn a_loop_or_markers_must_end_after_they_start() {
    let mut project = Project::new_default("Range");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let clip = clip_id(&project, 1, 0).expect("clip");
    let loop_error = run(
        &mut history,
        &mut project,
        &mut ids,
        SetClipLoop::new(clip, 2.0, 2.0, true),
    )
    .err();
    assert!(matches!(
        loop_error,
        Some(CommandError::InvalidRange {
            what: "clip loop",
            ..
        })
    ));
    let marker_error = run(
        &mut history,
        &mut project,
        &mut ids,
        SetClipMarkers::new(clip, -1.0, 2.0),
    )
    .err();
    assert!(matches!(
        marker_error,
        Some(CommandError::InvalidRange {
            what: "clip markers",
            ..
        })
    ));
}

#[test]
fn a_missing_clip_is_an_error() {
    let mut project = Project::new_default("Missing clip");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let error = run(
        &mut history,
        &mut project,
        &mut ids,
        DeleteClip::new(ClipId(4242)),
    )
    .err();
    assert_eq!(error, Some(CommandError::NoSuchClip(ClipId(4242))));
    assert!(!history.can_undo());
}

#[test]
fn notes_are_added_edited_and_removed_with_undo() {
    let mut project = Project::new_default("Notes");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let clip = clip_id(&project, 1, 0).expect("clip");

    run(
        &mut history,
        &mut project,
        &mut ids,
        AddNote::new(clip, MidiNote::new(60, 0.0, 1.0, 100)),
    )
    .expect("add note");
    run(
        &mut history,
        &mut project,
        &mut ids,
        EditNote::new(clip, 0, MidiNote::new(64, 0.0, 2.0, 90)),
    )
    .expect("edit note");
    let pitch = |project: &Project| match &project.tracks[0].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip")
        .payload
    {
        ClipPayload::Midi(data) => data.notes[0].pitch,
        ClipPayload::Audio(_) => panic!("expected a MIDI clip"),
    };
    assert_eq!(pitch(&project), 64);

    run(
        &mut history,
        &mut project,
        &mut ids,
        RemoveNote::new(clip, 0),
    )
    .expect("remove note");
    assert_eq!(note_count(&project, clip), Some(0));

    assert!(history.undo(&mut project));
    assert_eq!(note_count(&project, clip), Some(1));
    assert_eq!(pitch(&project), 64);
    assert!(history.undo(&mut project));
    assert_eq!(pitch(&project), 60);
    assert!(history.undo(&mut project));
    assert_eq!(note_count(&project, clip), Some(0));
}

#[test]
fn a_note_outside_the_ranges_of_live_is_refused() {
    let mut project = Project::new_default("Note ranges");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateMidiClip::new(TrackId(1), 0, "Intro", 4.0),
    )
    .expect("create clip");
    let clip = clip_id(&project, 1, 0).expect("clip");
    let mut high_pitch = MidiNote::new(60, 0.0, 1.0, 100);
    high_pitch.pitch = 200;
    let silent = MidiNote::new(60, 0.0, 1.0, 0);
    let mut bad_probability = MidiNote::new(60, 0.0, 1.0, 100);
    bad_probability.probability = 1.5;
    for note in [high_pitch, silent, bad_probability] {
        let error = run(
            &mut history,
            &mut project,
            &mut ids,
            AddNote::new(clip, note),
        )
        .err();
        assert!(
            matches!(error, Some(CommandError::OutOfRange { .. })),
            "expected a range error, got {error:?}"
        );
    }
    assert_eq!(note_count(&project, clip), Some(0));
}

#[test]
fn notes_belong_to_midi_clips_only() {
    let mut project = Project::new_default("Audio clip");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run(
        &mut history,
        &mut project,
        &mut ids,
        CreateAudioClip::new(TrackId(3), 0, "Loop", "loop.wav", 4.0),
    )
    .expect("create audio clip");
    let clip = clip_id(&project, 3, 0).expect("audio clip");
    let error = run(
        &mut history,
        &mut project,
        &mut ids,
        AddNote::new(clip, MidiNote::new(60, 0.0, 1.0, 100)),
    )
    .err();
    assert_eq!(error, Some(CommandError::NotMidiClip(clip)));
    assert!(project.validate().is_empty());
    assert_eq!(
        run(
            &mut history,
            &mut project,
            &mut ids,
            DeleteClip::new(ClipId(9999)),
        )
        .err(),
        Some(CommandError::NoSuchClip(ClipId(9999)))
    );
}
