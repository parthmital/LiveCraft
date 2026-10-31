//! Edit operations keep the Set valid: rejected edits change nothing, and random sequences of
//! accepted and rejected edits never break a rule of `Project::validate` (10,000 cases).

use livecraft_model::edit::EditError;
use livecraft_model::ids::{IdAllocator, TrackId};
use livecraft_model::model::*;
use livecraft_model::routing::{AudioInput, AudioOutput, Routing};
use proptest::prelude::*;

fn group(id: u64) -> Track {
    Track::new(TrackId(id), "Group", TrackType::Group, 0)
}

#[test]
fn set_parent_refuses_a_group_inside_its_own_child() {
    let mut proj = Project::new_default("Nest");
    proj.tracks.push(group(20));
    proj.tracks.push(group(21));
    proj.set_parent(TrackId(21), Some(TrackId(20)))
        .expect("nest group 21 in group 20");
    assert_eq!(
        proj.set_parent(TrackId(20), Some(TrackId(21))),
        Err(EditError::Cycle(TrackId(20)))
    );
    assert!(proj.validate().is_empty());
}

#[test]
fn removing_a_group_moves_its_children_up_a_level() {
    let mut proj = Project::new_default("Ungroup");
    proj.tracks.push(group(20));
    proj.tracks.push(group(21));
    proj.set_parent(TrackId(21), Some(TrackId(20)))
        .expect("nest");
    proj.set_parent(TrackId(20), None).expect("top level");
    proj.remove_track(TrackId(20))
        .expect("remove the outer group");
    assert_eq!(proj.track(TrackId(21)).and_then(|t| t.parent), None);
    assert!(proj.validate().is_empty());
}

#[test]
fn removing_a_return_track_takes_its_send_from_every_track() {
    let mut proj = Project::new_default("Returns");
    proj.remove_track(TrackId(5)).expect("remove return A");
    assert!(proj.all_tracks().all(|track| track.sends.len() == 1));
    assert!(proj.validate().is_empty());
}

#[test]
fn the_main_track_cannot_be_removed() {
    let mut proj = Project::new_default("Main");
    assert_eq!(
        proj.remove_track(TrackId(7)),
        Err(EditError::MainTrackFixed)
    );
}

#[test]
fn a_duplicate_id_is_refused() {
    let mut proj = Project::new_default("Duplicate");
    assert_eq!(
        proj.add_track(Track::new(TrackId(1), "Copy", TrackType::Audio, 0)),
        Err(EditError::IdInUse(1))
    );
}

#[test]
fn group_tracks_hold_no_clips() {
    let mut proj = Project::new_default("Clips");
    proj.add_track(group(20)).expect("add group");
    let clip = Clip::new_midi(livecraft_model::ids::ClipId(900), "Clip", 4.0);
    assert_eq!(
        proj.set_clip(TrackId(20), 0, clip),
        Err(EditError::GroupHoldsClips)
    );
}

#[test]
fn a_route_to_the_track_itself_is_refused() {
    let mut proj = Project::new_default("Route");
    let routing = Routing {
        audio_from: AudioInput::None,
        audio_to: AudioOutput::Track(TrackId(1)),
    };
    assert_eq!(
        proj.set_route(TrackId(1), routing),
        Err(EditError::NoSuchRouteTarget(TrackId(1)))
    );
}

/// One random edit. `kind` picks the operation and `pick` picks its targets. Rejected edits are
/// expected, and must leave the Set valid too, so their errors are ignored here.
fn random_edit(project: &mut Project, ids: &mut IdAllocator, kind: u8, pick: usize) {
    let tracks: Vec<TrackId> = project.all_tracks().map(|track| track.id).collect();
    let target = tracks[pick % tracks.len()];
    let other = tracks[(pick / 3 + 1) % tracks.len()];
    match kind % 9 {
        0 => {
            let track_type = [TrackType::Audio, TrackType::Midi, TrackType::Group][pick % 3];
            let mut track = Track::new(ids.track(), "Track", track_type, 4);
            track.parent = if pick.is_multiple_of(2) {
                None
            } else {
                Some(target)
            };
            let _ = project.add_track(track);
        }
        1 => {
            let _ = project.remove_track(target);
        }
        2 => {
            let parent = if pick.is_multiple_of(3) {
                None
            } else {
                Some(other)
            };
            let _ = project.set_parent(target, parent);
        }
        3 => {
            let _ = project.add_scene(Scene::new(ids.scene(), "Scene"));
        }
        4 => {
            let scene = project
                .scenes
                .get(pick % project.scenes.len().max(1))
                .map(|scene| scene.id);
            if let Some(scene) = scene {
                let _ = project.remove_scene(scene);
            }
        }
        5 => {
            let device = Device::new(ids.device(), "Device", DeviceType::Delay);
            let _ = project.add_device(target, device);
        }
        6 => {
            let first = project
                .track(target)
                .and_then(|track| track.device_chain.first())
                .map(|device| device.id);
            if let Some(device) = first {
                let _ = project.remove_device(target, device);
            }
        }
        7 => {
            let slots = project
                .track(target)
                .map_or(0, |track| track.clip_slots.len());
            if slots > 0 {
                let clip = Clip::new_midi(ids.clip(), "Clip", 4.0);
                let _ = project.set_clip(target, pick % slots, clip);
            }
        }
        _ => {
            let audio_to = if pick.is_multiple_of(2) {
                AudioOutput::Main
            } else {
                AudioOutput::Track(other)
            };
            let routing = Routing {
                audio_from: AudioInput::None,
                audio_to,
            };
            let _ = project.set_route(target, routing);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn random_edit_sequences_keep_the_set_valid(
        steps in prop::collection::vec((any::<u8>(), any::<usize>()), 1..30),
    ) {
        let mut project = Project::new_default("Random");
        let mut ids = project.id_allocator();
        for (kind, pick) in steps {
            random_edit(&mut project, &mut ids, kind, pick);
            prop_assert_eq!(project.validate(), Vec::new());
        }
    }
}
