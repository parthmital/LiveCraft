//! Integration tests for M10 Live Project management (manual 5.5–5.8) and B11 Health Report.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use livecraft_formats::als::map::AlsDocument;
use livecraft_formats::audio_encode::{AudioFileFormat, BitDepth, write_pcm_file};
use livecraft_formats::health::*;
use livecraft_formats::project_dir::*;
use livecraft_model::ids::ClipId;
use livecraft_model::model::{ArrangementClip, Clip, Device, DeviceType, Project, TrackType};
use serde::ser::{self, Serialize};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(label: &str) -> Self {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("livecraft_m10_{label}_{}_{id}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp test dir");
        Self { path }
    }
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_tone(path: &Path, fmt: AudioFileFormat, sample_rate: u32) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    let samples = vec![0.25_f32, -0.25, 0.125, -0.125];
    write_pcm_file(path, fmt, BitDepth::Bits16, sample_rate, 1, &samples).expect("write audio");
}

fn write_xml_doc(path: &Path, root_tag: &str, file_refs: &[(&str, Option<&str>)]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Ableton><{root_tag}>\n");
    for (abs_path, rel_path) in file_refs {
        xml.push_str("  <SampleRef><FileRef>");
        let _ = write!(xml, "<Path Value=\"{abs_path}\"/>");
        if let Some(rel) = rel_path {
            let _ = write!(xml, "<RelativePath Value=\"{rel}\"/>");
        }
        xml.push_str("</FileRef></SampleRef>\n");
    }
    let _ = writeln!(xml, "</{root_tag}></Ableton>");
    fs::write(path, xml).expect("write xml doc");
}

fn make_audio_project(name: &str, clip_refs: &[&str]) -> Project {
    let mut project = Project::new_default(name);
    let track = project
        .tracks
        .iter_mut()
        .find(|t| t.track_type == TrackType::Audio)
        .expect("default audio track");
    for (i, path) in clip_refs.iter().enumerate() {
        track.clip_slots[i].clip = Some(Clip::new_audio(
            ClipId(500 + i as u64),
            format!("Clip{i}"),
            *path,
            4.0,
        ));
    }
    project
}

// Manual 5.5, p. 142:
// "When you save a Live Set under a new name or in a new folder location, Live will create a new
// project folder and store the Live Set there — unless you are saving the Live Set into an
// existing Live Project."
// Manual 5.5.1, p. 143:
// "The project folder (“Tango Project“) contains the Live Set (“Tango.als“) and a Samples folder,
// which in turn contains a Recorded folder with two samples in it."
// Manual 5.5.1, p. 144:
// "The Tango Project now contains two Live Sets, and its Samples/Recorded folder contains the
// samples used by both of them."
// Manual 5.5.1, p. 146:
// "There is actually no need to keep a Project’s Live Set exactly one level below the Project
// itself. Within a project folder, you can create any number of sub-folders and move files
// around to organize them as desired, although you may need to use the File Manager to “teach“
// the Project about the changes you’ve made."
#[test]
fn manual_5_5_project_detection_creation_and_preservation() {
    let tmp = TempDir::new("m55");
    let workspace = tmp.path().join("Music");
    fs::create_dir_all(&workspace).expect("mkdir");
    assert!(!is_project_root(&workspace));

    let tango_doc = AlsDocument::from_project(&Project::new_default("Tango")).expect("doc");
    let tango_root = workspace.join("Tango Project");
    let saved_tango =
        save_set_into_project(&tango_doc, &workspace.join("Tango.als")).expect("save Tango");
    assert!(saved_tango.created_new_project && is_project_root(&tango_root));
    assert_eq!(saved_tango.project_dir.root, tango_root);
    assert_eq!(saved_tango.set_path, tango_root.join("Tango.als"));
    assert!(tango_root.join(PROJECT_INFO_DIR).is_dir() && saved_tango.set_path.is_file());
    for sub in [
        SAMPLES_RECORDED_DIR,
        SAMPLES_IMPORTED_DIR,
        SAMPLES_PROCESSED_DIR,
    ] {
        assert!(tango_root.join(SAMPLES_DIR).join(sub).is_dir());
    }

    // Preserve pre-existing Ableton Project Info private files; never generate Live private files.
    let info_count = fs::read_dir(tango_root.join(PROJECT_INFO_DIR))
        .expect("read info dir")
        .count();
    assert_eq!(
        info_count, 0,
        "LiveCraft must never generate Live's private Project Info files"
    );
    let private_cfg = tango_root.join(PROJECT_INFO_DIR).join("Project8_1.cfg");
    fs::write(&private_cfg, b"private-live-metadata").expect("write cfg");

    // Saving a second Set ("Samba.als") inside Tango Project reuses the existing Project folder.
    let samba_doc = AlsDocument::from_project(&Project::new_default("Samba")).expect("doc");
    let saved_samba =
        save_set_into_project(&samba_doc, &tango_root.join("Samba.als")).expect("save Samba");
    assert!(!saved_samba.created_new_project && saved_samba.project_dir.root == tango_root);
    assert!(saved_tango.set_path.is_file() && saved_samba.set_path.is_file());
    assert_eq!(
        fs::read(&private_cfg).expect("read private cfg"),
        b"private-live-metadata"
    );

    // Subfolders inside a Live Project still belong to the enclosing Project (5.5.1 p. 146).
    let deep_target = tango_root.join("AltVersions/Club/Remix.als");
    let saved_remix = save_set_into_project(&samba_doc, &deep_target).expect("save nested");
    assert!(!saved_remix.created_new_project);
    assert_eq!(detect_project_root(&deep_target), Some(tango_root.clone()));
    assert!(AlsDocument::open(&saved_remix.set_path).is_ok());

    // Direct create_project_layout / ProjectDir::open check.
    let manual_root = tmp.path().join("Direct Project");
    let pdir = create_project_layout(&manual_root).expect("layout");
    assert_eq!(ProjectDir::open(&manual_root).expect("open"), pdir);
    assert_eq!(
        pdir.samples_imported_dir(),
        manual_root.join(SAMPLES_DIR).join(SAMPLES_IMPORTED_DIR)
    );
}

