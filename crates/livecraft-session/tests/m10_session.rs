//! Integration tests for M10 workflows in `livecraft-session`:
//! - Project folder detection, `save_into_project`, and moved-project relative playback
//! - Missing-file detection, manual & automatic repair + undo/redo
//! - Collect All and Save (copy, never move) & Unused Files detection/deletion safety
//! - SMF `.mid` import/export + undo
//! - Live Clip `.alc` import (manual 5.3 empty vs non-empty track rule) & export (`CollectFilesOnExport`)
//! - Device `.adv` & Rack `.adg` presets + undo
//! - Pack `.alp` installation (including zip-slip rejection)
//! - B11 one-pass `health_report`

use livecraft_engine::driver::{AudioDriver, DriverConfig, DriverError, NullDriver};
use livecraft_formats::als::AlsDocument;
use livecraft_formats::live_clip::{LiveClipFile, load_live_clip, save_live_clip};
use livecraft_formats::midi_file::{SmfFormat, read_smf_file, write_smf};
use livecraft_formats::project_dir::create_project_layout;
use livecraft_model::device::{Device, DeviceSampleRef, DeviceType};
use livecraft_model::ids::{ClipId, DeviceId, TrackId};
use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project};
use livecraft_session::commands::device::RemoveDevice;
use livecraft_session::commands::track::CreateMidiTrack;
use livecraft_session::{
    AutomaticSearchOptions, CollectAllOptions, CollectFilesOnExport, DriverFactory,
    ExportClipOutcome, LibraryPreferences, PackError, Preferences, ProjectDirError, Session,
    SessionFormatError, UnusedFileCategory,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn null_factory() -> DriverFactory {
    Box::new(
        |config: &DriverConfig| -> Result<Box<dyn AudioDriver>, DriverError> {
            Ok(Box::new(NullDriver::new(
                config.sample_rate.unwrap_or(44_100),
                config.buffer_frames.unwrap_or(512),
                2,
            )))
        },
    )
}

fn test_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("m10_session")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create m10 test dir");
    dir
}

/// Writes a valid 16-bit mono PCM WAV file with non-zero tone samples so the engine produces audio.
fn write_pcm_wav(path: &Path, sample_rate: u32, frames: usize, amplitude: i16) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create wav parent");
    }
    let data_bytes = (frames * 2) as u32;
    let riff_size = 36 + data_bytes;
    let byte_rate = sample_rate * 2;
    let mut out = Vec::with_capacity(44 + frames * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM format
    out.extend_from_slice(&1u16.to_le_bytes()); // 1 channel
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_bytes.to_le_bytes());
    for i in 0..frames {
        let sample = if i % 2 == 0 { amplitude } else { -amplitude };
        out.extend_from_slice(&sample.to_le_bytes());
    }
    fs::write(path, out).expect("write wav");
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320u32 & mask);
        }
    }
    !crc
}

/// Writes an uncompressed STORE-mode ZIP/ALP archive without needing `zip` in dev-dependencies.
fn write_store_zip(path: &Path, entries: &[(&str, &[u8])]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create zip parent");
    }
    let mut data = Vec::new();
    let mut central = Vec::new();
    for &(name, payload) in entries {
        let name_bytes = name.as_bytes();
        let crc = crc32(payload);
        let size = payload.len() as u32;
        let offset = data.len() as u32;

        // Local file header
        data.extend_from_slice(b"PK\x03\x04");
        data.extend_from_slice(&20u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes()); // STORE
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&crc.to_le_bytes());
        data.extend_from_slice(&size.to_le_bytes());
        data.extend_from_slice(&size.to_le_bytes());
        data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(name_bytes);
        data.extend_from_slice(payload);

        // Central directory header
        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // STORE
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let cd_offset = data.len() as u32;
    let cd_size = central.len() as u32;
    let count = entries.len() as u16;
    data.extend_from_slice(&central);
    data.extend_from_slice(b"PK\x05\x06");
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());
    data.extend_from_slice(&count.to_le_bytes());
    data.extend_from_slice(&count.to_le_bytes());
    data.extend_from_slice(&cd_size.to_le_bytes());
    data.extend_from_slice(&cd_offset.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());

    fs::write(path, data).expect("write zip");
}

fn make_audio_set(name: &str, refs: &[(&str, Option<&str>)]) -> Project {
    let mut project = Project::new_default(name);
    project.tracks[2].device_chain.clear();
    project.main_track.device_chain.clear();
    for (idx, &(path, rel)) in refs.iter().enumerate() {
        let mut clip = Clip::new_audio(ClipId(900 + idx as u64), format!("Clip_{idx}"), path, 4.0);
        if let ClipPayload::Audio(ref mut audio) = clip.payload {
            audio.relative_path = rel.map(str::to_owned);
        }
        project.tracks[2].clip_slots[idx].clip = Some(clip);
    }
    project
}

