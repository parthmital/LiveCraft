//! Scene and transport commands (M04 item 7).

use livecraft_model::edit::EditError;
use livecraft_model::ids::SceneId;
use livecraft_model::model::Project;
use livecraft_session::commands::scene::{CreateScene, DeleteScene, DuplicateScene};
use livecraft_session::commands::transport::{SetLoop, SetMetronome, SetTempo, SetTimeSignature};
use livecraft_session::{Command, CommandError, Operation, UndoHistory};
use std::time::{Duration, Instant};

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

fn scene_ids(project: &Project) -> Vec<u64> {
    project.scenes.iter().map(|scene| scene.id.0).collect()
}

#[test]
fn a_created_scene_is_appended_and_undo_removes_it() {
    let mut project = Project::new_default("Scenes");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        CreateScene::new("9"),
        Instant::now(),
    )
    .expect("create scene");
    let created = project
        .scenes
        .last()
        .map(|scene| scene.id)
        .expect("new scene");
    assert_eq!(project.scenes.len(), 9);
    assert!(history.undo(&mut project));
    assert_eq!(project.scenes.len(), 8);
    assert!(history.redo(&mut project, &mut ids).expect("redo"));
    assert_eq!(project.scenes.last().map(|scene| scene.id), Some(created));
    assert!(project.validate().is_empty());
}

#[test]
fn a_deleted_scene_returns_to_its_row_on_undo() {
    let mut project = Project::new_default("Delete scene");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let before = scene_ids(&project);
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        DeleteScene::new(SceneId(3)),
        Instant::now(),
    )
    .expect("delete scene 3");
    assert_eq!(project.scenes.len(), 7);
    assert!(history.undo(&mut project));
    assert_eq!(scene_ids(&project), before);
}

#[test]
fn a_duplicated_scene_sits_after_its_source_with_a_new_id() {
    let mut project = Project::new_default("Duplicate scene");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        DuplicateScene::new(SceneId(2)),
        Instant::now(),
    )
    .expect("duplicate scene 2");
    let copy = project.scenes[2].clone();
    assert_ne!(copy.id, SceneId(2));
    assert_eq!(copy.name, project.scenes[1].name);
    assert!(project.validate().is_empty());

    assert!(history.undo(&mut project));
    assert_eq!(project.scenes.len(), 8);
    assert!(history.redo(&mut project, &mut ids).expect("redo"));
    assert_eq!(project.scenes[2].id, copy.id);
}

#[test]
fn a_missing_scene_is_an_error() {
    let mut project = Project::new_default("Missing scene");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let error = run_at(
        &mut history,
        &mut project,
        &mut ids,
        DeleteScene::new(SceneId(99)),
        Instant::now(),
    )
    .err();
    assert_eq!(
        error,
        Some(CommandError::Edit(EditError::NoSuchScene(SceneId(99))))
    );
}

#[test]
fn the_time_signature_is_set_and_undone() {
    let mut project = Project::new_default("Meter");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    assert_eq!(project.transport.time_signature(), (4, 4));
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTimeSignature::new(7, 8),
        Instant::now(),
    )
    .expect("set 7/8");
    assert_eq!(project.transport.time_signature(), (7, 8));
    assert!(history.undo(&mut project));
    assert_eq!(project.transport.time_signature(), (4, 4));
}

#[test]
fn an_impossible_time_signature_is_refused() {
    let mut project = Project::new_default("Bad meter");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let error = run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetTimeSignature::new(0, 4),
        Instant::now(),
    )
    .err();
    assert!(
        matches!(error, Some(CommandError::Time(_))),
        "got {error:?}"
    );
    assert_eq!(project.transport.time_signature(), (4, 4));
    assert!(!history.can_undo());
}

#[test]
fn the_loop_and_the_metronome_are_set_and_undone() {
    let mut project = Project::new_default("Loop");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let before = (
        project.transport.loop_enabled,
        project.transport.loop_start_beats,
        project.transport.loop_length_beats,
        project.transport.metronome_enabled,
    );
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetLoop::new(true, 4.0, 8.0),
        Instant::now(),
    )
    .expect("loop");
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetMetronome::new(true),
        Instant::now(),
    )
    .expect("metronome");
    assert!(project.transport.loop_enabled);
    assert_eq!(project.transport.loop_start_beats, 4.0);
    assert_eq!(project.transport.loop_length_beats, 8.0);
    assert!(project.transport.metronome_enabled);

    assert!(history.undo(&mut project));
    assert!(!project.transport.metronome_enabled);
    assert!(history.undo(&mut project));
    assert_eq!(
        (
            project.transport.loop_enabled,
            project.transport.loop_start_beats,
            project.transport.loop_length_beats,
            project.transport.metronome_enabled,
        ),
        before
    );
}

#[test]
fn a_loop_must_start_at_zero_or_later_and_have_a_length() {
    let mut project = Project::new_default("Loop range");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let negative = run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetLoop::new(true, -1.0, 4.0),
        Instant::now(),
    )
    .err();
    assert!(matches!(
        negative,
        Some(CommandError::InvalidRange { what: "loop", .. })
    ));
    let empty = run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetLoop::new(true, 0.0, 0.0),
        Instant::now(),
    )
    .err();
    assert!(matches!(
        empty,
        Some(CommandError::NotPositive {
            what: "loop length",
            ..
        })
    ));
    assert!(!history.can_undo());
}

#[test]
fn a_tempo_drag_is_one_undo_step() {
    let mut project = Project::new_default("Tempo drag");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let t0 = Instant::now();
    for (offset_ms, bpm) in [(0, 121.0), (100, 122.0), (200, 128.0)] {
        run_at(
            &mut history,
            &mut project,
            &mut ids,
            SetTempo::new(bpm),
            t0 + Duration::from_millis(offset_ms),
        )
        .expect("drag");
    }
    assert_eq!(project.transport.bpm(), 128.0);
    assert!(history.undo(&mut project));
    assert!(!history.can_undo());
    assert_eq!(project.transport.bpm(), 120.0);
}
