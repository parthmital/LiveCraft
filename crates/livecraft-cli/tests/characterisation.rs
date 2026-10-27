use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project};
use livecraft_testkit::render_project_summary;

#[test]
fn test_characterisation_render_metrics() {
    let project = Project::new_default("demo");
    let summary = render_project_summary(&project, 4, 44100);
    assert_eq!(summary.frames, 352800);
    assert!((summary.peak_l - 0.0).abs() < 1e-6);
    assert!((summary.peak_r - 0.0).abs() < 1e-6);
    assert!((summary.rms_l - 0.0).abs() < 1e-6);
    assert!((summary.rms_r - 0.0).abs() < 1e-6);

    let mut chords_project = Project::new_default("ChordsDemo");
    let mut clip = Clip::new_midi(1, "Lead Chords", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        mdata.notes.push(MidiNote::new(64, 0.0, 2.0, 100));
        mdata.notes.push(MidiNote::new(67, 0.0, 2.0, 100));
    }
    chords_project.tracks[0].clip_slots[0].clip = Some(clip);
    let summary2 = render_project_summary(&chords_project, 4, 44100);
    assert_eq!(summary2.frames, 352800);
    assert!((summary2.peak_l - 0.26114187).abs() < 1e-6);
    assert!((summary2.peak_r - 0.26114187).abs() < 1e-6);
    assert!((summary2.rms_l - 0.06759972).abs() < 1e-6);
    assert!((summary2.rms_r - 0.06759972).abs() < 1e-6);
}

#[test]
fn test_characterisation_inspect_json() {
    let project = Project::new_default("demo");
    let val: serde_json::Value = serde_json::to_value(&project).expect("serialize to value");
    let json = serde_json::to_string_pretty(&val).expect("pretty print sorted json");
    insta::assert_snapshot!("demo_project_inspect", json);
}
