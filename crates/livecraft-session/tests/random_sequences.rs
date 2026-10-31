//! Random command sequences, then undo all, give back the Set they started from (M04 acceptance).
//!
//! Each case applies up to 30 generated commands. Some are invalid on purpose (missing targets,
//! out-of-range values), and an invalid command must change nothing. After every step the Set must
//! pass `validate`, and after undoing every recorded step the Set must equal the start.

use livecraft_model::automation::{AutomationLane, AutomationPoint, AutomationTarget};
use livecraft_model::device::DeviceType;
use livecraft_model::ids::{ClipId, DeviceId, IdAllocator, LaneId, SceneId, TrackId};
use livecraft_model::model::{MidiNote, Project};
use livecraft_session::commands::automation::AddAutomationPoint;
use livecraft_session::commands::clip::{CreateMidiClip, DeleteClip, MoveClip};
use livecraft_session::commands::device::{InsertDevice, MoveDevice, RemoveDevice, SetDeviceParam};
use livecraft_session::commands::note::AddNote;
use livecraft_session::commands::scene::{CreateScene, DeleteScene, DuplicateScene};
use livecraft_session::commands::track::{
    CreateAudioTrack, CreateGroupTrack, CreateReturnTrack, DeleteTrack, RenameTrack, ReorderTrack,
    SetTrackArmed, SetTrackPan, SetTrackSend, SetTrackVolume,
};
use livecraft_session::commands::transport::{SetLoop, SetMetronome, SetTempo, SetTimeSignature};
use livecraft_session::{Command, UndoHistory};
use proptest::prelude::*;
use std::time::{Duration, Instant};

/// Generated command kinds. A kind is `kind % KINDS`, so every value of `kind` picks one.
const KINDS: u8 = 26;

/// The track a pick names, or the first track when the pick is past the end of the list.
fn pick_track(project: &Project, pick: u8) -> TrackId {
    let tracks: Vec<TrackId> = project.all_tracks().map(|track| track.id).collect();
    tracks
        .get(usize::from(pick) % tracks.len().max(1))
        .copied()
        .unwrap_or(TrackId(0))
}

fn pick_return(project: &Project, pick: u8) -> TrackId {
    let returns: Vec<TrackId> = project.return_tracks.iter().map(|track| track.id).collect();
    returns
        .get(usize::from(pick) % returns.len().max(1))
        .copied()
        .unwrap_or(TrackId(0))
}

fn pick_clip(project: &Project, pick: u8) -> ClipId {
    let clips: Vec<ClipId> = project
        .all_tracks()
        .flat_map(|track| track.clip_slots.iter())
        .filter_map(|slot| slot.clip.as_ref().map(|clip| clip.id))
        .collect();
    clips
        .get(usize::from(pick) % clips.len().max(1))
        .copied()
        .unwrap_or(ClipId(0))
}

fn pick_device(project: &Project, track: TrackId, pick: u8) -> DeviceId {
    let devices: Vec<DeviceId> = project
        .track(track)
        .map(|track| track.device_chain.iter().map(|device| device.id).collect())
        .unwrap_or_default();
    devices
        .get(usize::from(pick) % devices.len().max(1))
        .copied()
        .unwrap_or(DeviceId(0))
}

fn pick_scene(project: &Project, pick: u8) -> SceneId {
    let scenes: Vec<SceneId> = project.scenes.iter().map(|scene| scene.id).collect();
    scenes
        .get(usize::from(pick) % scenes.len().max(1))
        .copied()
        .unwrap_or(SceneId(0))
}

/// The command a generated step describes, against the Set as it is now.
fn build(project: &Project, lane: LaneId, kind: u8, a: u8, b: u8, value: f32) -> Box<dyn Command> {
    let track = pick_track(project, a);
    match kind % KINDS {
        0 => Box::new(SetTrackVolume::new(track, value)),
        1 => Box::new(SetTrackPan::new(track, (value / 40.0).clamp(-1.0, 1.0))),
        2 => Box::new(SetTrackArmed::new(track, b.is_multiple_of(2))),
        3 => Box::new(RenameTrack::new(track, format!("T{b}"))),
        4 => Box::new(CreateAudioTrack::new(format!("A{a}"))),
        5 => Box::new(CreateReturnTrack::new(format!("R{a}"))),
        6 => Box::new(CreateGroupTrack::new(format!("G{a}"))),
        7 => Box::new(DeleteTrack::new(track)),
        8 => Box::new(ReorderTrack::new(track, usize::from(b) % 8)),
        9 => Box::new(SetTrackSend::new(
            track,
            pick_return(project, b),
            (value.abs() / 40.0).min(1.0),
        )),
        10 => Box::new(CreateMidiClip::new(
            track,
            usize::from(a) % 8,
            "C",
            1.0 + f64::from(b % 8),
        )),
        11 => Box::new(DeleteClip::new(pick_clip(project, a))),
        12 => Box::new(MoveClip::new(
            pick_clip(project, a),
            track,
            usize::from(b) % 8,
        )),
        13 => Box::new(AddNote::new(
            pick_clip(project, a),
            MidiNote::new(b % 128, f64::from(a % 4), 1.0, 1 + (a % 127)),
        )),
        14 => Box::new(InsertDevice::new(track, DeviceType::Compressor, "Comp")),
        15 => Box::new(SetDeviceParam::new(
            track,
            pick_device(project, track, a),
            "band_1_gain",
            value,
        )),
        16 => Box::new(RemoveDevice::new(track, pick_device(project, track, a))),
        17 => Box::new(MoveDevice::new(
            track,
            pick_device(project, track, a),
            usize::from(b) % 4,
        )),
        18 => Box::new(SetTempo::new(20.0 + (value.abs() * 30.0) % 900.0)),
        19 => Box::new(AddAutomationPoint::new(
            TrackId(1),
            lane,
            AutomationPoint::new(f64::from(a % 16), value, 0.0),
        )),
        20 => Box::new(DuplicateScene::new(pick_scene(project, a))),
        21 => Box::new(DeleteScene::new(pick_scene(project, a))),
        22 => Box::new(CreateScene::new(format!("S{a}"))),
        23 => Box::new(SetTimeSignature::new(
            a % 7 + 1,
            if b.is_multiple_of(2) { 4 } else { 8 },
        )),
        24 => Box::new(SetLoop::new(
            b.is_multiple_of(2),
            f64::from(a % 8),
            1.0 + f64::from(b % 8),
        )),
        _ => Box::new(SetMetronome::new(b.is_multiple_of(2))),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn random_command_sequences_undo_back_to_the_start(
        steps in prop::collection::vec((any::<u8>(), any::<u8>(), any::<u8>(), -40.0f32..6.0), 1..30),
    ) {
        let mut project = Project::new_default("Random");
        let mut ids: IdAllocator = project.id_allocator();
        let lane = ids.lane();
        project.tracks[0]
            .automation_lanes
            .push(AutomationLane::new(lane, AutomationTarget::TrackVolume));
        let start = format!("{project:?}");

        let mut history = UndoHistory::default();
        let t0 = Instant::now();
        for (index, (kind, a, b, value)) in steps.into_iter().enumerate() {
            let command = build(&project, lane, kind, a, b, value);
            // 200 ms apart, so neighbouring gestures can merge, as they do in use.
            let at = t0 + Duration::from_millis(index as u64 * 200);
            let _ = history.execute(command, &mut project, &mut ids, at);
            prop_assert!(project.validate().is_empty(), "invalid after step {index}: {:?}", project.validate());
        }

        while history.undo(&mut project) {}
        prop_assert_eq!(format!("{project:?}"), start);
    }
}
