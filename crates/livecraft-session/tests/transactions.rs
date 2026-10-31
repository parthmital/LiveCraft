//! Transactions and the Undo History (M04 item 8, manual 5.4.2).

use livecraft_model::edit::EditError;
use livecraft_model::ids::{ClipId, TrackId};
use livecraft_model::model::Project;
use livecraft_session::commands::clip::CreateMidiClip;
use livecraft_session::commands::track::{RenameTrack, SetTrackPan, SetTrackVolume};
use livecraft_session::{
    Command, CommandError, HistoryEntry, OPENING_LABEL, Operation, UndoHistory,
};
use std::time::{Duration, Instant};

fn entry(label: &str, undone: bool) -> HistoryEntry {
    HistoryEntry {
        label: label.to_string(),
        undone,
    }
}

fn run_at<O: Operation>(
    history: &mut UndoHistory,
    project: &mut Project,
    ids: &mut livecraft_model::ids::IdAllocator,
    operation: O,
    at: Instant,
) -> Result<(), CommandError> {
    let command: Box<dyn Command> = Box::new(operation);
    history.execute(command, project, ids, at)
}

fn clip_in(project: &Project, track: u64, slot: usize) -> Option<ClipId> {
    project
        .track(TrackId(track))
        .and_then(|track| track.clip_slots.get(slot))
        .and_then(|slot| slot.clip.as_ref())
        .map(|clip| clip.id)
}

#[test]
fn a_transaction_is_one_labelled_step_that_undo_reverts_together() {
    let mut project = Project::new_default("Transaction");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    history
        .transaction(
            "Duplicate Clips",
            &mut project,
            &mut ids,
            Instant::now(),
            |tx| {
                tx.run(CreateMidiClip::new(TrackId(1), 0, "A", 4.0))?;
                tx.run(CreateMidiClip::new(TrackId(1), 1, "B", 4.0))?;
                Ok(())
            },
        )
        .expect("transaction");
    assert!(clip_in(&project, 1, 0).is_some() && clip_in(&project, 1, 1).is_some());
    assert_eq!(
        history.entries(),
        [entry("Duplicate Clips", false), entry(OPENING_LABEL, false)]
    );

    assert!(history.undo(&mut project));
    assert_eq!(clip_in(&project, 1, 0), None);
    assert_eq!(clip_in(&project, 1, 1), None);
    assert!(!history.can_undo(), "the transaction was one step");
    assert!(history.redo(&mut project, &mut ids).expect("redo"));
    assert!(clip_in(&project, 1, 0).is_some() && clip_in(&project, 1, 1).is_some());
}

#[test]
fn a_failing_transaction_undoes_what_it_applied_and_records_nothing() {
    let mut project = Project::new_default("Failing");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let result = history.transaction("Broken", &mut project, &mut ids, Instant::now(), |tx| {
        tx.run(CreateMidiClip::new(TrackId(1), 0, "A", 4.0))?;
        tx.run(SetTrackVolume::new(TrackId(99), -3.0))?;
        Ok(())
    });
    assert_eq!(
        result,
        Err(CommandError::Edit(EditError::NoSuchTrack(TrackId(99))))
    );
    assert_eq!(clip_in(&project, 1, 0), None, "the clip was rolled back");
    assert_eq!(history.entries(), [entry(OPENING_LABEL, false)]);
    assert!(project.validate().is_empty());
}

#[test]
fn an_empty_transaction_records_nothing() {
    let mut project = Project::new_default("Empty");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    history
        .transaction(
            "Nothing",
            &mut project,
            &mut ids,
            Instant::now(),
            |_| Ok(()),
        )
        .expect("empty transaction");
    assert_eq!(history.entries(), [entry(OPENING_LABEL, false)]);
}

/// Three steps on three different targets, one second apart, so none merges into another.
fn three_steps(project: &mut Project, ids: &mut livecraft_model::ids::IdAllocator) -> UndoHistory {
    let mut history = UndoHistory::default();
    let t0 = Instant::now();
    run_at(
        &mut history,
        project,
        ids,
        SetTrackVolume::new(TrackId(1), -1.0),
        t0,
    )
    .expect("volume");
    run_at(
        &mut history,
        project,
        ids,
        SetTrackPan::new(TrackId(2), 0.5),
        t0 + Duration::from_secs(1),
    )
    .expect("pan");
    run_at(
        &mut history,
        project,
        ids,
        RenameTrack::new(TrackId(3), "Renamed".to_string()),
        t0 + Duration::from_secs(2),
    )
    .expect("rename");
    history
}

#[test]
fn entries_are_listed_newest_first_and_undo_greys_out_the_newest() {
    let mut project = Project::new_default("Entries");
    let mut ids = project.id_allocator();
    let mut history = three_steps(&mut project, &mut ids);
    assert_eq!(
        history.entries(),
        [
            entry("Rename Track", false),
            entry("Set Track Pan", false),
            entry("Set Track Volume", false),
            entry(OPENING_LABEL, false),
        ]
    );
    assert!(history.undo(&mut project));
    assert_eq!(
        history.entries(),
        [
            entry("Rename Track", true),
            entry("Set Track Pan", false),
            entry("Set Track Volume", false),
            entry(OPENING_LABEL, false),
        ]
    );
}

#[test]
fn go_to_reverts_to_an_older_entry_and_reapplies_to_a_greyed_one() {
    let mut project = Project::new_default("Go to");
    let mut ids = project.id_allocator();
    let mut history = three_steps(&mut project, &mut ids);
    let volume = |project: &Project| project.track(TrackId(1)).map(|t| t.volume_db);
    let pan = |project: &Project| project.track(TrackId(2)).map(|t| t.pan);
    let name = |project: &Project| project.track(TrackId(3)).map(|t| t.name.clone());

    // Entry 2 is "Set Track Volume": only the first step stays applied.
    assert!(history.go_to(2, &mut project, &mut ids).expect("revert"));
    assert_eq!(volume(&project), Some(-1.0));
    assert_eq!(pan(&project), Some(0.0));
    assert_eq!(name(&project).as_deref(), Some("3-Audio"));
    assert_eq!(
        history.entries(),
        [
            entry("Rename Track", true),
            entry("Set Track Pan", true),
            entry("Set Track Volume", false),
            entry(OPENING_LABEL, false),
        ]
    );

    // Entry 0 is the newest step: selecting it reapplies every undone step.
    assert!(
        history
            .go_to(0, &mut project, &mut ids)
            .expect("reapply all")
    );
    assert_eq!(pan(&project), Some(0.5));
    assert_eq!(name(&project).as_deref(), Some("Renamed"));

    // The opening of the Set is the last entry: every step is undone.
    assert!(history.go_to(3, &mut project, &mut ids).expect("open"));
    assert_eq!(volume(&project), Some(0.0));
    assert!(!history.can_undo());

    assert!(
        !history
            .go_to(9, &mut project, &mut ids)
            .expect("no such entry")
    );
}