#[test]
fn lc006_write_policy_and_path_validation() {
    let tmp = TempDir::new("lc006");
    let doc = AlsDocument::from_project(&Project::new_default("Safe")).expect("doc");
    assert!(matches!(
        ProjectDir::open(tmp.path()),
        Err(ProjectDirError::NotAProject(_))
    ));
    assert!(matches!(
        save_set_into_project(&doc, tmp.path()),
        Err(ProjectDirError::InvalidPath(_))
    ));
    assert!(matches!(
        save_set_into_project(&doc, &tmp.path().join("..").join("Escape.als")),
        Err(ProjectDirError::InvalidPath(_))
    ));
    assert!(matches!(
        save_set_into_project(&doc, &tmp.path().join(".als")),
        Err(ProjectDirError::InvalidProjectName(_))
    ));
}

// Manual 5.6, p. 147:
// "If you load a Live Set, Live Clip or preset that references files which are missing from their
// referenced locations, Live’s Status Bar (located at the bottom of the main screen) will
// display a warning message."
// Manual 5.6.1, p. 148:
// "Note that Live will not care if the file you offer is really the file that was missing."
// Manual 5.6.2, p. 149:
// "Search Folder — includes a user-defined folder, as well as any sub-folders, in the search."
// "Search Project — includes this Set’s project folder in the search."
// "Search Library — includes the Live Library in the search."
// "No candidate found — you can choose another folder and try again, or locate the sample manually."
// "One candidate found — Live accepts the candidate and considers the problem solved."
// "Several candidates found — Live requires your assistance"
#[test]
fn manual_5_6_missing_detection_relative_fallback_and_manual_repair() {
    let tmp = TempDir::new("m56_rel");
    let root = tmp.path().join("SetA Project");
    create_project_layout(&root).expect("layout");

    let kick = root.join("Samples/Recorded/kick.wav");
    let snare = root.join("Samples/Recorded/snare.wav");
    let replacement = root.join("Samples/Imported/completely_different_clap.wav");
    write_tone(&kick, AudioFileFormat::Wav, 44100);
    write_tone(&snare, AudioFileFormat::Wav, 44100);
    write_tone(&replacement, AudioFileFormat::Wav, 44100);

    let ctx = ProjectContext::for_project(&root);
    let rel_ref = SampleReference {
        path: tmp
            .path()
            .join("Gone/snare.wav")
            .to_string_lossy()
            .into_owned(),
        relative_path: Some("Samples/Recorded/snare.wav".into()),
    };
    assert_eq!(
        resolve_sample_reference(&rel_ref, &ctx),
        SampleResolution::Resolved {
            resolved_path: snare,
            via_relative_path: true,
            location: SampleLocation::InProject,
        }
    );

    let missing_ref = "Samples/Recorded/does_not_exist.wav";
    let mut project = make_audio_project("SetA", &[&kick.to_string_lossy(), missing_ref]);
    project.tracks[3]
        .arrangement_clips
        .push(ArrangementClip::new(
            ClipId(901),
            Clip::new_audio(ClipId(901), "ArrMissing", missing_ref, 4.0),
            0.0,
            4.0,
        ));
    let missing = detect_missing_files(&project, &ctx);
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].file_name, "does_not_exist.wav");
    assert_eq!(missing[0].usages.len(), 2);

    // Manual 5.6.1: Live does not care if the manually supplied replacement has a different name.
    let updated =
        replace_sample_reference(&mut project, missing_ref, &replacement.to_string_lossy());
    assert_eq!(updated, 2);
    assert!(detect_missing_files(&project, &ctx).is_empty());

    // Document-level XML FileRef outside AudioClip is also checked.
    let xml = format!(
        "<?xml version=\"1.0\"?><Ableton><LiveSet><Tracks/><MasterTrack><DeviceChain><Devices>\
         <OriginalSimpler><SampleRef><FileRef><Path Value=\"{}\"/></FileRef></SampleRef>\
         </OriginalSimpler></Devices></DeviceChain></MasterTrack></LiveSet></Ableton>",
        tmp.path().join("MissingSimpler.wav").display()
    );
    let doc = AlsDocument::from_xml_named(&xml, "SimplerSet").expect("xml");
    let doc_missing = detect_missing_files_in_document(&doc, &ctx);
    assert_eq!(doc_missing.len(), 1);
    assert!(matches!(
        doc_missing[0].usages.as_slice(),
        [ClipLocationRef::XmlFileRef(tag)] if tag == "OriginalSimpler"
    ));
}

