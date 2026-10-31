//! Device and parameter commands (M04 item 6).

use livecraft_model::automation::{AutomationLane, AutomationTarget};
use livecraft_model::device::DeviceType;
use livecraft_model::edit::EditError;
use livecraft_model::ids::{DeviceId, IdAllocator, TrackId};
use livecraft_model::model::Project;
use livecraft_session::commands::device::{
    InsertDevice, MoveDevice, RemoveDevice, SetDeviceEnabled, SetDeviceParam,
};
use livecraft_session::{Command, CommandError, Operation, UndoHistory};
use std::time::{Duration, Instant};

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

/// The ids of a track's devices, in chain order.
fn chain(project: &Project, track: u64) -> Vec<u64> {
    project
        .track(TrackId(track))
        .map(|track| {
            track
                .device_chain
                .iter()
                .map(|device| device.id.0)
                .collect()
        })
        .unwrap_or_default()
}

fn param(project: &Project, track: u64, device: u64, name: &str) -> Option<f32> {
    project
        .track(TrackId(track))
        .and_then(|track| {
            track
                .device_chain
                .iter()
                .find(|known| known.id == DeviceId(device))
        })
        .and_then(|device| device.param(name))
}

#[test]
fn an_inserted_device_keeps_its_id_through_undo_and_redo() {
    let mut project = Project::new_default("Insert");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let now = Instant::now();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        InsertDevice::new(TrackId(3), DeviceType::Compressor, "Compressor"),
        now,
    )
    .expect("insert");
    let created = *chain(&project, 3).last().expect("new device");
    assert_eq!(chain(&project, 3).len(), 2);
    assert!(project.validate().is_empty());

    assert!(history.undo(&mut project));
    assert_eq!(chain(&project, 3), [102]);
    assert!(history.redo(&mut project, &mut ids).expect("redo"));
    assert_eq!(chain(&project, 3).last(), Some(&created));
    assert!(project.validate().is_empty());
}

#[test]
fn removing_a_device_restores_it_and_its_automation_on_undo() {
    let mut project = Project::new_default("Remove");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let lane = ids.lane();
    project.tracks[0].automation_lanes.push(AutomationLane::new(
        lane,
        AutomationTarget::DeviceParam {
            device_id: DeviceId(101),
            param_key: "osc1_level".to_string(),
        },
    ));

    run_at(
        &mut history,
        &mut project,
        &mut ids,
        RemoveDevice::new(TrackId(1), DeviceId(101)),
        Instant::now(),
    )
    .expect("remove operator");
    assert_eq!(chain(&project, 1), Vec::<u64>::new());
    assert!(
        project
            .track(TrackId(1))
            .is_some_and(|t| t.automation_lanes.is_empty())
    );
    assert!(project.validate().is_empty());

    assert!(history.undo(&mut project));
    assert_eq!(chain(&project, 1), [101]);
    assert_eq!(param(&project, 1, 101, "osc1_level"), Some(1.0));
    let restored = project.track(TrackId(1)).expect("track 1").automation_lanes[0].id;
    assert_eq!(restored, lane);
    assert!(project.validate().is_empty());
}

#[test]
fn moving_a_device_and_undo_restore_the_chain_order() {
    let mut project = Project::new_default("Move");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let now = Instant::now();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        InsertDevice::new(TrackId(3), DeviceType::Compressor, "Compressor"),
        now,
    )
    .expect("insert");
    let compressor = DeviceId(*chain(&project, 3).last().expect("compressor"));
    assert_eq!(chain(&project, 3), [102, compressor.0]);

    run_at(
        &mut history,
        &mut project,
        &mut ids,
        MoveDevice::new(TrackId(3), compressor, 0),
        now,
    )
    .expect("move compressor to the front");
    assert_eq!(chain(&project, 3), [compressor.0, 102]);
    assert!(history.undo(&mut project));
    assert_eq!(chain(&project, 3), [102, compressor.0]);
}

#[test]
fn a_device_can_be_switched_off_and_on_again() {
    let mut project = Project::new_default("Enable");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let enabled = |project: &Project| {
        project
            .track(TrackId(1))
            .and_then(|track| track.device_chain.first())
            .map(|device| device.is_enabled)
    };
    assert_eq!(enabled(&project), Some(true));
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetDeviceEnabled::new(TrackId(1), DeviceId(101), false),
        Instant::now(),
    )
    .expect("switch off");
    assert_eq!(enabled(&project), Some(false));
    assert!(history.undo(&mut project));
    assert_eq!(enabled(&project), Some(true));
}

#[test]
fn a_parameter_is_set_by_name_and_undo_restores_the_default() {
    let mut project = Project::new_default("Parameter");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetDeviceParam::new(TrackId(3), DeviceId(102), "band_1_gain", 3.0),
        Instant::now(),
    )
    .expect("set gain");
    assert_eq!(param(&project, 3, 102, "band_1_gain"), Some(3.0));
    assert!(history.undo(&mut project));
    assert_eq!(param(&project, 3, 102, "band_1_gain"), Some(0.0));
}

#[test]
fn a_parameter_without_a_value_is_unset_again_by_undo() {
    let mut project = Project::new_default("Unset");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    assert_eq!(param(&project, 1, 101, "algorithm"), None);
    run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetDeviceParam::new(TrackId(1), DeviceId(101), "algorithm", 2.0),
        Instant::now(),
    )
    .expect("set algorithm");
    assert_eq!(param(&project, 1, 101, "algorithm"), Some(2.0));
    assert!(history.undo(&mut project));
    assert_eq!(param(&project, 1, 101, "algorithm"), None);
}

#[test]
fn a_parameter_name_outside_the_device_table_is_an_error() {
    let mut project = Project::new_default("Unknown parameter");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let error = run_at(
        &mut history,
        &mut project,
        &mut ids,
        SetDeviceParam::new(TrackId(3), DeviceId(102), "no_such_param", 1.0),
        Instant::now(),
    )
    .err();
    assert_eq!(
        error,
        Some(CommandError::NoSuchParam {
            device: DeviceId(102),
            name: "no_such_param".to_string(),
        })
    );
    assert!(!history.can_undo());
}

#[test]
fn a_knob_drag_is_one_undo_step() {
    let mut project = Project::new_default("Knob");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let t0 = Instant::now();
    for (offset_ms, value) in [(0, 1.0), (100, 2.0), (200, 4.0)] {
        run_at(
            &mut history,
            &mut project,
            &mut ids,
            SetDeviceParam::new(TrackId(3), DeviceId(102), "band_1_gain", value),
            t0 + Duration::from_millis(offset_ms),
        )
        .expect("drag");
    }
    assert_eq!(param(&project, 3, 102, "band_1_gain"), Some(4.0));
    assert!(history.undo(&mut project));
    assert!(!history.can_undo(), "the drag is one step");
    assert_eq!(param(&project, 3, 102, "band_1_gain"), Some(0.0));
}

#[test]
fn a_device_on_a_missing_track_is_an_error() {
    let mut project = Project::new_default("Missing track");
    let mut ids = project.id_allocator();
    let mut history = UndoHistory::default();
    let error = run_at(
        &mut history,
        &mut project,
        &mut ids,
        InsertDevice::new(TrackId(99), DeviceType::Utility, "Utility"),
        Instant::now(),
    )
    .err();
    assert_eq!(
        error,
        Some(CommandError::Edit(EditError::NoSuchTrack(TrackId(99))))
    );
}
