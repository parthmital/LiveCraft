//! Lossless `.als` open/edit/undo/redo/save integration and property tests for `Session`.

mod common;

use common::{
    RICH_UNKNOWN_XML, assert_approx_f32, assert_approx_f64, assert_projects_equivalent,
    null_factory, temp_dir,
};
use livecraft_formats::als::{AlsDocument, AlsParser};
use livecraft_model::color::ABLETON_PALETTE;
use livecraft_model::device::{DeviceType, param_table};
use livecraft_model::ids::{ClipId, DeviceId, SceneId, TrackId};
use livecraft_model::model::{ClipPayload, MidiNote, Project, ScaleConfig, ScaleType, TrackType};
use livecraft_session::commands::clip::{
    CreateMidiClip, DeleteClip, MoveClip, RenameClip, SetClipLoop,
};
use livecraft_session::commands::device::{
    InsertDevice, MoveDevice, RemoveDevice, SetDeviceEnabled, SetDeviceParam,
};
use livecraft_session::commands::note::{AddNote, RemoveNote};
use livecraft_session::commands::scale::SetScale;
use livecraft_session::commands::scene::{CreateScene, DeleteScene, DuplicateScene};
use livecraft_session::commands::track::{
    CreateAudioTrack, CreateGroupTrack, CreateMidiTrack, CreateReturnTrack, DeleteTrack,
    RenameTrack, ReorderTrack, SetTrackArmed, SetTrackColor, SetTrackMuted, SetTrackPan,
    SetTrackSend, SetTrackSoloed, SetTrackVolume,
};
use livecraft_session::commands::transport::{SetLoop, SetMetronome, SetTempo, SetTimeSignature};
use livecraft_session::{CommandError, Session, Transaction};
use livecraft_time::Beats;
use proptest::prelude::*;
use std::path::PathBuf;