#[test]
fn manual_5_6_2_automatic_search_outcomes_and_repair() {
    let tmp = TempDir::new("m56_auto");
    let root = tmp.path().join("Search Project");
    create_project_layout(&root).expect("layout");
    let folder = tmp.path().join("ExternalSamples");
    let library = tmp.path().join("UserLibrary");

    let in_folder = folder.join("Drums/Deep/rimshot.wav");
    let in_project = root.join("Samples/Processed/vox.wav");
    let in_library = library.join("Keys/rhodes.wav");
    let dup_a = folder.join("KitA/bass.wav");
    let dup_b = folder.join("KitB/bass.wav");
    for p in [&in_folder, &in_project, &in_library, &dup_a, &dup_b] {
        write_tone(p, AudioFileFormat::Wav, 44100);
    }

    let mut project = make_audio_project(
        "AutoRepair",
        &[
            "C:/OldDisk/rimshot.wav",
            "C:/OldDisk/vox.wav",
            "C:/OldDisk/rhodes.wav",
            "C:/OldDisk/bass.wav",
            "C:/OldDisk/unfindable.wav",
        ],
    );
    let mut doc = AlsDocument::from_project(&project).expect("doc");
    let ctx = ProjectContext {
        project_root: Some(root.clone()),
        set_dir: Some(root.clone()),
        user_library: Some(library),
        factory_packs_roots: Vec::new(),
    };

    // With all search scopes off, nothing matches.
    let missing_before = detect_missing_files(&project, &ctx);
    assert_eq!(missing_before.len(), 5);
    let off_outcomes = automatic_search(&missing_before, &AutomaticSearchOptions::default(), &ctx)
        .expect("search off");
    assert!(
        off_outcomes
            .iter()
            .all(|(_, o)| *o == SearchCandidateOutcome::NoCandidate)
    );

    // Enable Search Folder + Search Project + Search Library.
    let opts = AutomaticSearchOptions {
        search_folder: Some(folder),
        search_project: true,
        search_library: true,
        library_roots: Vec::new(),
    };
    let report =
        repair_missing_files_auto(&mut project, Some(&mut doc), &ctx, &opts).expect("auto repair");
    assert_eq!(report.repaired.len(), 3);
    assert_eq!(report.unresolved.len(), 2);

    for (item, outcome) in &report.unresolved {
        match item.file_name.as_str() {
            "unfindable.wav" => assert_eq!(*outcome, SearchCandidateOutcome::NoCandidate),
            "bass.wav" => match outcome {
                SearchCandidateOutcome::MultipleCandidates(paths) => assert_eq!(paths.len(), 2),
                other => panic!("expected MultipleCandidates for bass.wav, got {other:?}"),
            },
            other => panic!("unexpected unresolved file {other}"),
        }
    }

    // Re-checking synced document sees only bass.wav and unfindable.wav still missing.
    let remaining = detect_missing_files_in_document(&doc, &ctx);
    let mut names: Vec<_> = remaining.iter().map(|m| m.file_name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["bass.wav", "unfindable.wav"]);
}

