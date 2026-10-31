//! Each rule of `Project::validate` reports the broken reference or structure it names.

use livecraft_model::ids::{ClipId, TrackId};
use livecraft_model::model::*;
use livecraft_model::routing::AudioOutput;

fn group(id: u64, parent: Option<u64>) -> Track {
    let mut track = Track::new(TrackId(id), "Group", TrackType::Group, 0);
    track.parent = parent.map(TrackId);
    track
}

#[test]
fn the_default_set_is_valid() {
    assert!(Project::new_default("Valid").validate().is_empty());
}

#[test]
fn a_group_track_that_holds_a_clip_is_reported() {
    let mut proj = Project::new_default("Clips");
    let mut group = group(20, None);
    group.clip_slots.push(ClipSlot::with_clip(
        9000,
        Clip::new_midi(ClipId(30), "Clip", 4.0),
    ));
    proj.tracks.push(group);
    assert!(
        proj.validate()
            .contains(&ValidationError::GroupHasClips(TrackId(20)))
    );
}

#[test]
fn a_parent_that_is_not_a_group_is_reported() {
    let mut proj = Project::new_default("Parent");
    proj.tracks[1].parent = Some(TrackId(1));
    assert!(proj.validate().contains(&ValidationError::ParentNotGroup {
        track: TrackId(2),
        parent: TrackId(1),
    }));
}

#[test]
fn a_missing_parent_is_reported() {
    let mut proj = Project::new_default("Missing");
    proj.tracks[1].parent = Some(TrackId(99));
    assert!(proj.validate().contains(&ValidationError::MissingParent {
        track: TrackId(2),
        parent: TrackId(99),
    }));
}

#[test]
fn a_cycle_of_group_parents_is_reported_for_each_track_in_it() {
    let mut proj = Project::new_default("Cycle");
    let outer = group(20, None);
    let inner = group(21, Some(20));
    proj.tracks.push(outer);
    proj.tracks.push(inner);
    proj.tracks[4].parent = Some(TrackId(21));
    let errors = proj.validate();
    assert!(errors.contains(&ValidationError::ParentCycle(TrackId(20))));
    assert!(errors.contains(&ValidationError::ParentCycle(TrackId(21))));
}

#[test]
fn a_track_with_the_wrong_number_of_sends_is_reported() {
    let mut proj = Project::new_default("Sends");
    proj.tracks[0].sends.push(0.0);
    assert!(proj.validate().contains(&ValidationError::SendCount {
        track: TrackId(1),
        sends: 3,
        returns: 2,
    }));
}

#[test]
fn a_duplicate_track_id_is_reported() {
    let mut proj = Project::new_default("Duplicate");
    proj.tracks
        .push(Track::new(TrackId(1), "Copy", TrackType::Audio, 0));
    assert!(
        proj.validate()
            .contains(&ValidationError::DuplicateTrack(TrackId(1)))
    );
}

#[test]
fn a_route_to_a_missing_track_is_reported() {
    let mut proj = Project::new_default("Route");
    proj.tracks[0].routing.audio_to = AudioOutput::Track(TrackId(99));
    assert!(
        proj.validate()
            .contains(&ValidationError::MissingRouteTarget {
                track: TrackId(1),
                target: TrackId(99),
            })
    );
}