#[test]
fn lossless_session_edits_undo_redo_and_save_preserve_unknown_xml() {
    let dir = temp_dir("edits_undo_redo_save");
    let input_path = dir.join("input.als");
    let output_path = dir.join("output.als");

    AlsDocument::from_xml(RICH_UNKNOWN_XML)
        .expect("parse fixture")
        .save_als(&input_path)
        .expect("write input .als");

    let mut session = Session::open_als(&input_path).expect("Session::open_als");
    assert!(session.als_document().is_some());
    assert!(!session.is_dirty());
    assert_eq!(
        session.project().tracks[0].device_chain[1].device_type,
        DeviceType::Custom("MxDeviceAudioEffect".to_owned())
    );

    session
        .execute(RenameTrack::new(TrackId(1), "Bass Lead".to_owned()))
        .expect("rename track");
    session.execute(SetTempo::new(136.5)).expect("set tempo");
    session
        .execute(SetTrackVolume::new(TrackId(1), -4.5))
        .expect("set volume");
    session
        .execute(CreateMidiClip::new(TrackId(1), 0, "Lead Hook", 4.0))
        .expect("create midi clip");

    let clip_id = session
        .project()
        .track(TrackId(1))
        .and_then(|t| t.clip_slots.first())
        .and_then(|s| s.clip.as_ref())
        .expect("created clip")
        .id;

    session
        .execute(AddNote::new(clip_id, MidiNote::new(60, 0.0, 1.0, 110)))
        .expect("add note 1");
    session
        .execute(AddNote::new(clip_id, MidiNote::new(64, 1.0, 0.5, 95)))
        .expect("add note 2");

    // Undo and redo both note addition and volume change.
    assert!(session.undo());
    match &session
        .project()
        .track(TrackId(1))
        .expect("track 1")
        .clip_slots[0]
        .clip
        .as_ref()
        .expect("clip")
        .payload
    {
        ClipPayload::Midi(md) => assert_eq!(md.notes.len(), 1),
        ClipPayload::Audio(_) => panic!("expected MIDI clip"),
    }
    assert!(session.redo().expect("redo note"));

    session
        .execute(SetTrackVolume::new(TrackId(1), -12.0))
        .expect("change volume again");
    assert!(session.undo());
    assert_eq!(
        session.project().track(TrackId(1)).expect("t1").volume_db,
        -4.5
    );
    assert!(session.redo().expect("redo volume"));
    assert_eq!(
        session.project().track(TrackId(1)).expect("t1").volume_db,
        -12.0
    );
    session
        .execute(SetTrackVolume::new(TrackId(1), -4.5))
        .expect("set final volume");
    assert!(session.is_dirty());

    session.save(&output_path).expect("Session::save");
    assert!(!session.is_dirty());

    let saved_xml = AlsParser::decompress_to_xml(&output_path).expect("decompress saved");
    // All untouched unknown XML nodes, attributes, quotes, comments, and M4L device survived identically.
    assert!(saved_xml.contains(r#"Revision="fixture-rev""#));
    assert!(saved_xml.contains(r#"SelectedToolPanel="7""#));
    assert!(saved_xml.contains(r#"CustomTrackAttr='keep-single-quote'"#));
    assert!(saved_xml.contains(r#"<CustomUnknownRootTag KeepMe='{"raw": [1, 2, 3]}' />"#));
    assert!(saved_xml.contains("<!-- Preserve root comment -->"));
    assert!(saved_xml.contains(r#"<TrackDelay Value="0" />"#));
    assert!(saved_xml.contains(r#"<CustomTrackMeta Foo="bar" />"#));
    assert!(saved_xml.contains(r#"<AudioSpecificExtra Tag="untouched" />"#));
    assert!(saved_xml.contains(r#"<MxDeviceAudioEffect Id="301">"#));
    assert!(saved_xml.contains(r#"<UserName Value="M4L Shimmer" />"#));
    assert!(
        saved_xml.contains("<PatcherBlob><![CDATA[<maxpat>{opaque_data}</maxpat>]]></PatcherBlob>")
    );
    assert!(saved_xml.contains(r#"<CustomParam Attr='single-quoted' />"#));

    let reopened = AlsDocument::open(&output_path)
        .expect("reopen saved .als")
        .into_project();
    assert!(reopened.validate().is_empty());
    assert_projects_equivalent(session.project(), &reopened);
    assert_eq!(reopened.tracks[0].name, "Bass Lead");
    assert_approx_f64(
        reopened.transport.tempo.bpm_at(Beats(0.0)),
        136.5,
        1e-6,
        "bpm",
    );
    assert_approx_f32(reopened.tracks[0].volume_db, -4.5, 1e-3, "volume_db");
    assert_eq!(
        reopened.tracks[0].device_chain[1].device_type,
        DeviceType::Custom("MxDeviceAudioEffect".to_owned())
    );
    assert_eq!(reopened.tracks[1].name, "2-Audio");
}

#[test]
fn undoing_all_edits_saves_identical_xml() {
    let dir = temp_dir("undo_all_zero_diff");
    let input_path = dir.join("input.als");
    let output_path = dir.join("undone.als");

    AlsDocument::from_xml(RICH_UNKNOWN_XML)
        .expect("parse fixture")
        .save_als(&input_path)
        .expect("write input");

    let mut session = Session::open_als(&input_path).expect("open_als");
    session
        .execute(RenameTrack::new(TrackId(1), "Temp Name".to_owned()))
        .expect("rename");
    session.execute(SetTempo::new(148.0)).expect("tempo");
    session
        .execute(SetTrackVolume::new(TrackId(1), -8.0))
        .expect("vol");
    session
        .execute(CreateMidiClip::new(TrackId(1), 0, "Temp Clip", 4.0))
        .expect("clip");
    let clip_id = session
        .project()
        .track(TrackId(1))
        .and_then(|t| t.clip_slots.first())
        .and_then(|s| s.clip.as_ref())
        .expect("clip")
        .id;
    session
        .execute(AddNote::new(clip_id, MidiNote::new(62, 0.0, 1.0, 100)))
        .expect("note");

    while session.undo() {}
    session.save(&output_path).expect("save undone");

    let saved_xml = AlsParser::decompress_to_xml(&output_path).expect("decompress");
    assert_eq!(saved_xml, RICH_UNKNOWN_XML);
}

#[test]
fn session_constructors_and_doc_attachment_preserve_lossless_als() {
    let dir = temp_dir("constructors_and_attachment");
    let input_path = dir.join("input.als");
    let from_doc_path = dir.join("from_doc.als");
    let driver_path = dir.join("with_driver.als");
    let open_file_path = dir.join("open_file.als");
    let attached_path = dir.join("attached.als");

    AlsDocument::from_xml(RICH_UNKNOWN_XML)
        .expect("parse fixture")
        .save_als(&input_path)
        .expect("save input");

    // 1. Session::from_als_document
    let doc = AlsDocument::open(&input_path).expect("open doc");
    let mut from_doc = Session::from_als_document(doc, None);
    assert!(from_doc.als_document().is_some());
    from_doc
        .execute(RenameTrack::new(TrackId(1), "FromDoc".to_owned()))
        .expect("rename");
    from_doc.save(&from_doc_path).expect("save from_doc");
    let xml1 = AlsParser::decompress_to_xml(&from_doc_path).expect("xml1");
    assert!(xml1.contains(r#"<CustomUnknownRootTag KeepMe='{"raw": [1, 2, 3]}' />"#));
    assert!(xml1.contains("FromDoc"));

    // 2. Session::open_als_with_driver
    let mut with_driver =
        Session::open_als_with_driver(&input_path, null_factory(), None).expect("with_driver");
    assert!(with_driver.als_document().is_some());
    with_driver
        .execute(SetTempo::new(112.0))
        .expect("set tempo");
    with_driver.save(&driver_path).expect("save with_driver");
    let xml2 = AlsParser::decompress_to_xml(&driver_path).expect("xml2");
    assert!(xml2.contains(r#"<MxDeviceAudioEffect Id="301">"#));
    assert!(xml2.contains(r#"<Manual Value="112" />"#));

    // 3. Session::open_file (falls back to silent/none gracefully when headless)
    let mut opened_file = Session::open_file(&input_path).expect("open_file");
    assert!(opened_file.als_document().is_some());
    opened_file
        .execute(SetTrackVolume::new(TrackId(2), -3.0))
        .expect("set volume");
    opened_file.save(&open_file_path).expect("save open_file");
    let reopened = AlsDocument::open(&open_file_path)
        .expect("reopen")
        .into_project();
    assert_approx_f32(reopened.tracks[1].volume_db, -3.0, 1e-3, "open_file volume");

    // 4. Session::new -> save (attaches generated doc) -> edit -> save -> attach_als_document
    let mut fresh = Session::new(Project::new_default("Fresh"), None);
    assert!(fresh.als_document().is_none());
    fresh.save(&attached_path).expect("initial save");
    assert!(fresh.als_document().is_some());
    fresh
        .execute(RenameTrack::new(TrackId(1), "PatchedAfterSave".to_owned()))
        .expect("rename after initial save");
    fresh.save(&attached_path).expect("incremental save");
    let reloaded = AlsDocument::open(&attached_path)
        .expect("reload attached")
        .into_project();
    assert_eq!(reloaded.tracks[0].name, "PatchedAfterSave");

    let replacement_doc = AlsDocument::open(&input_path).expect("open replacement");
    fresh.attach_als_document(replacement_doc);
    assert!(fresh.als_document().is_some());
}

fn pick_non_main_track(project: &Project, pick: u8) -> Option<(TrackId, TrackType)> {
    let candidates: Vec<(TrackId, TrackType)> = project
        .tracks
        .iter()
        .chain(&project.return_tracks)
        .map(|t| (t.id, t.track_type))
        .collect();
    if candidates.is_empty() {
        None
    } else {
        Some(candidates[usize::from(pick) % candidates.len()])
    }
}

fn pick_clip(project: &Project, pick: u8) -> Option<(TrackId, usize, ClipId, bool)> {
    let clips: Vec<(TrackId, usize, ClipId, bool)> = project
        .tracks
        .iter()
        .flat_map(|t| {
            t.clip_slots.iter().enumerate().filter_map(move |(idx, s)| {
                s.clip
                    .as_ref()
                    .map(|c| (t.id, idx, c.id, matches!(c.payload, ClipPayload::Midi(_))))
            })
        })
        .collect();
    if clips.is_empty() {
        None
    } else {
        Some(clips[usize::from(pick) % clips.len()])
    }
}

fn pick_empty_slot(project: &Project, pick: u8) -> Option<(TrackId, usize)> {
    let slots: Vec<(TrackId, usize)> = project
        .tracks
        .iter()
        .filter(|t| matches!(t.track_type, TrackType::Midi | TrackType::Audio))
        .flat_map(|t| {
            t.clip_slots
                .iter()
                .enumerate()
                .filter_map(move |(idx, s)| s.clip.is_none().then_some((t.id, idx)))
        })
        .collect();
    if slots.is_empty() {
        None
    } else {
        Some(slots[usize::from(pick) % slots.len()])
    }
}

fn pick_device(project: &Project, pick: u8) -> Option<(TrackId, DeviceId, DeviceType)> {
    let devices: Vec<(TrackId, DeviceId, DeviceType)> = project
        .all_tracks()
        .flat_map(|t| {
            t.device_chain
                .iter()
                .map(move |d| (t.id, d.id, d.device_type.clone()))
        })
        .collect();
    if devices.is_empty() {
        None
    } else {
        Some(devices[usize::from(pick) % devices.len()].clone())
    }
}

fn apply_valid_command(
    tx: &mut Transaction<'_>,
    kind: u8,
    a: u8,
    b: u8,
    val: f32,
) -> Result<(), CommandError> {
    let project = tx.project();
    match kind % 28 {
        0 => {
            if let Some((tid, _)) = pick_non_main_track(project, a) {
                tx.run(RenameTrack::new(tid, format!("Trk_{b}")))?;
            }
        }
        1 => {
            if let Some((tid, _)) = pick_non_main_track(project, a) {
                tx.run(SetTrackVolume::new(tid, val.clamp(-40.0, 6.0)))?;
            }
        }
        2 => {
            if let Some((tid, _)) = pick_non_main_track(project, a) {
                let pan = (val / 40.0).clamp(-1.0, 1.0);
                tx.run(SetTrackPan::new(tid, pan))?;
            }
        }
        3 => {
            let armable: Vec<TrackId> = project
                .tracks
                .iter()
                .filter(|t| !matches!(t.track_type, TrackType::Return | TrackType::Main))
                .map(|t| t.id)
                .collect();
            if !armable.is_empty() {
                let tid = armable[usize::from(a) % armable.len()];
                tx.run(SetTrackArmed::new(tid, b.is_multiple_of(2)))?;
            }
        }
        4 => {
            if let Some((tid, _)) = pick_non_main_track(project, a) {
                tx.run(SetTrackMuted::new(tid, b.is_multiple_of(2)))?;
            }
        }
        5 => {
            if let Some((tid, _)) = pick_non_main_track(project, a) {
                tx.run(SetTrackSoloed::new(tid, b.is_multiple_of(2)))?;
            }
        }
        6 => {
            if let Some((tid, _)) = pick_non_main_track(project, a) {
                let color = ABLETON_PALETTE[usize::from(b) % ABLETON_PALETTE.len()];
                tx.run(SetTrackColor::new(tid, color))?;
            }
        }
        7 => {
            if !project.return_tracks.is_empty()
                && let Some((tid, _)) = pick_non_main_track(project, a)
            {
                let rid = project.return_tracks[usize::from(b) % project.return_tracks.len()].id;
                let level = if b.is_multiple_of(5) {
                    0.0
                } else {
                    0.05 + (val.abs() / 40.0).min(0.95)
                };
                tx.run(SetTrackSend::new(tid, rid, level))?;
            }
        }
        8 if project.return_tracks.len() == 2 => {
            tx.run(CreateAudioTrack::new(format!("Aud_{a}")))?;
        }
        9 if project.return_tracks.len() == 2 => {
            tx.run(CreateMidiTrack::new(format!("Mid_{a}")))?;
        }
        10 if project.return_tracks.len() == 2 => {
            tx.run(CreateGroupTrack::new(format!("Grp_{a}")))?;
        }
        11 => {
            tx.run(CreateReturnTrack::new(format!("Ret_{a}")))?;
        }
        12 if project.tracks.len() > 1 => {
            let tid = project.tracks[usize::from(a) % project.tracks.len()].id;
            tx.run(DeleteTrack::new(tid))?;
        }
        13 if project.tracks.len() > 1 => {
            let tid = project.tracks[usize::from(a) % project.tracks.len()].id;
            let idx = usize::from(b) % project.tracks.len();
            tx.run(ReorderTrack::new(tid, idx))?;
        }
        14 => {
            if let Some((tid, slot)) = pick_empty_slot(project, a) {
                let len = 1.0 + f64::from(b % 8);
                tx.run(CreateMidiClip::new(tid, slot, format!("Clip_{a}"), len))?;
            }
        }
        15 => {
            if let Some((_, _, cid, _)) = pick_clip(project, a)
                && let Some((tid, slot)) = pick_empty_slot(project, b)
            {
                tx.run(MoveClip::new(cid, tid, slot))?;
            }
        }
        16 => {
            if let Some((_, _, cid, _)) = pick_clip(project, a) {
                tx.run(DeleteClip::new(cid))?;
            }
        }
        17 => {
            if let Some((_, _, cid, _)) = pick_clip(project, a) {
                tx.run(RenameClip::new(cid, format!("Clip_{b}")))?;
            }
        }
        18 => {
            if let Some((_, _, cid, _)) = pick_clip(project, a) {
                let start = f64::from(a % 4);
                let end = start + 1.0 + f64::from(b % 8);
                tx.run(SetClipLoop::new(cid, start, end, b.is_multiple_of(2)))?;
            }
        }
        19 => {
            if let Some((_, _, cid, true)) = pick_clip(project, a) {
                let note = MidiNote::new(
                    24 + (b % 72),
                    f64::from(a % 8),
                    0.5 + f64::from(b % 4) * 0.25,
                    1 + (a % 126),
                );
                tx.run(AddNote::new(cid, note))?;
            }
        }
        20 => {
            if let Some((tid, slot, cid, true)) = pick_clip(project, a)
                && let Some(clip) = project
                    .track(tid)
                    .and_then(|t| t.clip_slots.get(slot))
                    .and_then(|s| s.clip.as_ref())
                && let ClipPayload::Midi(ref md) = clip.payload
                && !md.notes.is_empty()
            {
                let idx = usize::from(b) % md.notes.len();
                tx.run(RemoveNote::new(cid, idx))?;
            }
        }
        21 => {
            if let Some((tid, _)) = pick_non_main_track(project, a) {
                let kinds = [
                    DeviceType::Operator,
                    DeviceType::EqEight,
                    DeviceType::Compressor,
                    DeviceType::Limiter,
                    DeviceType::Utility,
                    DeviceType::Delay,
                    DeviceType::Reverb,
                    DeviceType::Saturator,
                    DeviceType::AutoFilter,
                    DeviceType::Chorus,
                    DeviceType::AutoPan,
                    DeviceType::Gate,
                    DeviceType::EqThree,
                    DeviceType::Redux,
                    DeviceType::Simpler,
                ];
                let dtype = kinds[usize::from(b) % kinds.len()].clone();
                tx.run(InsertDevice::new(tid, dtype, format!("Dev_{b}")))?;
            }
        }
        22 => {
            if let Some((tid, did, dtype)) = pick_device(project, a) {
                let table = param_table(&dtype);
                if !table.is_empty() {
                    let spec = &table[usize::from(b) % table.len()];
                    let pval = 0.5 + val.abs();
                    tx.run(SetDeviceParam::new(tid, did, spec.name, pval))?;
                }
            }
        }
        23 => {
            if let Some((tid, did, _)) = pick_device(project, a) {
                tx.run(SetDeviceEnabled::new(tid, did, b.is_multiple_of(2)))?;
            }
        }
        24 => {
            if let Some((tid, did, _)) = pick_device(project, a)
                && let Some(trk) = project.track(tid)
                && trk.device_chain.len() > 1
            {
                let idx = usize::from(b) % trk.device_chain.len();
                tx.run(MoveDevice::new(tid, did, idx))?;
            }
        }
        25 => {
            if let Some((tid, did, dtype)) = pick_device(project, a)
                && !matches!(dtype, DeviceType::Custom(_))
            {
                tx.run(RemoveDevice::new(tid, did))?;
            }
        }
        26 => {
            let bpm = 40.0 + (val.abs() * 4.0) % 180.0;
            tx.run(SetTempo::new(bpm))?;
        }
        _ => match b % 5 {
            0 => {
                let den = if a.is_multiple_of(2) { 4 } else { 8 };
                tx.run(SetTimeSignature::new((a % 7) + 1, den))?;
            }
            1 => {
                tx.run(SetLoop::new(
                    a.is_multiple_of(2),
                    f64::from(a % 8),
                    1.0 + f64::from(b % 16),
                ))?;
            }
            2 => {
                tx.run(SetMetronome::new(a.is_multiple_of(2)))?;
            }
            3 => {
                let st = ScaleType::ALL[usize::from(b) % ScaleType::ALL.len()];
                tx.run(SetScale::new(ScaleConfig {
                    root_note: a % 12,
                    scale_type: st,
                    is_active: (u16::from(a) + u16::from(b)).is_multiple_of(2),
                }))?;
            }
            _ => {
                if !project.scenes.is_empty() && a.is_multiple_of(3) {
                    let sid: SceneId = project.scenes[usize::from(b) % project.scenes.len()].id;
                    if b.is_multiple_of(2) {
                        tx.run(DuplicateScene::new(sid))?;
                    } else {
                        tx.run(DeleteScene::new(sid))?;
                    }
                } else {
                    tx.run(CreateScene::new(format!("Sc_{a}")))?;
                }
            }
        },
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn random_transactions_roundtrip_losslessly_through_session_save(
        start_from_als in any::<bool>(),
        phase1 in prop::collection::vec(prop::collection::vec((any::<u8>(), any::<u8>(), any::<u8>(), -40.0f32..6.0), 1..=5), 1..=4),
        phase2 in prop::collection::vec(prop::collection::vec((any::<u8>(), any::<u8>(), any::<u8>(), -40.0f32..6.0), 1..=5), 1..=4),
        do_undo in any::<bool>(),
        do_redo in any::<bool>(),
    ) {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("lossless_als_proptest");
        std::fs::create_dir_all(&dir).expect("create proptest dir");
        let als_path = dir.join("case.als");

        let mut session = if start_from_als {
            let doc = AlsDocument::from_xml(RICH_UNKNOWN_XML).expect("parse fixture");
            Session::from_als_document(doc, None)
        } else {
            Session::new(Project::new_default("PropTest"), None)
        };

        for (tx_idx, batch) in phase1.into_iter().enumerate() {
            session
                .transaction(&format!("Tx1_{tx_idx}"), |tx| {
                    for (kind, a, b, val) in batch {
                        apply_valid_command(tx, kind, a, b, val)?;
                    }
                    Ok(())
                })
                .expect("phase 1 transaction");
        }
        if do_undo {
            let _ = session.undo();
            if do_redo {
                let _ = session.redo().expect("redo");
            }
        }
        prop_assert!(session.project().validate().is_empty());

        session.save(&als_path).expect("phase 1 save");
        let reloaded_1 = AlsDocument::open(&als_path).expect("reopen 1").into_project();
        prop_assert!(reloaded_1.validate().is_empty());
        assert_projects_equivalent(session.project(), &reloaded_1);

        // Phase 2 exercises incremental patching on the retained AlsDocument.
        for (tx_idx, batch) in phase2.into_iter().enumerate() {
            session
                .transaction(&format!("Tx2_{tx_idx}"), |tx| {
                    for (kind, a, b, val) in batch {
                        apply_valid_command(tx, kind, a, b, val)?;
                    }
                    Ok(())
                })
                .expect("phase 2 transaction");
        }
        if do_undo {
            let _ = session.undo();
        }
        prop_assert!(session.project().validate().is_empty());

        session.save(&als_path).expect("phase 2 save");
        let reloaded_2 = AlsDocument::open(&als_path).expect("reopen 2").into_project();
        prop_assert!(reloaded_2.validate().is_empty());
        assert_projects_equivalent(session.project(), &reloaded_2);

        if start_from_als {
            let saved_xml = AlsParser::decompress_to_xml(&als_path).expect("decompress");
            let has_root_tag = saved_xml
                .contains(r#"<CustomUnknownRootTag KeepMe='{"raw": [1, 2, 3]}' />"#);
            let has_comment = saved_xml.contains("<!-- Preserve root comment -->");
            prop_assert!(has_root_tag, "missing CustomUnknownRootTag");
            prop_assert!(has_comment, "missing root comment");
        }
    }
}