// Manual 5.7, p. 150, 152:
// "To prevent broken file references, you can collect (i.e., copy) all external files used in a
// Set into its Project folder."
// "The files are then added to their corresponding subfolder in the Project’s Samples folder (for
// audio files) or Presets folder (for devices)."
// "Live considers any files that are outside the Project folder as external — this includes files
// from Packs, other Projects, the User Library, and files located elsewhere on your computer."
// Manual 5.7.1, p. 152–153:
// "Always — Files are automatically copied into the destination folder without notification. This
// is the default setting."
// "Ask — A dialog appears once a clip, preset, or track is moved, where you can choose whether its
// dependent files are copied."
// "Never — Files are not copied when moved and the original file references are used."
#[test]
fn manual_5_7_collect_all_and_save_toggles_copy_never_move_and_dedup() {
    let tmp = TempDir::new("m57");
    let root = tmp.path().join("Target Project");
    create_project_layout(&root).expect("layout");

    let other_proj = tmp.path().join("Other Project");
    create_project_layout(&other_proj).expect("other layout");
    let user_lib = tmp.path().join("User Library");
    let packs = tmp.path().join("Factory Packs");

    let ext_elsewhere = tmp.path().join("Downloads/lead.wav");
    let ext_other_proj = other_proj.join("Samples/Recorded/pad.wav");
    let ext_user_lib = user_lib.join("Samples/hats.wav");
    let ext_pack = packs.join("Core/perc.wav");
    let ext_elsewhere_alt = tmp.path().join("Alt/lead.wav");
    for p in [&ext_elsewhere, &ext_other_proj, &ext_user_lib, &ext_pack] {
        write_tone(p, AudioFileFormat::Wav, 44100);
    }
    // Same filename `lead.wav` with different bytes to verify non-destructive disambiguation.
    fs::create_dir_all(ext_elsewhere_alt.parent().expect("parent")).expect("mkdir");
    write_pcm_file(
        &ext_elsewhere_alt,
        AudioFileFormat::Wav,
        BitDepth::Bits16,
        44100,
        1,
        &[0.9, -0.9, 0.5, -0.5],
    )
    .expect("write alt lead");

    let mut project = make_audio_project(
        "CollectDemo",
        &[
            &ext_elsewhere.to_string_lossy(),
            &ext_elsewhere.to_string_lossy(),
            &ext_other_proj.to_string_lossy(),
            &ext_user_lib.to_string_lossy(),
            &ext_pack.to_string_lossy(),
            &ext_elsewhere_alt.to_string_lossy(),
        ],
    );
    let mut doc = AlsDocument::from_project(&project).expect("doc");
    let ctx = ProjectContext {
        project_root: Some(root.clone()),
        set_dir: Some(root.clone()),
        user_library: Some(user_lib),
        factory_packs_roots: vec![packs],
    };

    assert_eq!(
        classify_sample_location(&ext_elsewhere, &ctx),
        SampleLocation::Elsewhere
    );
    assert_eq!(
        classify_sample_location(&ext_other_proj, &ctx),
        SampleLocation::OtherProject(other_proj.clone())
    );
    assert_eq!(
        classify_sample_location(&ext_user_lib, &ctx),
        SampleLocation::UserLibrary
    );
    assert_eq!(
        classify_sample_location(&ext_pack, &ctx),
        SampleLocation::FactoryPacks
    );

    // Pass 1: default toggles collect Elsewhere + Other Projects, skipping User Library & Packs.
    let rep1 = collect_all_and_save(
        &mut project,
        Some(&mut doc),
        &ctx,
        &CollectAllOptions::default(),
    )
    .expect("collect default");
    assert_eq!(rep1.collected.len(), 3); // lead.wav (once for 2 clips), pad.wav, lead_1.wav
    assert_eq!(rep1.updated_references, 4);
    assert_eq!(rep1.skipped_by_policy.len(), 2);

    // Originals are NEVER moved or deleted (LC-006).
    assert!(ext_elsewhere.is_file());
    assert!(ext_other_proj.is_file());
    assert!(root.join("Samples/Imported/lead.wav").is_file());
    assert!(root.join("Samples/Imported/lead_1.wav").is_file());
    assert!(root.join("Samples/Imported/pad.wav").is_file());

    // Pass 2: CollectAllOptions::all() copies User Library & Factory Packs without touching InProject files.
    let rep2 = collect_all_and_save(
        &mut project,
        Some(&mut doc),
        &ctx,
        &CollectAllOptions::all(),
    )
    .expect("collect all");
    assert_eq!(rep2.collected.len(), 2);
    assert_eq!(rep2.updated_references, 2);
    assert!(rep2.skipped_by_policy.is_empty());

    // Pre-existing identical file in Samples/Imported is reused cleanly (`reused_existing == true`).
    let dup_ext = tmp.path().join("External/reusable.wav");
    write_tone(&dup_ext, AudioFileFormat::Wav, 44100);
    fs::copy(&dup_ext, root.join("Samples/Imported/reusable.wav")).expect("seed dup");
    let mut dup_proj = make_audio_project("Reuse", &[&dup_ext.to_string_lossy()]);
    let rep3 =
        collect_all_and_save(&mut dup_proj, None, &ctx, &CollectAllOptions::all()).expect("reuse");
    assert_eq!(rep3.collected.len(), 1);
    assert!(rep3.collected[0].reused_existing);
    assert!(!root.join("Samples/Imported/reusable_1.wav").exists());

    // Export policy enum (manual 5.7.1, p. 152).
    assert_eq!(
        CollectFilesOnExport::default(),
        CollectFilesOnExport::Always
    );
    assert_eq!(CollectFilesOnExport::Always.should_copy(None), Some(true));
    assert_eq!(
        CollectFilesOnExport::Never.should_copy(Some(true)),
        Some(false)
    );
    assert_eq!(CollectFilesOnExport::Ask.should_copy(None), None);
    assert_eq!(
        CollectFilesOnExport::Ask.should_copy(Some(true)),
        Some(true)
    );
    assert_eq!(
        CollectFilesOnExport::Ask.should_copy(Some(false)),
        Some(false)
    );
}

