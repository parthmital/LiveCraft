use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use livecraft_formats::als::{AlsDocument, AlsParser, read::decompress_als_or_xml};
use livecraft_formats::health::ProjectHealthReport;
use livecraft_formats::midi_file::{SmfFormat, read_smf_file, write_smf};
use livecraft_formats::project_dir::{
    CollectReport, SampleLocation, UnusedFilesReport, create_project_layout,
};
use livecraft_model::device::{Device, DeviceType};
use livecraft_model::ids::{ClipId, DeviceId};
use livecraft_model::model::{Clip, ClipPayload, MidiNote, Project};

fn run_livecraft(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_livecraft"))
        .args(args)
        .output()
        .expect("run the livecraft binary")
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("test path is valid UTF-8")
}

fn test_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("m10_cli")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create m10_cli test dir");
    dir
}

fn write_tone_wav(path: &Path, sample_rate: u32, frames: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create wav parent dir");
    }
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    for i in 0..frames {
        let s = ((i as f32) * 440.0 * std::f32::consts::TAU / (sample_rate as f32)).sin() * 0.5;
        writer.write_sample(s).expect("write L");
        writer.write_sample(s).expect("write R");
    }
    writer.finalize().expect("finalize wav");
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

fn write_store_zip(path: &Path, entries: &[(&str, &[u8])]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create zip parent dir");
    }
    let mut data = Vec::new();
    let mut central = Vec::new();
    for &(name, payload) in entries {
        let name_bytes = name.as_bytes();
        let crc = crc32(payload);
        let size = payload.len() as u32;
        let offset = data.len() as u32;

        data.extend_from_slice(b"PK\x03\x04");
        data.extend_from_slice(&20u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(&crc.to_le_bytes());
        data.extend_from_slice(&size.to_le_bytes());
        data.extend_from_slice(&size.to_le_bytes());
        data.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.extend_from_slice(name_bytes);
        data.extend_from_slice(payload);

        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
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

#[test]
fn render_and_stems_resolve_broken_path_via_relative_path_in_project_folder() {
    let dir = test_dir("render_relative");
    let proj_root = dir.join("MovedAudio Project");
    create_project_layout(&proj_root).expect("create project layout");
    let sample_wav = proj_root.join("Samples/Imported/tone.wav");
    write_tone_wav(&sample_wav, 44_100, 44_100);

    let proj = make_audio_set(
        "MovedAudio",
        &[(
            "C:/nonexistent_old_machine_98765/Samples/Imported/tone.wav",
            Some("Samples/Imported/tone.wav"),
        )],
    );
    let als_path = proj_root.join("MovedAudio.als");
    AlsDocument::from_project(&proj)
        .expect("build als doc")
        .save_als(&als_path)
        .expect("save als");

    let xml = decompress_als_or_xml(&als_path).expect("read saved xml");
    assert!(xml.contains("nonexistent_old_machine_98765"));
    assert!(xml.contains("Samples/Imported/tone.wav"));

    let out_wav = dir.join("mix.wav");
    let render_out = run_livecraft(&[
        "render",
        path_str(&als_path),
        "-o",
        path_str(&out_wav),
        "-d",
        "1",
        "-b",
        "32",
        "--no-report",
    ]);
    assert!(
        render_out.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&render_out.stderr)
    );
    let samples: Vec<f32> = hound::WavReader::open(&out_wav)
        .expect("open rendered wav")
        .into_samples::<f32>()
        .map(|s| s.expect("sample"))
        .collect();
    assert!(
        samples.iter().any(|&s| s.abs() > 0.05),
        "rendered mix should contain audio resolved via RelativePath"
    );

    let stems_dir = dir.join("stems");
    let stems_out = run_livecraft(&[
        "stems",
        path_str(&als_path),
        "-o",
        path_str(&stems_dir),
        "-d",
        "1",
        "-b",
        "32",
        "--no-report",
    ]);
    assert!(
        stems_out.status.success(),
        "stems failed: {}",
        String::from_utf8_lossy(&stems_out.stderr)
    );
    let audio_stem = fs::read_dir(&stems_dir)
        .expect("read stems dir")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|n| n.contains("3-Audio") && n.ends_with(".wav"))
        })
        .expect("3-Audio stem file exists");
    let stem_samples: Vec<f32> = hound::WavReader::open(&audio_stem)
        .expect("open stem wav")
        .into_samples::<f32>()
        .map(|s| s.expect("sample"))
        .collect();
    assert!(
        stem_samples.iter().any(|&s| s.abs() > 0.05),
        "3-Audio stem should contain audio resolved via RelativePath"
    );
}