#[test]
fn opening_als_detects_missing_files_and_moved_project_relative_fallback_clears_and_plays() {
    let dir = test_dir("moved_relative_play");

    // 1. Missing file detected immediately on open_als.
    let broken_proj = make_audio_set(
        "BrokenSet",
        &[("C:/nonexistent_drive_xyz/missing_kick.wav", None)],
    );
    let broken_als = dir.join("Broken.als");
    AlsDocument::from_project(&broken_proj)
        .expect("build broken doc")
        .save_als(&broken_als)
        .expect("save broken als");

    let broken_session = Session::open_als(&broken_als).expect("open broken als");
    assert_eq!(broken_session.set_path(), Some(broken_als.as_path()));
    assert_eq!(broken_session.missing_files().len(), 1);
    assert_eq!(
        broken_session.missing_files()[0].file_name,
        "missing_kick.wav"
    );

    // 2. Moved Live Project with stale absolute path + valid RelativePath clears missing status and plays.
    let orig_root = dir.join("Original Project");
    create_project_layout(&orig_root).expect("orig layout");
    let orig_wav = orig_root.join("Samples/Imported/beat.wav");
    write_pcm_wav(&orig_wav, 44_100, 16_384, 16_000);

    let moved_proj = make_audio_set(
        "MovedSet",
        &[(
            "C:/old_machine_path_12345/Samples/Imported/beat.wav",
            Some("Samples/Imported/beat.wav"),
        )],
    );
    let orig_als = orig_root.join("MovedSet.als");
    AlsDocument::from_project(&moved_proj)
        .expect("build moved doc")
        .save_als(&orig_als)
        .expect("save orig als");

    let moved_root = dir.join("Moved Project");
    fs::rename(&orig_root, &moved_root).expect("move project folder");
    let moved_als = moved_root.join("MovedSet.als");

    let mut session =
        Session::open_als_with_driver(&moved_als, null_factory(), None).expect("open moved als");
    assert_eq!(session.set_path(), Some(moved_als.as_path()));
    assert_eq!(
        session.project_context().project_root.as_deref(),
        Some(moved_root.as_path())
    );
    assert!(
        session.missing_files().is_empty(),
        "relative fallback inside moved project must clear missing_files: {:?}",
        session.missing_files()
    );

    session.trigger_clip(TrackId(3), 0);
    session.play();
    let deadline = Instant::now() + Duration::from_millis(800);
    let mut heard_audio = false;
    while Instant::now() < deadline {
        let status = session.status();
        if status.master_meter.peak_l > 0.05 || status.master_meter.peak_r > 0.05 {
            heard_audio = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        heard_audio,
        "moved project clip resolved via RelativePath must produce non-zero audio meter output"
    );
}

#[test]
fn save_into_project_and_library_preferences_update_context_and_persist() {
    let dir = test_dir("save_into_project");
    let prefs_path = dir.join("Preferences.toml");
    let user_lib = dir.join("UserLibrary");
    let pack_root = dir.join("FactoryPacks");

    let mut session = Session::with_driver_factory(
        Project::new_default("Untracked"),
        null_factory(),
        Some(prefs_path.clone()),
    );
    assert!(session.set_path().is_none());
    assert!(session.project_context().project_root.is_none());

    let target_new = dir.join("NewSong.als");
    let loc1 = session
        .save_into_project(&target_new)
        .expect("save into new project");
    assert!(loc1.created_new_project);
    let expected_root = dir.join("NewSong Project");
    assert_eq!(loc1.project_dir.root, expected_root);
    assert_eq!(loc1.set_path, expected_root.join("NewSong.als"));
    assert!(expected_root.join("Ableton Project Info").is_dir());
    assert!(expected_root.join("Samples/Imported").is_dir());
    assert_eq!(session.set_path(), Some(loc1.set_path.as_path()));
    assert_eq!(
        session.project_context().project_root.as_deref(),
        Some(expected_root.as_path())
    );
    assert!(!session.is_dirty());
    assert!(session.als_document().is_some());

    // Saving a second Set path inside the existing project reuses it (`created_new_project == false`).
    let target_sub = expected_root.join("NewSong_v2.als");
    let loc2 = session
        .save_into_project(&target_sub)
        .expect("save inside existing project");
    assert!(!loc2.created_new_project);
    assert_eq!(loc2.set_path, target_sub);
    assert_eq!(session.set_path(), Some(target_sub.as_path()));

    // Updating LibraryPreferences updates ProjectContext and saves Preferences.toml.
    let lib_prefs = LibraryPreferences {
        user_library: Some(user_lib.clone()),
        factory_packs_roots: vec![pack_root.clone()],
        collect_files_on_export: CollectFilesOnExport::Ask,
        ..LibraryPreferences::default()
    };
    session
        .set_library_preferences(lib_prefs.clone())
        .expect("set library preferences");
    assert_eq!(
        session.project_context().user_library.as_deref(),
        Some(user_lib.as_path())
    );
    assert_eq!(
        session.project_context().factory_packs_roots,
        vec![pack_root]
    );
    let reloaded_prefs = Preferences::load(&prefs_path).expect("reload prefs");
    assert_eq!(reloaded_prefs.library, lib_prefs);
}

#[test]
fn locate_missing_manual_and_auto_repair_with_undo_and_redo() {
    let dir = test_dir("locate_missing");
    let root = dir.join("Repair Project");
    create_project_layout(&root).expect("layout");

    // 1. Manual repair + undo/redo (including non-existent replacement rejection).
    let replacement = root.join("Samples/Imported/actual_kick.wav");
    write_pcm_wav(&replacement, 44_100, 2048, 12_000);

    let proj_manual = make_audio_set("Manual", &[("C:/missing/old_kick.wav", None)]);
    let als_manual = root.join("Manual.als");
    AlsDocument::from_project(&proj_manual)
        .expect("doc")
        .save_als(&als_manual)
        .expect("save");

    let mut manual_session = Session::open_als(&als_manual).expect("open manual");
    assert_eq!(manual_session.missing_files().len(), 1);
    assert!(
        manual_session
            .locate_missing_manual("C:/missing/old_kick.wav", &dir.join("does_not_exist.wav"))
            .is_err()
    );
    let count = manual_session
        .locate_missing_manual("C:/missing/old_kick.wav", &replacement)
        .expect("manual locate");
    assert_eq!(count, 1);
    assert!(manual_session.missing_files().is_empty());

    assert!(manual_session.undo());
    assert_eq!(manual_session.missing_files().len(), 1);
    assert!(manual_session.redo().expect("redo manual locate"));
    assert!(manual_session.missing_files().is_empty());

    // 2. Automatic repair: single-candidate resolved, zero/multi-candidate left unresolved, undoable.
    let search_dir = dir.join("SearchPool");
    write_pcm_wav(
        &search_dir.join("Sub1/unique_snare.wav"),
        44_100,
        1024,
        8_000,
    );
    write_pcm_wav(&search_dir.join("SubA/ambiguous.wav"), 44_100, 1024, 8_000);
    write_pcm_wav(&search_dir.join("SubB/ambiguous.wav"), 44_100, 1024, 8_000);

    let proj_auto = make_audio_set(
        "Auto",
        &[
            ("C:/old/unique_snare.wav", None),
            ("C:/old/ambiguous.wav", None),
            ("C:/old/nowhere.wav", None),
        ],
    );
    let als_auto = root.join("Auto.als");
    AlsDocument::from_project(&proj_auto)
        .expect("auto doc")
        .save_als(&als_auto)
        .expect("save auto");

    let mut auto_session = Session::open_als(&als_auto).expect("open auto");
    assert_eq!(auto_session.missing_files().len(), 3);

    let rep = auto_session
        .locate_missing_auto(&AutomaticSearchOptions {
            search_folder: Some(search_dir),
            search_project: true,
            search_library: false,
            library_roots: Vec::new(),
        })
        .expect("locate missing auto");
    assert_eq!(rep.repaired.len(), 1);
    assert_eq!(rep.repaired[0].0, "C:/old/unique_snare.wav");
    assert_eq!(rep.unresolved.len(), 2);
    assert_eq!(auto_session.missing_files().len(), 2);
    assert_eq!(auto_session.entries()[0].label, "Locate Missing Files");

    assert!(auto_session.undo());
    assert_eq!(auto_session.missing_files().len(), 3);
    assert!(auto_session.redo().expect("redo auto locate"));
    assert_eq!(auto_session.missing_files().len(), 2);

    // 3. Manual repair of an unmodeled XML <FileRef> patches `als_doc` and persists across save().
    let xml_only_als = root.join("XmlOnly.als");
    let xml_only = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
        <Ableton MajorVersion=\"5\" MinorVersion=\"12.0_12117\">\n\
        <LiveSet><Tracks/><MainTrack><DeviceChain><Devices>\n\
        <MultiSamplePart Id=\"42\"><SampleRef><FileRef>\n\
        <Path Value=\"C:/missing/xml_only_pad.wav\"/>\n\
        </FileRef></SampleRef></MultiSamplePart>\n\
        </Devices></DeviceChain></MainTrack></LiveSet></Ableton>";
    AlsDocument::from_xml_named(xml_only, "XmlOnly")
        .expect("parse xml_only")
        .save_als(&xml_only_als)
        .expect("save xml_only");
    let mut xml_session = Session::open_als(&xml_only_als).expect("open xml_only");
    assert_eq!(xml_session.missing_files().len(), 1);
    let patched = xml_session
        .locate_missing_manual("C:/missing/xml_only_pad.wav", &replacement)
        .expect("locate xml ref");
    assert_eq!(patched, 1);
    assert!(xml_session.missing_files().is_empty());
    xml_session.save(&xml_only_als).expect("save repaired xml");
    let reopened_xml = Session::open_als(&xml_only_als).expect("reopen repaired xml");
    assert!(reopened_xml.missing_files().is_empty());
}

#[test]
fn collect_all_and_save_copies_never_moves_and_unused_file_deletion_enforces_safety() {
    let dir = test_dir("collect_and_unused");
    let root = dir.join("Main Project");
    create_project_layout(&root).expect("main layout");
    let other_root = dir.join("Other Project");
    create_project_layout(&other_root).expect("other layout");
    let user_lib = dir.join("UserLibrary");
    let pack_root = dir.join("FactoryPacks");

    let ext_elsewhere = dir.join("External/kick.wav");
    let ext_other_proj = other_root.join("Samples/Recorded/snare.wav");
    let ext_user_lib = user_lib.join("hats.wav");
    let ext_pack = pack_root.join("Core/perc.wav");
    for p in [&ext_elsewhere, &ext_other_proj, &ext_user_lib, &ext_pack] {
        write_pcm_wav(p, 44_100, 1024, 9_000);
    }

    let proj = make_audio_set(
        "CollectSet",
        &[
            (&ext_elsewhere.to_string_lossy(), None),
            (&ext_other_proj.to_string_lossy(), None),
            (&ext_user_lib.to_string_lossy(), None),
            (&ext_pack.to_string_lossy(), None),
        ],
    );
    let als_path = root.join("CollectSet.als");
    AlsDocument::from_project(&proj)
        .expect("doc")
        .save_als(&als_path)
        .expect("save");

    let mut session = Session::open_als(&als_path).expect("open");
    session
        .set_library_preferences(LibraryPreferences {
            user_library: Some(user_lib.clone()),
            factory_packs_roots: vec![pack_root.clone()],
            collect_files_on_export: CollectFilesOnExport::Always,
            ..LibraryPreferences::default()
        })
        .expect("set library prefs");

    // Default options collect Elsewhere + OtherProject, skipping UserLibrary + FactoryPacks.
    let c1 = session
        .collect_all_and_save(&CollectAllOptions::default())
        .expect("collect default");
    assert_eq!(c1.collected.len(), 2);
    assert_eq!(c1.skipped_by_policy.len(), 2);
    assert!(!session.is_dirty());
    assert!(root.join("Samples/Imported/kick.wav").is_file());
    assert!(root.join("Samples/Imported/snare.wav").is_file());
    // Originals remain untouched (copy, never move).
    assert!(ext_elsewhere.is_file());
    assert!(ext_other_proj.is_file());

    // CollectAllOptions::all() collects UserLibrary + FactoryPacks as well.
    let c2 = session
        .collect_all_and_save(&CollectAllOptions::all())
        .expect("collect all");
    assert_eq!(c2.collected.len(), 2);
    assert!(c2.skipped_by_policy.is_empty());
    assert!(root.join("Samples/Imported/hats.wav").is_file());
    assert!(root.join("Samples/Imported/perc.wav").is_file());
    assert!(ext_user_lib.is_file());
    assert!(ext_pack.is_file());

    // Unused files detection and strict deletion safety.
    let orphan_imp = root.join("Samples/Imported/orphan_imp.wav");
    let orphan_rec = root.join("Samples/Recorded/orphan_rec.wav");
    let info_file = root.join("Ableton Project Info/ProjectInfo.cfg");
    let backup_wav = root.join("Backup/old_backup.wav");
    write_pcm_wav(&orphan_imp, 44_100, 512, 4_000);
    write_pcm_wav(&orphan_rec, 44_100, 512, 4_000);
    fs::write(&info_file, b"info").expect("write info");
    write_pcm_wav(&backup_wav, 44_100, 512, 4_000);

    let unused = session.find_unused_files().expect("find unused");
    assert_eq!(unused.unused_files.len(), 2);
    assert_eq!(
        unused.unused_files[0].category,
        UnusedFileCategory::Imported
    );
    assert_eq!(
        unused.unused_files[1].category,
        UnusedFileCategory::Recorded
    );

    // Must refuse external, referenced, protected, backup, and traversal paths.
    assert!(matches!(
        session.delete_unused_file(&ext_elsewhere),
        Err(ProjectDirError::WritePolicyDenied(_))
    ));
    assert!(matches!(
        session.delete_unused_file(&root.join("Samples/Imported/kick.wav")),
        Err(ProjectDirError::WritePolicyDenied(_))
    ));
    assert!(matches!(
        session.delete_unused_file(&info_file),
        Err(ProjectDirError::WritePolicyDenied(_))
    ));
    assert!(matches!(
        session.delete_unused_file(&backup_wav),
        Err(ProjectDirError::WritePolicyDenied(_))
    ));
    assert!(matches!(
        session.delete_unused_file(Path::new("../External/kick.wav")),
        Err(ProjectDirError::InvalidPath(_))
    ));

    // Symlink/junction pointing outside the project must also be refused.
    let link_wav = root.join("Samples/Imported/symlink_ext.wav");
    #[cfg(unix)]
    let _ = std::os::unix::fs::symlink(&ext_elsewhere, &link_wav);
    #[cfg(windows)]
    let _ = std::os::windows::fs::symlink_file(&ext_elsewhere, &link_wav);
    if fs::symlink_metadata(&link_wav).is_ok() {
        assert!(matches!(
            session.delete_unused_file(&link_wav),
            Err(ProjectDirError::WritePolicyDenied(_))
        ));
    }

    // Verified unused file deletion succeeds and leaves referenced/other unused files intact.
    session
        .delete_unused_file(&orphan_imp)
        .expect("delete verified orphan");
    assert!(!orphan_imp.exists());
    assert!(orphan_rec.is_file());
    assert!(root.join("Samples/Imported/kick.wav").is_file());
}

#[test]
fn midi_file_import_with_undo_and_export_midi_clip() {
    let dir = test_dir("midi_import_export");
    let mid_in = dir.join("melody.mid");
    let notes = vec![
        MidiNote::new(60, 0.0, 1.0, 100),
        MidiNote::new(64, 1.0, 0.5, 90),
        MidiNote::new(67, 2.0, 1.5, 110),
    ];
    let smf_bytes =
        write_smf(&notes, 128.0, (3, 4), SmfFormat::Format0, Some("LeadLine")).expect("encode smf");
    fs::write(&mid_in, smf_bytes).expect("write mid");

    let mut session = Session::new(Project::new_default("MidiIO"), None);
    let initial_tracks = session.project().tracks.len();

    // 1. Import into new track (track_id = None) -> 1 undo removes track and clip completely.
    let slot_clip_id = session
        .import_midi_file(None, None, None, &mid_in)
        .expect("import midi onto new track");
    assert_eq!(session.project().tracks.len(), initial_tracks + 1);
    let new_track_id = session.project().tracks.last().expect("new track").id;
    assert_eq!(session.entries()[0].label, "Import MIDI File");

    let mid_out = dir.join("exported_slot.mid");
    session
        .export_midi_clip(slot_clip_id, &mid_out)
        .expect("export slot midi clip");
    let parsed = read_smf_file(&mid_out).expect("read exported smf");
    assert_eq!(parsed.merged_notes.len(), 3);
    assert_eq!(parsed.merged_notes[0].pitch, 60);
    assert_eq!(parsed.merged_notes[2].pitch, 67);

    assert!(session.undo());
    assert_eq!(session.project().tracks.len(), initial_tracks);
    assert!(session.project().track(new_track_id).is_none());
    assert!(session.redo().expect("redo midi import"));
    assert_eq!(session.project().tracks.len(), initial_tracks + 1);

    // 2. Import into Arrangement of existing MIDI track -> undoable & exportable.
    let arr_clip_id = session
        .import_midi_file(Some(TrackId(1)), None, Some(4.0), &mid_in)
        .expect("import midi into arrangement");
    assert_eq!(session.project().tracks[0].arrangement_clips.len(), 1);
    assert_eq!(
        session.project().tracks[0].arrangement_clips[0].start_beat,
        4.0
    );
    let arr_out = dir.join("exported_arr.mid");
    session
        .export_midi_clip(arr_clip_id, &arr_out)
        .expect("export arrangement midi clip");
    assert_eq!(
        read_smf_file(&arr_out)
            .expect("read arr")
            .merged_notes
            .len(),
        3
    );

    assert!(session.undo());
    assert!(session.project().tracks[0].arrangement_clips.is_empty());
}

#[test]
fn live_clip_import_obeys_manual_5_3_and_export_obeys_collect_policy() {
    let dir = test_dir("live_clip_workflows");

    // Prepare a MIDI .alc containing notes + 2 devices with params.
    let mut midi_clip = Clip::new_midi(ClipId(40), "BassRiff", 4.0);
    if let ClipPayload::Midi(ref mut m) = midi_clip.payload {
        m.notes.push(MidiNote::new(36, 0.0, 0.5, 115));
    }
    let mut op = Device::new(DeviceId(50), "BassOp", DeviceType::Operator);
    op.set_param("filter_cutoff", 1320.0);
    let mut sat = Device::new(DeviceId(51), "Drive", DeviceType::Saturator);
    sat.set_param("drive_db", 8.5);
    let alc_midi = dir.join("BassRiff.alc");
    save_live_clip(
        &LiveClipFile {
            clip: midi_clip,
            devices: vec![op, sat],
        },
        &alc_midi,
        None,
    )
    .expect("save midi alc");

    let mut session = Session::new(Project::new_default("AlcRules"), None);
    // Track 1 has default Operator; Track 2 has NO clips and NO devices!
    assert!(!session.project().tracks[0].device_chain.is_empty());
    assert!(session.project().tracks[1].device_chain.is_empty());

    // 1. Empty track (no clips, no devices) -> restores clip AND devices.
    session
        .import_live_clip(Some(TrackId(2)), Some(0), None, &alc_midi)
        .expect("import onto empty track");
    let t2 = session.project().track(TrackId(2)).expect("t2");
    assert!(t2.clip_slots[0].clip.is_some());
    assert_eq!(t2.device_chain.len(), 2);
    assert!((t2.device_chain[0].get_param("filter_cutoff") - 1320.0).abs() < 1e-3);

    assert!(session.undo());
    let t2_undone = session.project().track(TrackId(2)).expect("t2");
    assert!(t2_undone.clip_slots[0].clip.is_none());
    assert!(t2_undone.device_chain.is_empty());

    // 2. Non-empty track (already has Operator device) -> restores clip WITHOUT .alc devices.
    let devs_before = session.project().tracks[0].device_chain.len();
    session
        .import_live_clip(Some(TrackId(1)), Some(0), None, &alc_midi)
        .expect("import onto track with device");
    let t1 = session.project().track(TrackId(1)).expect("t1");
    assert!(t1.clip_slots[0].clip.is_some());
    assert_eq!(t1.device_chain.len(), devs_before);

    // 3. Track with no devices, AFTER adding a clip (or track 2 with a clip) -> also does NOT restore devices.
    session
        .execute(CreateMidiTrack::new("ClipOnly"))
        .expect("create empty midi track");
    let t_clip_only_id = session.project().tracks.last().expect("new track").id;
    session
        .import_live_clip(Some(t_clip_only_id), Some(0), None, &alc_midi)
        .expect("import onto empty track first time");
    assert_eq!(
        session
            .project()
            .track(t_clip_only_id)
            .expect("t")
            .device_chain
            .len(),
        2
    );
    // Remove devices from that track so ONLY the clip remains, then import second .alc -> must NOT add devices!
    let dev_ids: Vec<DeviceId> = session
        .project()
        .track(t_clip_only_id)
        .expect("t")
        .device_chain
        .iter()
        .map(|d| d.id)
        .collect();
    for did in dev_ids {
        session
            .execute(RemoveDevice::new(t_clip_only_id, did))
            .expect("remove dev");
    }
    session
        .import_live_clip(Some(t_clip_only_id), Some(1), None, &alc_midi)
        .expect("import onto track that already has a clip");
    assert!(
        session
            .project()
            .track(t_clip_only_id)
            .expect("t")
            .device_chain
            .is_empty(),
        "manual 5.3: track that already has clips must receive clip without devices"
    );

    // 4. Empty Set space (`track_id = None`) -> creates new track + restores devices + clip; undo removes all.
    let track_count_before = session.project().tracks.len();
    session
        .import_live_clip(None, None, None, &alc_midi)
        .expect("import onto empty space");
    assert_eq!(session.project().tracks.len(), track_count_before + 1);
    let created_track = session.project().tracks.last().expect("created");
    assert!(created_track.clip_slots[0].clip.is_some());
    assert_eq!(created_track.device_chain.len(), 2);
    assert!(session.undo());
    assert_eq!(session.project().tracks.len(), track_count_before);

    // Rejects mismatched track type (MIDI .alc onto Audio track) and ambiguous placement.
    assert!(matches!(
        session.import_live_clip(Some(TrackId(3)), Some(0), None, &alc_midi),
        Err(SessionFormatError::TrackTypeMismatch { .. })
    ));
    assert!(matches!(
        session.import_live_clip(Some(TrackId(2)), Some(0), Some(4.0), &alc_midi),
        Err(SessionFormatError::InvalidPlacement(_))
    ));

    // 5. export_live_clip under Always / Ask / Never.
    let ext_wav = dir.join("External/vox.wav");
    write_pcm_wav(&ext_wav, 44_100, 1024, 10_000);
    let mut audio_proj = Project::new_default("ExportAlc");
    let vox_clip = Clip::new_audio(ClipId(801), "Vox", ext_wav.to_string_lossy(), 4.0);
    audio_proj.tracks[2].clip_slots[0].clip = Some(vox_clip);
    let mut exp_session = Session::new(audio_proj, None);

    // Always
    let always_dir = dir.join("ExportAlways");
    let always_alc = always_dir.join("VoxAlways.alc");
    exp_session
        .set_library_preferences(LibraryPreferences {
            collect_files_on_export: CollectFilesOnExport::Always,
            ..LibraryPreferences::default()
        })
        .expect("set always");
    let out_always = exp_session
        .export_live_clip(TrackId(3), ClipId(801), &always_alc, None)
        .expect("export always");
    assert_eq!(
        out_always,
        ExportClipOutcome::Saved {
            copied_samples: true
        }
    );
    assert!(always_dir.join("Samples/Imported/vox.wav").is_file());

    // Ask
    let ask_dir = dir.join("ExportAsk");
    let ask_alc = ask_dir.join("VoxAsk.alc");
    exp_session
        .set_library_preferences(LibraryPreferences {
            collect_files_on_export: CollectFilesOnExport::Ask,
            ..LibraryPreferences::default()
        })
        .expect("set ask");
    assert_eq!(
        exp_session
            .export_live_clip(TrackId(3), ClipId(801), &ask_alc, None)
            .expect("ask without choice"),
        ExportClipOutcome::NeedsCopyConfirmation
    );
    assert!(!ask_alc.exists());

    assert_eq!(
        exp_session
            .export_live_clip(TrackId(3), ClipId(801), &ask_alc, Some(false))
            .expect("ask false"),
        ExportClipOutcome::Saved {
            copied_samples: false
        }
    );
    assert!(ask_alc.is_file());
    assert!(!ask_dir.join("Samples/Imported/vox.wav").exists());

    assert_eq!(
        exp_session
            .export_live_clip(TrackId(3), ClipId(801), &ask_alc, Some(true))
            .expect("ask true"),
        ExportClipOutcome::Saved {
            copied_samples: true
        }
    );
    assert!(ask_dir.join("Samples/Imported/vox.wav").is_file());

    // Never (even if Some(true) is passed)
    let never_dir = dir.join("ExportNever");
    let never_alc = never_dir.join("VoxNever.alc");
    exp_session
        .set_library_preferences(LibraryPreferences {
            collect_files_on_export: CollectFilesOnExport::Never,
            ..LibraryPreferences::default()
        })
        .expect("set never");
    assert_eq!(
        exp_session
            .export_live_clip(TrackId(3), ClipId(801), &never_alc, Some(true))
            .expect("export never"),
        ExportClipOutcome::Saved {
            copied_samples: false
        }
    );
    assert!(never_alc.is_file());
    assert!(!never_dir.join("Samples/Imported/vox.wav").exists());
    let loaded_never = load_live_clip(&never_alc).expect("load never alc");
    assert_eq!(loaded_never.clip.name, "Vox");
}

#[test]
fn device_and_rack_presets_save_load_with_undo_and_pack_install_and_health_report() {
    let dir = test_dir("presets_pack_health");

    // 1. Device (.adv) and Rack (.adg) preset save/load + undo/redo.
    let mut proj = Project::new_default("Presets");
    let mut eq = Device::new(DeviceId(700), "VocalEQ", DeviceType::EqEight);
    eq.set_param("band_1_freq", 110.0);
    eq.set_param("band_1_gain", -4.5);
    let mut simpler = Device::new(DeviceId(701), "SnareShot", DeviceType::Simpler);
    simpler.sample_refs.push(DeviceSampleRef {
        path: "C:/Samples/snare.wav".to_owned(),
        relative_path: Some("Samples/Imported/snare.wav".to_owned()),
    });
    proj.tracks[1].device_chain.push(eq);
    proj.tracks[1].device_chain.push(simpler);

    let mut session = Session::new(proj, None);
    let adv_path = dir.join("VocalEQ.adv");
    let adg_path = dir.join("VocalRack.adg");
    session
        .save_device_preset(TrackId(2), DeviceId(700), &adv_path)
        .expect("save adv");
    session
        .save_rack_preset(TrackId(2), "Vocal Rack", [0.25; 8], &adg_path)
        .expect("save adg");

    let loaded_dev_id = session
        .load_device_preset(TrackId(1), &adv_path)
        .expect("load adv");
    assert_ne!(loaded_dev_id, DeviceId(700));
    let t1_last = session
        .project()
        .track(TrackId(1))
        .expect("t1")
        .device_chain
        .last()
        .expect("loaded dev");
    assert_eq!(t1_last.id, loaded_dev_id);
    assert!((t1_last.get_param("band_1_gain") - (-4.5)).abs() < 1e-3);
    assert!(session.undo());
    assert_eq!(
        session
            .project()
            .track(TrackId(1))
            .expect("t1")
            .device_chain
            .len(),
        1
    );
    assert!(session.redo().expect("redo adv"));
    assert_eq!(
        session
            .project()
            .track(TrackId(1))
            .expect("t1")
            .device_chain
            .last()
            .expect("redone dev")
            .id,
        loaded_dev_id
    );

    let loaded_rack_ids = session
        .load_rack_preset(TrackId(4), &adg_path)
        .expect("load adg");
    assert_eq!(loaded_rack_ids.len(), 2);
    assert_eq!(
        session
            .project()
            .track(TrackId(4))
            .expect("t4")
            .device_chain
            .len(),
        2
    );
    assert_eq!(
        session
            .project()
            .track(TrackId(4))
            .expect("t4")
            .device_chain[1]
            .sample_refs
            .len(),
        1
    );
    assert!(session.undo());
    assert!(
        session
            .project()
            .track(TrackId(4))
            .expect("t4")
            .device_chain
            .is_empty()
    );

    // 2. Pack (.alp) install + zip-slip rejection.
    let user_lib = dir.join("UserLibrary");
    session
        .set_library_preferences(LibraryPreferences {
            user_library: Some(user_lib.clone()),
            ..LibraryPreferences::default()
        })
        .expect("set user lib");

    let valid_alp = dir.join("DemoPack.alp");
    write_store_zip(
        &valid_alp,
        &[
            ("DemoPack/Sets/Demo.als", b"als-bytes"),
            ("DemoPack/Samples/kick.wav", b"RIFFdemo"),
        ],
    );
    let pack_rep = session
        .install_pack(&valid_alp, None)
        .expect("install valid pack into user library");
    assert_eq!(pack_rep.extracted_files.len(), 2);
    assert_eq!(
        pack_rep.sets_found,
        vec![user_lib.join("DemoPack/Sets/Demo.als")]
    );
    assert!(user_lib.join("DemoPack/Samples/kick.wav").is_file());

    let evil_alp = dir.join("Evil.alp");
    let evil_dest = dir.join("EvilDest");
    write_store_zip(&evil_alp, &[("../../evil.txt", b"pwn")]);
    let evil_err = session
        .install_pack(&evil_alp, Some(&evil_dest))
        .expect_err("zip-slip pack must be rejected");
    assert!(matches!(
        evil_err,
        SessionFormatError::Pack(PackError::PathTraversal(_))
    ));
    assert!(!dir.join("evil.txt").exists());
    assert!(!evil_dest.exists());

    // 3. B11 Health Report: reports all 5 diagnostic categories in one pass.
    let health_root = dir.join("Health Project");
    create_project_layout(&health_root).expect("health layout");
    let wav_48k = health_root.join("Samples/Recorded/take_48k.wav");
    let unused_wav = health_root.join("Samples/Imported/unused.wav");
    let external_wav = dir.join("Outside/ext_44k.wav");
    write_pcm_wav(&wav_48k, 48_000, 1024, 5_000);
    write_pcm_wav(&unused_wav, 44_100, 1024, 5_000);
    write_pcm_wav(&external_wav, 44_100, 1024, 5_000);

    let mut health_proj = make_audio_set(
        "HealthDemo",
        &[
            (
                "Samples/Recorded/take_48k.wav",
                Some("Samples/Recorded/take_48k.wav"),
            ),
            (&external_wav.to_string_lossy(), None),
            ("C:/missing/gone.wav", None),
        ],
    );
    health_proj.tracks[0].device_chain.push(Device::new(
        DeviceId(999),
        "MissingPlugin",
        DeviceType::Custom("AuPluginDevice".to_owned()),
    ));
    let health_als = health_root.join("HealthDemo.als");
    AlsDocument::from_project(&health_proj)
        .expect("health doc")
        .save_als(&health_als)
        .expect("save health als");

    let health_session = Session::open_als(&health_als).expect("open health als");
    let report = health_session
        .health_report(Some(44_100))
        .expect("run health report");
    assert!(!report.is_healthy());
    assert_eq!(report.missing_files.len(), 1);
    assert_eq!(report.unused_files.len(), 1);
    assert_eq!(report.missing_plugins.len(), 1);
    assert_eq!(report.sample_rate_mismatches.len(), 1);
    assert_eq!(report.external_files.len(), 1);
    assert_eq!(report.issue_count(), 5);
    let formatted = report.format_health_report();
    assert!(formatted.contains("ISSUES FOUND"));

    // A clean self-contained Set reports healthy.
    let clean_root = dir.join("Clean Project");
    create_project_layout(&clean_root).expect("clean layout");
    let clean_wav = clean_root.join("Samples/Imported/clean.wav");
    write_pcm_wav(&clean_wav, 44_100, 1024, 5_000);
    let clean_proj = make_audio_set(
        "CleanDemo",
        &[(
            "Samples/Imported/clean.wav",
            Some("Samples/Imported/clean.wav"),
        )],
    );
    let clean_als = clean_root.join("CleanDemo.als");
    AlsDocument::from_project(&clean_proj)
        .expect("clean doc")
        .save_als(&clean_als)
        .expect("save clean als");
    let clean_session = Session::open_als(&clean_als).expect("open clean als");
    let clean_report = clean_session
        .health_report(Some(44_100))
        .expect("clean health report");
    assert!(clean_report.is_healthy());
    assert_eq!(clean_report.issue_count(), 0);
}