// Manual 5.8, p. 156:
// "Live considers any file in the Project which is not referenced by its Sets, clips, or presets
// as unused — even if the file is actively used in other Projects."
#[test]
fn manual_5_8_unused_files_scans_als_alc_adv_adg_and_ignores_other_projects() {
    let tmp = TempDir::new("m58");
    let root = tmp.path().join("ProjA Project");
    create_project_layout(&root).expect("layout A");

    let expected_unused = [
        ("Extra/orphan_other.wav", UnusedFileCategory::Other),
        ("Samples/Freeze/orphan_frz.wav", UnusedFileCategory::Freeze),
        (
            "Samples/Imported/orphan_imp.wav",
            UnusedFileCategory::Imported,
        ),
        (
            "Samples/Processed/orphan_proc.wav",
            UnusedFileCategory::Processed,
        ),
        (
            "Samples/Recorded/orphan_rec.wav",
            UnusedFileCategory::Recorded,
        ),
    ];
    let used_adv = root.join("Samples/Processed/used_adv.wav");
    for rel in [
        "Samples/Recorded/used_als.wav",
        "Samples/Imported/used_alc.wav",
        "Samples/Processed/used_adv.wav",
        "Samples/Freeze/used_adg.wav",
        "Backup/old_take.wav",
    ] {
        write_tone(&root.join(rel), AudioFileFormat::Wav, 44100);
    }
    for (rel, _) in expected_unused {
        write_tone(&root.join(rel), AudioFileFormat::Wav, 44100);
    }

    // Main Set (.als gzip) + .alc + .adv + .adg documents.
    let als_proj = make_audio_project("Main", &["Samples/Recorded/used_als.wav"]);
    AlsDocument::from_project(&als_proj)
        .expect("doc")
        .save_als(root.join("Main.als"))
        .expect("save als");
    write_xml_doc(
        &root.join("Clips/Beat.alc"),
        "LiveClip",
        &[(
            "C:/Missing/used_alc.wav",
            Some("../Samples/Imported/used_alc.wav"),
        )],
    );
    write_xml_doc(
        &root.join("Presets/Synth.adv"),
        "Simpler",
        &[(&used_adv.to_string_lossy(), None)],
    );
    write_xml_doc(
        &root.join("Racks/Kit.adg"),
        "DrumGroupDevice",
        &[("", Some("Samples/Freeze/used_adg.wav"))],
    );

    // Separate Project B actively uses ProjA's `orphan_rec.wav` — ProjA must still call it unused!
    let proj_b = tmp.path().join("ProjB Project");
    create_project_layout(&proj_b).expect("layout B");
    let orphan_in_a = root.join("Samples/Recorded/orphan_rec.wav");
    let b_proj = make_audio_project("Other", &[&orphan_in_a.to_string_lossy()]);
    AlsDocument::from_project(&b_proj)
        .expect("doc b")
        .save_als(proj_b.join("Other.als"))
        .expect("save als b");

    let report = find_unused_files(&root).expect("find unused");
    assert_eq!(report.scanned_documents.len(), 4);
    let got: Vec<_> = report
        .unused_files
        .iter()
        .map(|u| (u.relative_path.as_str(), u.category))
        .collect();
    assert_eq!(got, expected_unused);
}