#[test]
fn new_with_project_flag_creates_project_folder_and_preserves_plain_new() {
    let dir = test_dir("new_project");

    // Without --project: writes <path> directly.
    let plain_als = dir.join("PlainSet.als");
    let plain_out = run_livecraft(&["new", path_str(&plain_als), "--bpm", "131"]);
    assert!(
        plain_out.status.success(),
        "{}",
        String::from_utf8_lossy(&plain_out.stderr)
    );
    assert!(plain_als.is_file());
    assert!(!dir.join("PlainSet Project").exists());
    let plain_loaded = AlsParser::load_project(&plain_als).expect("load plain als");
    assert_eq!(plain_loaded.transport.bpm(), 131.0);

    // With --project: creates <Name> Project/<Name>.als layout.
    let proj_target = dir.join("StudioSong.als");
    let proj_out = run_livecraft(&["new", path_str(&proj_target), "--bpm", "128", "--project"]);
    assert!(
        proj_out.status.success(),
        "{}",
        String::from_utf8_lossy(&proj_out.stderr)
    );
    let expected_root = dir.join("StudioSong Project");
    let expected_als = expected_root.join("StudioSong.als");
    assert!(!proj_target.exists());
    assert!(expected_root.join("Ableton Project Info").is_dir());
    assert!(expected_root.join("Samples/Recorded").is_dir());
    assert!(expected_root.join("Samples/Imported").is_dir());
    assert!(expected_root.join("Samples/Processed").is_dir());
    assert!(expected_als.is_file());
    let proj_loaded = AlsParser::load_project(&expected_als).expect("load project als");
    assert_eq!(proj_loaded.name, "StudioSong");
    assert_eq!(proj_loaded.transport.bpm(), 128.0);
}

