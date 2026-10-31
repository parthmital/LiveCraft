//! The session is how front ends change the Set (M04 item 9): one frame of edits is one undo step,
//! a gesture is one step, and the Set is marked dirty until it is saved.

use livecraft_model::ids::TrackId;
use livecraft_model::model::Project;
use livecraft_session::commands::track::{CreateAudioTrack, SetTrackPan, SetTrackVolume};
use livecraft_session::{Command, Session, demo_set, load_set};
use std::path::PathBuf;

fn volume(session: &Session, track: u64) -> Option<f32> {
    session.project().track(TrackId(track)).map(|t| t.volume_db)
}

#[test]
fn several_edits_from_one_frame_are_one_labelled_undo_step() {
    let mut session = Session::new(Project::new_default("Frame"), None);
    session
        .apply(vec![
            Box::new(SetTrackVolume::new(TrackId(1), -3.0)) as Box<dyn Command>,
            Box::new(SetTrackPan::new(TrackId(2), 0.5)),
        ])
        .expect("apply the frame");
    assert_eq!(volume(&session, 1), Some(-3.0));
    assert_eq!(session.entries()[0].label, "Set Track Volume");
    assert!(session.undo());
    assert_eq!(volume(&session, 1), Some(0.0));
    assert_eq!(
        session.project().track(TrackId(2)).map(|t| t.pan),
        Some(0.0)
    );
    assert!(!session.undo(), "the frame was one step");
}

#[test]
fn a_drag_across_frames_is_one_undo_step() {
    let mut session = Session::new(Project::new_default("Drag"), None);
    for volume_db in [-1.0, -2.0, -3.0] {
        session
            .apply(vec![
                Box::new(SetTrackVolume::new(TrackId(1), volume_db)) as Box<dyn Command>
            ])
            .expect("drag frame");
    }
    assert_eq!(volume(&session, 1), Some(-3.0));
    assert!(session.undo());
    assert!(!session.undo());
    assert_eq!(volume(&session, 1), Some(0.0));
}

#[test]
fn an_empty_frame_changes_nothing() {
    let mut session = Session::new(Project::new_default("Empty"), None);
    session.apply(Vec::new()).expect("empty frame");
    assert!(!session.is_dirty());
    assert!(!session.undo());
}

#[test]
fn the_set_is_dirty_after_an_edit_until_it_is_saved() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("session_dirty");
    std::fs::create_dir_all(&dir).expect("create the test folder");
    let mut session = Session::new(Project::new_default("Dirty"), None);
    assert!(!session.is_dirty());
    session
        .apply(vec![
            Box::new(CreateAudioTrack::new("5-Audio")) as Box<dyn Command>
        ])
        .expect("create track");
    assert!(session.is_dirty());
    session.save(&dir.join("dirty.als")).expect("save");
    assert!(!session.is_dirty());
}

#[test]
fn a_saved_set_loads_back_with_the_edits_in_it() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("session_roundtrip");
    std::fs::create_dir_all(&dir).expect("create the test folder");
    let path = dir.join("roundtrip.als");
    let mut session = Session::new(Project::new_default("Round trip"), None);
    session
        .apply(vec![
            Box::new(SetTrackVolume::new(TrackId(1), -9.0)) as Box<dyn Command>
        ])
        .expect("set volume");
    session.save(&path).expect("save");
    let loaded = load_set(&path).expect("load");
    assert_eq!(loaded.track(TrackId(1)).map(|t| t.volume_db), Some(-9.0));
}

#[test]
fn the_demo_set_has_its_chord_clip_with_nine_notes() {
    let project = demo_set().expect("demo set");
    let slot = project.track(TrackId(1)).and_then(|t| t.clip_slots.first());
    let clip = slot.and_then(|s| s.clip.as_ref()).expect("chord clip");
    assert_eq!(clip.name, "Intro Chords");
    match &clip.payload {
        livecraft_model::model::ClipPayload::Midi(data) => assert_eq!(data.notes.len(), 9),
        livecraft_model::model::ClipPayload::Audio(_) => panic!("the demo clip is MIDI"),
    }
    assert!(project.validate().is_empty());
}