#[test]
fn b11_health_report_probes_wav_aiff_flac_mp3_and_formats_json() {
    let tmp = TempDir::new("b11");
    let root = tmp.path().join("Health Project");
    create_project_layout(&root).expect("layout");

    // Probe checks: WAV (hound), AIFF, FLAC, and minimal synthetic MP3 header at 48 kHz.
    let wav44 = root.join("Samples/Recorded/good_44k.wav");
    let wav48 = root.join("Samples/Imported/hi_48k.wav");
    let aiff48 = root.join("Samples/Imported/hi_48k.aif");
    let flac48 = root.join("Samples/Imported/hi_48k.flac");
    let mp3_48 = tmp.path().join("probe_48k.mp3");
    let unused_wav = root.join("Samples/Imported/unused.wav");
    let user_lib = tmp.path().join("UserLib");
    let ext_wav = user_lib.join("external_44k.wav");

    write_tone(&wav44, AudioFileFormat::Wav, 44100);
    write_tone(&wav48, AudioFileFormat::Wav, 48000);
    write_tone(&aiff48, AudioFileFormat::Aiff, 48000);
    write_tone(&flac48, AudioFileFormat::Flac, 48000);
    write_tone(&unused_wav, AudioFileFormat::Wav, 44100);
    write_tone(&ext_wav, AudioFileFormat::Wav, 44100);
    // MPEG-1 Layer III frame header: 0xFF 0xFB, bitrate=9 (128kbps), sr_idx=1 (48000 Hz)
    fs::write(&mp3_48, [0xFF_u8, 0xFB, 0x94, 0x00, 0x00, 0x00]).expect("write mp3");

    assert_eq!(probe_audio_sample_rate(&wav44).expect("wav44"), 44100);
    assert_eq!(probe_audio_sample_rate(&wav48).expect("wav48"), 48000);
    assert_eq!(probe_audio_sample_rate(&aiff48).expect("aiff48"), 48000);
    assert_eq!(probe_audio_sample_rate(&flac48).expect("flac48"), 48000);
    assert_eq!(probe_audio_sample_rate(&mp3_48).expect("mp3_48"), 48000);

    // Healthy standalone project passes all 5 checks.
    let clean_root = tmp.path().join("Pure Project");
    create_project_layout(&clean_root).expect("pure layout");
    let pure_wav = clean_root.join("Samples/Recorded/ok.wav");
    write_tone(&pure_wav, AudioFileFormat::Wav, 44100);
    let pure_proj = make_audio_project("Pure", &["Samples/Recorded/ok.wav"]);
    let pure_set = clean_root.join("Pure.als");
    AlsDocument::from_project(&pure_proj)
        .expect("pure doc")
        .save_als(&pure_set)
        .expect("save pure");
    let pure_rep =
        inspect_set_health(&pure_set, &HealthCheckOptions::default()).expect("pure health");
    assert!(pure_rep.is_healthy() && pure_rep.issue_count() == 0);
    assert!(pure_rep.format_health_report().contains("HEALTHY"));

    // Unhealthy project triggers all 5 B11 diagnostics in one pass.
    let mut bad = make_audio_project(
        "Audit",
        &[
            "Samples/Imported/hi_48k.wav",
            "Samples/Imported/hi_48k.aif",
            "Samples/Imported/hi_48k.flac",
            "Samples/Recorded/missing_ghost.wav",
            &ext_wav.to_string_lossy(),
        ],
    );
    for (id, name, tag) in [
        (901, "MissingSynth", "AuPlugin"),
        (902, "AvailableComp", "Vst3Plugin"),
    ] {
        bad.tracks[0].device_chain.push(Device::new(
            id.into(),
            name,
            DeviceType::Custom(tag.into()),
        ));
    }
    let bad_doc = AlsDocument::from_project(&bad).expect("bad doc");
    let bad_set = root.join("Audit.als");
    bad_doc.save_als(&bad_set).expect("save bad");

    let opts = HealthCheckOptions {
        set_path: Some(bad_set.clone()),
        project_root: Some(root.clone()),
        expected_sample_rate: Some(44100),
        user_library: Some(user_lib),
        factory_packs_roots: Vec::new(),
        available_plugins: Some(vec!["AvailableComp".into()]),
    };
    let report = inspect_set_health(&bad_set, &opts).expect("inspect unhealthy");
    assert_eq!(report, check_document_health(&bad_doc, &opts).expect("doc"));
    assert_eq!(
        report.missing_plugins,
        check_project_health(&bad, &opts)
            .expect("mem")
            .missing_plugins
    );
    assert!(!report.is_healthy() && report.issue_count() == 8);
    assert_eq!(report.missing_files.len(), 1);
    assert_eq!(report.unused_files.len(), 2); // good_44k.wav + unused.wav
    assert_eq!(report.missing_plugins.len(), 1);
    assert_eq!(report.missing_plugins[0].device_name, "MissingSynth");
    assert_eq!(report.sample_rate_mismatches.len(), 3);
    assert_eq!(report.external_files.len(), 1);
    assert_eq!(
        report.external_files[0].location,
        SampleLocation::UserLibrary
    );

    let formatted = report.format_health_report();
    let json = to_json(&report);
    for s in [
        "ISSUES FOUND",
        "Missing files (1):",
        "Unused files (2):",
        "Missing plug-ins (1):",
        "Sample-rate mismatches (3):",
        "External files (1):",
    ] {
        assert!(formatted.contains(s), "report missing `{s}`:\n{formatted}");
    }
    for k in [
        "\"set_name\":\"Audit\"",
        "\"missing_files\":[",
        "\"unused_files\":[",
        "\"missing_plugins\":[",
        "\"sample_rate_mismatches\":[",
        "\"external_files\":[",
    ] {
        assert!(json.contains(k), "JSON missing `{k}`: {json}");
    }
}