#[test]
fn health_reports_healthy_and_unhealthy_projects_in_text_and_json() {
    let dir = test_dir("health");

    // 1. Healthy project
    let clean_root = dir.join("Clean Project");
    create_project_layout(&clean_root).expect("clean layout");
    let clean_wav = clean_root.join("Samples/Imported/clean.wav");
    write_tone_wav(&clean_wav, 44_100, 1024);
    let clean_proj = make_audio_set(
        "Clean",
        &[(
            "Samples/Imported/clean.wav",
            Some("Samples/Imported/clean.wav"),
        )],
    );
    let clean_als = clean_root.join("Clean.als");
    AlsDocument::from_project(&clean_proj)
        .expect("clean doc")
        .save_als(&clean_als)
        .expect("save clean als");

    let clean_text = run_livecraft(&[
        "health",
        path_str(&clean_als),
        "--sample-rate",
        "44100",
        "--strict",
    ]);
    assert!(
        clean_text.status.success(),
        "{}",
        String::from_utf8_lossy(&clean_text.stderr)
    );
    let clean_stdout = String::from_utf8_lossy(&clean_text.stdout);
    assert!(clean_stdout.contains("HEALTHY"), "{clean_stdout}");
    assert!(clean_stdout.contains("All checks passed"), "{clean_stdout}");

    let clean_json_out = run_livecraft(&[
        "health",
        path_str(&clean_als),
        "--sample-rate",
        "44100",
        "--json",
    ]);
    assert!(clean_json_out.status.success());
    let clean_report: ProjectHealthReport =
        serde_json::from_slice(&clean_json_out.stdout).expect("parse healthy JSON");
    assert!(clean_report.is_healthy());
    assert_eq!(clean_report.issue_count(), 0);

    // 2. Unhealthy project covering all 5 categories
    let broken_root = dir.join("Broken Project");
    create_project_layout(&broken_root).expect("broken layout");
    let wav_48k = broken_root.join("Samples/Recorded/take_48k.wav");
    let unused_wav = broken_root.join("Samples/Imported/unused.wav");
    let user_lib = dir.join("UserLibrary");
    let user_hat = user_lib.join("hat.wav");
    write_tone_wav(&wav_48k, 48_000, 1024);
    write_tone_wav(&unused_wav, 44_100, 1024);
    write_tone_wav(&user_hat, 44_100, 1024);

    let mut broken_proj = make_audio_set(
        "Broken",
        &[
            (
                "Samples/Recorded/take_48k.wav",
                Some("Samples/Recorded/take_48k.wav"),
            ),
            (&user_hat.to_string_lossy(), None),
            ("C:/missing_drive_xyz/gone.wav", None),
        ],
    );
    broken_proj.tracks[0].device_chain.push(Device::new(
        DeviceId(999),
        "MissingPlugin",
        DeviceType::Custom("AuPluginDevice".to_owned()),
    ));
    let broken_als = broken_root.join("Broken.als");
    AlsDocument::from_project(&broken_proj)
        .expect("broken doc")
        .save_als(&broken_als)
        .expect("save broken als");

    let broken_text = run_livecraft(&[
        "health",
        path_str(&broken_als),
        "--sample-rate",
        "44100",
        "--user-library",
        path_str(&user_lib),
    ]);
    assert!(broken_text.status.success());
    let broken_stdout = String::from_utf8_lossy(&broken_text.stdout);
    for expected in [
        "ISSUES FOUND",
        "Missing files (1):",
        "Unused files (1):",
        "Missing plug-ins (1):",
        "Sample-rate mismatches (1):",
        "External files (1):",
    ] {
        assert!(
            broken_stdout.contains(expected),
            "missing '{expected}' in:\n{broken_stdout}"
        );
    }

    let broken_json_out = run_livecraft(&[
        "health",
        path_str(&broken_als),
        "--sample-rate",
        "44100",
        "--user-library",
        path_str(&user_lib),
        "--json",
    ]);
    assert!(broken_json_out.status.success());
    let broken_report: ProjectHealthReport =
        serde_json::from_slice(&broken_json_out.stdout).expect("parse unhealthy JSON");
    assert!(!broken_report.is_healthy());
    assert_eq!(broken_report.issue_count(), 5);
    assert_eq!(broken_report.missing_files.len(), 1);
    assert_eq!(broken_report.unused_files.len(), 1);
    assert_eq!(broken_report.missing_plugins.len(), 1);
    assert_eq!(broken_report.sample_rate_mismatches.len(), 1);
    assert_eq!(broken_report.external_files.len(), 1);
    assert_eq!(
        broken_report.external_files[0].location,
        SampleLocation::UserLibrary
    );

    let strict_out = run_livecraft(&[
        "health",
        path_str(&broken_als),
        "--sample-rate",
        "44100",
        "--strict",
    ]);
    assert!(!strict_out.status.success());
}

#[test]
fn collect_copies_external_wavs_without_moving_originals() {
    let dir = test_dir("collect");
    let proj_root = dir.join("Song Project");
    create_project_layout(&proj_root).expect("layout");
    let user_lib = dir.join("UserLibrary");

    let ext_kick = dir.join("External/kick.wav");
    let lib_snare = user_lib.join("snare.wav");
    write_tone_wav(&ext_kick, 44_100, 1024);
    write_tone_wav(&lib_snare, 44_100, 1024);

    let proj = make_audio_set(
        "Song",
        &[
            (&ext_kick.to_string_lossy(), None),
            (&lib_snare.to_string_lossy(), None),
        ],
    );
    let als_path = proj_root.join("Song.als");
    AlsDocument::from_project(&proj)
        .expect("doc")
        .save_als(&als_path)
        .expect("save als");

    // Default collect copies Elsewhere (kick.wav) and skips UserLibrary (snare.wav).
    let first = run_livecraft(&[
        "collect",
        path_str(&als_path),
        "--user-lib-dir",
        path_str(&user_lib),
        "--json",
    ]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let rep1: CollectReport =
        serde_json::from_slice(&first.stdout).expect("parse collect report 1");
    assert_eq!(rep1.collected.len(), 1);
    assert_eq!(rep1.skipped_by_policy.len(), 1);
    assert!(proj_root.join("Samples/Imported/kick.wav").is_file());
    assert!(
        ext_kick.is_file(),
        "original external WAV must not be moved"
    );
    assert!(!proj_root.join("Samples/Imported/snare.wav").exists());

    // With --user-library: also collects snare.wav without moving original.
    let second = run_livecraft(&[
        "collect",
        path_str(&als_path),
        "--user-lib-dir",
        path_str(&user_lib),
        "--user-library",
        "--json",
    ]);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let rep2: CollectReport =
        serde_json::from_slice(&second.stdout).expect("parse collect report 2");
    assert_eq!(rep2.collected.len(), 1);
    assert!(rep2.skipped_by_policy.is_empty());
    assert!(proj_root.join("Samples/Imported/snare.wav").is_file());
    assert!(
        lib_snare.is_file(),
        "original library WAV must not be moved"
    );

    // Health check on the collected Set now reports zero issues.
    let health = run_livecraft(&[
        "health",
        path_str(&als_path),
        "--user-library",
        path_str(&user_lib),
        "--json",
    ]);
    assert!(health.status.success());
    let health_rep: ProjectHealthReport =
        serde_json::from_slice(&health.stdout).expect("parse health after collect");
    assert!(health_rep.is_healthy());
}

#[test]
fn unused_reports_unused_samples_and_rejects_non_project_dir() {
    let dir = test_dir("unused");
    let proj_root = dir.join("Unused Project");
    create_project_layout(&proj_root).expect("layout");

    let used_wav = proj_root.join("Samples/Imported/used.wav");
    let orphan_imp = proj_root.join("Samples/Imported/orphan_imp.wav");
    let orphan_rec = proj_root.join("Samples/Recorded/orphan_rec.wav");
    write_tone_wav(&used_wav, 44_100, 1024);
    write_tone_wav(&orphan_imp, 44_100, 512);
    write_tone_wav(&orphan_rec, 44_100, 512);

    let proj = make_audio_set(
        "UnusedSet",
        &[(
            "Samples/Imported/used.wav",
            Some("Samples/Imported/used.wav"),
        )],
    );
    AlsDocument::from_project(&proj)
        .expect("doc")
        .save_als(proj_root.join("UnusedSet.als"))
        .expect("save als");

    let text_out = run_livecraft(&["unused", path_str(&proj_root)]);
    assert!(
        text_out.status.success(),
        "{}",
        String::from_utf8_lossy(&text_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&text_out.stdout);
    assert!(stdout.contains("orphan_imp.wav"), "{stdout}");
    assert!(stdout.contains("orphan_rec.wav"), "{stdout}");
    assert!(!stdout.contains("used.wav"), "{stdout}");

    let json_out = run_livecraft(&["unused", path_str(&proj_root), "--json"]);
    assert!(json_out.status.success());
    let report: UnusedFilesReport =
        serde_json::from_slice(&json_out.stdout).expect("parse unused JSON");
    assert_eq!(report.scanned_documents.len(), 1);
    assert_eq!(report.unused_files.len(), 2);
    let rels: Vec<&str> = report
        .unused_files
        .iter()
        .map(|u| u.relative_path.as_str())
        .collect();
    assert_eq!(
        rels,
        vec![
            "Samples/Imported/orphan_imp.wav",
            "Samples/Recorded/orphan_rec.wav"
        ]
    );

    // Non-project folder is rejected cleanly.
    let non_proj = run_livecraft(&["unused", path_str(&dir)]);
    assert!(!non_proj.status.success());
}

#[test]
fn pack_install_extracts_valid_alp_and_rejects_zip_slip() {
    let dir = test_dir("pack_install");

    let valid_alp = dir.join("DemoPack.alp");
    write_store_zip(
        &valid_alp,
        &[
            ("DemoPack/Sets/Demo.als", b"als-bytes"),
            ("DemoPack/Samples/kick.wav", b"RIFFdemo"),
        ],
    );

    let dest_json = dir.join("DestJson");
    let json_out = run_livecraft(&[
        "pack",
        "install",
        path_str(&valid_alp),
        path_str(&dest_json),
        "--json",
    ]);
    assert!(
        json_out.status.success(),
        "{}",
        String::from_utf8_lossy(&json_out.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&json_out.stdout).expect("parse pack install JSON");
    assert_eq!(parsed["extracted_files"].as_array().map(Vec::len), Some(2));
    assert_eq!(parsed["sets_found"].as_array().map(Vec::len), Some(1));
    assert_eq!(parsed["total_bytes"], serde_json::json!(17));
    assert_eq!(
        fs::read(dest_json.join("DemoPack/Sets/Demo.als")).expect("read Demo.als"),
        b"als-bytes"
    );
    assert_eq!(
        fs::read(dest_json.join("DemoPack/Samples/kick.wav")).expect("read kick.wav"),
        b"RIFFdemo"
    );

    let dest_text = dir.join("DestText");
    let text_out = run_livecraft(&[
        "pack",
        "install",
        path_str(&valid_alp),
        path_str(&dest_text),
    ]);
    assert!(text_out.status.success());
    let text_stdout = String::from_utf8_lossy(&text_out.stdout);
    assert!(text_stdout.contains("Installed Pack"), "{text_stdout}");
    assert!(text_stdout.contains("Demo.als"), "{text_stdout}");

    // Zip-slip ../../evil must fail without writing any files.
    let evil_alp = dir.join("Evil.alp");
    let evil_dest = dir.join("EvilDest");
    write_store_zip(&evil_alp, &[("../../evil.txt", b"pwn")]);
    let evil_out = run_livecraft(&["pack", "install", path_str(&evil_alp), path_str(&evil_dest)]);
    assert!(!evil_out.status.success(), "zip-slip pack must be rejected");
    assert!(!evil_dest.exists());
    assert!(!dir.join("evil.txt").exists());
}

#[test]
fn midi_import_and_export_round_trip() {
    let dir = test_dir("midi_roundtrip");

    let mid_in = dir.join("melody.mid");
    let notes = vec![
        MidiNote::new(60, 0.0, 1.0, 100),
        MidiNote::new(64, 1.0, 0.5, 90),
        MidiNote::new(67, 2.0, 1.5, 110),
    ];
    let smf_bytes =
        write_smf(&notes, 124.0, (4, 4), SmfFormat::Format0, Some("Bassline")).expect("encode smf");
    fs::write(&mid_in, smf_bytes).expect("write mid_in");

    let base_als = dir.join("base.als");
    AlsParser::save_project_to_als(&Project::new_default("MidiCli"), &base_als)
        .expect("save base als");

    // 1. Import into track 1 slot 0.
    let imported_als = dir.join("imported.als");
    let imp_out = run_livecraft(&[
        "midi",
        "import",
        path_str(&base_als),
        path_str(&mid_in),
        "--track",
        "1",
        "--slot",
        "0",
        "-o",
        path_str(&imported_als),
    ]);
    assert!(
        imp_out.status.success(),
        "{}",
        String::from_utf8_lossy(&imp_out.stderr)
    );

    let loaded = AlsParser::load_project(&imported_als).expect("load imported als");
    let clip = loaded.tracks[0].clip_slots[0]
        .clip
        .as_ref()
        .expect("imported clip in slot 0");
    let clip_id_str = clip.id.0.to_string();
    match &clip.payload {
        ClipPayload::Midi(m) => {
            assert_eq!(m.notes.len(), 3);
            assert_eq!(m.notes[0].pitch, 60);
            assert_eq!(m.notes[1].pitch, 64);
            assert_eq!(m.notes[2].pitch, 67);
        }
        other => panic!("expected MIDI clip, got {other:?}"),
    }

    // 2. Export the imported clip back to .mid and verify notes round-trip.
    let mid_out = dir.join("exported.mid");
    let exp_out = run_livecraft(&[
        "midi",
        "export",
        path_str(&imported_als),
        "--clip",
        &clip_id_str,
        "-o",
        path_str(&mid_out),
    ]);
    assert!(
        exp_out.status.success(),
        "{}",
        String::from_utf8_lossy(&exp_out.stderr)
    );
    let reparsed = read_smf_file(&mid_out).expect("read exported mid");
    assert_eq!(reparsed.merged_notes.len(), 3);
    assert_eq!(reparsed.merged_notes[0].pitch, 60);
    assert_eq!(reparsed.merged_notes[1].pitch, 64);
    assert_eq!(reparsed.merged_notes[2].pitch, 67);
    assert!((reparsed.merged_notes[2].start_beat - 2.0).abs() < 1e-3);
    assert!((reparsed.merged_notes[2].duration_beats - 1.5).abs() < 1e-3);

    // 3. Import into arrangement (--beat 4.0) and onto a new track (no --track).
    let arr_als = dir.join("arr.als");
    let arr_out = run_livecraft(&[
        "midi",
        "import",
        path_str(&base_als),
        path_str(&mid_in),
        "--track",
        "1",
        "--beat",
        "4.0",
        "-o",
        path_str(&arr_als),
    ]);
    assert!(arr_out.status.success());
    let arr_loaded = AlsParser::load_project(&arr_als).expect("load arr als");
    assert_eq!(arr_loaded.tracks[0].arrangement_clips.len(), 1);
    assert_eq!(arr_loaded.tracks[0].arrangement_clips[0].start_beat, 4.0);

    let new_trk_als = dir.join("new_trk.als");
    let new_trk_out = run_livecraft(&[
        "midi",
        "import",
        path_str(&base_als),
        path_str(&mid_in),
        "-o",
        path_str(&new_trk_als),
    ]);
    assert!(new_trk_out.status.success());
    let new_trk_loaded = AlsParser::load_project(&new_trk_als).expect("load new track als");
    assert_eq!(new_trk_loaded.tracks.len(), 5);
    assert!(new_trk_loaded.tracks[4].clip_slots[0].clip.is_some());
}

#[test]
fn clip_export_alc_and_import_alc_preserve_devices_and_copy_audio() {
    let dir = test_dir("clip_alc");

    // 1. Export MIDI .alc with custom device parameter and import onto empty vs non-empty tracks.
    let mut src_proj = Project::new_default("AlcSrc");
    src_proj.tracks[0].device_chain[0].set_param("filter_cutoff", 1450.0);
    let mut midi_clip = Clip::new_midi(ClipId(77), "SynthHook", 4.0);
    if let ClipPayload::Midi(ref mut m) = midi_clip.payload {
        m.notes.push(MidiNote::new(48, 0.0, 1.0, 110));
    }
    src_proj.tracks[0].clip_slots[0].clip = Some(midi_clip);
    let src_als = dir.join("src.als");
    AlsParser::save_project_to_als(&src_proj, &src_als).expect("save src als");

    let midi_alc = dir.join("SynthHook.alc");
    let exp_midi = run_livecraft(&[
        "clip",
        "export-alc",
        path_str(&src_als),
        "--track",
        "1",
        "--clip",
        "77",
        "-o",
        path_str(&midi_alc),
    ]);
    assert!(
        exp_midi.status.success(),
        "{}",
        String::from_utf8_lossy(&exp_midi.stderr)
    );
    assert!(midi_alc.is_file());

    let dst_als = dir.join("dst.als");
    let dst_proj = Project::new_default("AlcDst");
    let dst_t1_devs = dst_proj.tracks[0].device_chain.len();
    AlsParser::save_project_to_als(&dst_proj, &dst_als).expect("save dst als");

    // Empty track 2 receives clip AND device chain.
    let dst_empty_als = dir.join("dst_empty.als");
    let imp_empty = run_livecraft(&[
        "clip",
        "import-alc",
        path_str(&dst_als),
        path_str(&midi_alc),
        "--track",
        "2",
        "--slot",
        "0",
        "-o",
        path_str(&dst_empty_als),
    ]);
    assert!(
        imp_empty.status.success(),
        "{}",
        String::from_utf8_lossy(&imp_empty.stderr)
    );
    let loaded_empty = AlsParser::load_project(&dst_empty_als).expect("load dst empty");
    assert!(loaded_empty.tracks[1].clip_slots[0].clip.is_some());
    assert_eq!(loaded_empty.tracks[1].device_chain.len(), 1);
    assert!(
        (loaded_empty.tracks[1].device_chain[0].get_param("filter_cutoff") - 1450.0).abs() < 1e-2
    );

    // Non-empty track 1 receives clip WITHOUT duplicating devices.
    let dst_nonempty_als = dir.join("dst_nonempty.als");
    let imp_nonempty = run_livecraft(&[
        "clip",
        "import-alc",
        path_str(&dst_als),
        path_str(&midi_alc),
        "--track",
        "1",
        "--slot",
        "0",
        "-o",
        path_str(&dst_nonempty_als),
    ]);
    assert!(imp_nonempty.status.success());
    let loaded_nonempty = AlsParser::load_project(&dst_nonempty_als).expect("load dst nonempty");
    assert!(loaded_nonempty.tracks[0].clip_slots[0].clip.is_some());
    assert_eq!(loaded_nonempty.tracks[0].device_chain.len(), dst_t1_devs);

    // 2. Audio .alc export with --copy-into copies sample into target project without moving original.
    let ext_guitar = dir.join("External/guitar.wav");
    write_tone_wav(&ext_guitar, 44_100, 2048);
    let mut audio_src = Project::new_default("AudioSrc");
    audio_src.tracks[2].clip_slots[0].clip = Some(Clip::new_audio(
        ClipId(88),
        "Guitar",
        ext_guitar.to_string_lossy(),
        4.0,
    ));
    let audio_src_als = dir.join("audio_src.als");
    AlsParser::save_project_to_als(&audio_src, &audio_src_als).expect("save audio src");

    let target_root = dir.join("Target Project");
    create_project_layout(&target_root).expect("target layout");
    let target_alc = target_root.join("Clips/Guitar.alc");
    let exp_audio = run_livecraft(&[
        "clip",
        "export-alc",
        path_str(&audio_src_als),
        "--track",
        "3",
        "--clip",
        "88",
        "-o",
        path_str(&target_alc),
        "--copy-into",
        path_str(&target_root),
    ]);
    assert!(
        exp_audio.status.success(),
        "{}",
        String::from_utf8_lossy(&exp_audio.stderr)
    );
    assert!(target_root.join("Samples/Imported/guitar.wav").is_file());
    assert!(ext_guitar.is_file(), "original WAV must remain untouched");

    let target_set = target_root.join("TargetSet.als");
    AlsParser::save_project_to_als(&Project::new_default("TargetSet"), &target_set)
        .expect("save target set");
    let target_out = target_root.join("TargetSetImported.als");
    let imp_audio = run_livecraft(&[
        "clip",
        "import-alc",
        path_str(&target_set),
        path_str(&target_alc),
        "--track",
        "3",
        "--slot",
        "0",
        "-o",
        path_str(&target_out),
    ]);
    assert!(
        imp_audio.status.success(),
        "{}",
        String::from_utf8_lossy(&imp_audio.stderr)
    );
    let target_loaded = AlsParser::load_project(&target_out).expect("load target imported");
    let imported_clip = target_loaded.tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("imported audio clip");
    match &imported_clip.payload {
        ClipPayload::Audio(audio) => {
            assert_eq!(
                audio.relative_path.as_deref(),
                Some("Samples/Imported/guitar.wav")
            );
        }
        other => panic!("expected Audio clip, got {other:?}"),
    }
}