// Tiny macro-backed serde::Serializer to verify JSON serialization without extra crates.
fn to_json<T: Serialize>(value: &T) -> String {
    let mut s = MiniJson(String::new());
    value.serialize(&mut s).expect("serialize JSON");
    s.0
}
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
struct JsonErr(String);
impl ser::Error for JsonErr {
    fn custom<T: std::fmt::Display>(m: T) -> Self {
        Self(m.to_string())
    }
}
struct MiniJson(String);
struct JsonCompound<'a>(&'a mut MiniJson, bool, &'static str);
impl MiniJson {
    fn esc(&mut self, v: &str) {
        self.0.push('"');
        for c in v.chars() {
            match c {
                '"' => self.0.push_str("\\\""),
                '\\' => self.0.push_str("\\\\"),
                c => self.0.push(c),
            }
        }
        self.0.push('"');
    }
    fn open(&mut self, pfx: &str, var: Option<&str>, end: &'static str) -> JsonCompound<'_> {
        if let Some(v) = var {
            self.0.push('{');
            self.esc(v);
            self.0.push(':');
        }
        self.0.push_str(pfx);
        JsonCompound(self, true, end)
    }
}
impl JsonCompound<'_> {
    fn push<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), JsonErr> {
        if !self.1 {
            self.0.0.push(',');
        }
        self.1 = false;
        v.serialize(&mut *self.0)
    }
    fn done(self) -> Result<(), JsonErr> {
        self.0.0.push_str(self.2);
        Ok(())
    }
}
macro_rules! jnum { ($($f:ident($t:ty)),*) => { $(fn $f(self, v: $t) -> Result<(), JsonErr> { let _ = write!(self.0, "{v}"); Ok(()) })* }; }
macro_rules! jseq { ($($tr:ident :: $fn:ident),*) => { $(impl ser::$tr for JsonCompound<'_> { type Ok = (); type Error = JsonErr; fn $fn<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), JsonErr> { self.push(v) } fn end(self) -> Result<(), JsonErr> { self.done() } })* }; }
macro_rules! jmap { ($($tr:ident),*) => { $(impl ser::$tr for JsonCompound<'_> { type Ok = (); type Error = JsonErr; fn serialize_field<T: ?Sized + Serialize>(&mut self, k: &'static str, v: &T) -> Result<(), JsonErr> { self.push(k)?; self.0.0.push(':'); v.serialize(&mut *self.0) } fn end(self) -> Result<(), JsonErr> { self.done() } })* }; }
#[rustfmt::skip]
impl<'a> ser::Serializer for &'a mut MiniJson {
    type Ok = (); type Error = JsonErr;
    type SerializeSeq = JsonCompound<'a>; type SerializeTuple = JsonCompound<'a>;
    type SerializeTupleStruct = JsonCompound<'a>; type SerializeTupleVariant = JsonCompound<'a>;
    type SerializeMap = JsonCompound<'a>; type SerializeStruct = JsonCompound<'a>;
    type SerializeStructVariant = JsonCompound<'a>;
    jnum!(serialize_bool(bool), serialize_i8(i8), serialize_i16(i16), serialize_i32(i32), serialize_i64(i64), serialize_u8(u8), serialize_u16(u16), serialize_u32(u32), serialize_u64(u64), serialize_f32(f32), serialize_f64(f64));
    fn serialize_char(self, v: char) -> Result<(), JsonErr> { self.esc(&v.to_string()); Ok(()) }
    fn serialize_str(self, v: &str) -> Result<(), JsonErr> { self.esc(v); Ok(()) }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), JsonErr> { self.collect_seq(v) }
    fn serialize_none(self) -> Result<(), JsonErr> { self.0.push_str("null"); Ok(()) }
    fn serialize_some<T: ?Sized + Serialize>(self, v: &T) -> Result<(), JsonErr> { v.serialize(self) }
    fn serialize_unit(self) -> Result<(), JsonErr> { self.0.push_str("null"); Ok(()) }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), JsonErr> { self.serialize_unit() }
    fn serialize_unit_variant(self, _: &'static str, _: u32, v: &'static str) -> Result<(), JsonErr> { self.serialize_str(v) }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &'static str, v: &T) -> Result<(), JsonErr> { v.serialize(self) }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _: &'static str, _: u32, k: &'static str, v: &T) -> Result<(), JsonErr> {
        self.0.push('{'); self.esc(k); self.0.push(':'); v.serialize(&mut *self)?; self.0.push('}'); Ok(())
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<JsonCompound<'a>, JsonErr> { Ok(self.open("[", None, "]")) }
    fn serialize_tuple(self, n: usize) -> Result<JsonCompound<'a>, JsonErr> { self.serialize_seq(Some(n)) }
    fn serialize_tuple_struct(self, _: &'static str, n: usize) -> Result<JsonCompound<'a>, JsonErr> { self.serialize_seq(Some(n)) }
    fn serialize_tuple_variant(self, _: &'static str, _: u32, v: &'static str, _: usize) -> Result<JsonCompound<'a>, JsonErr> { Ok(self.open("[", Some(v), "]}")) }
    fn serialize_map(self, _: Option<usize>) -> Result<JsonCompound<'a>, JsonErr> { Ok(self.open("{", None, "}")) }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<JsonCompound<'a>, JsonErr> { Ok(self.open("{", None, "}")) }
    fn serialize_struct_variant(self, _: &'static str, _: u32, v: &'static str, _: usize) -> Result<JsonCompound<'a>, JsonErr> { Ok(self.open("{", Some(v), "}}")) }
}
jseq!(
    SerializeSeq::serialize_element,
    SerializeTuple::serialize_element,
    SerializeTupleStruct::serialize_field,
    SerializeTupleVariant::serialize_field
);
jmap!(SerializeStruct, SerializeStructVariant);
impl ser::SerializeMap for JsonCompound<'_> {
    type Ok = ();
    type Error = JsonErr;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, k: &T) -> Result<(), JsonErr> {
        self.push(k)
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, v: &T) -> Result<(), JsonErr> {
        self.0.0.push(':');
        v.serialize(&mut *self.0)
    }
    fn end(self) -> Result<(), JsonErr> {
        self.done()
    }
}
