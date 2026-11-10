use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use livecraft_formats::als::AlsParser;
use livecraft_formats::live_clip::{LiveClipFile, load_live_clip, save_live_clip};
use livecraft_formats::midi_file::{
    MAX_MIDI_BYTES, SmfError, SmfFormat, export_clip_to_smf, read_smf, read_smf_file,
    read_smf_to_clip, write_smf,
};
use livecraft_formats::pack::{MAX_ENTRY_BYTES, MAX_TOTAL_BYTES, PackError, install_pack};
use livecraft_formats::presets::{
    RackPreset, load_device_preset, load_rack_preset, save_device_preset, save_rack_preset,
};
use livecraft_model::ids::{ClipId, DeviceId};
use livecraft_model::model::{
    Clip, ClipPayload, Device, DeviceType, LaunchMode, MidiNote, WarpMode,
};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

fn temp_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("livecraft_m10_formats_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn create_test_zip(path: &Path, entries: &[(&str, &[u8])]) {
    let file = File::create(path).expect("create zip");
    let mut zw = ZipWriter::new(file);
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for &(name, data) in entries {
        zw.start_file(name, opts).expect("start zip entry");
        zw.write_all(data).expect("write zip entry");
    }
    zw.finish().expect("finish zip");
}

#[test]
fn smf_export_and_import_clip_roundtrip_format_0_and_1() {
    let dir = temp_test_dir("smf_clip");

    let mut clip = Clip::new_midi(ClipId(10), "Arp Idea", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes = vec![
            MidiNote::new(60, 0.0, 0.5, 100),
            MidiNote::new(64, 0.5, 0.5, 92),
            MidiNote::new(67, 1.0, 1.0, 110),
            MidiNote::new(72, 2.0, 2.0, 120),
        ];
    }

    for (fmt, fname) in [
        (SmfFormat::Format0, "arp_f0.mid"),
        (SmfFormat::Format1, "arp_f1.mid"),
    ] {
        let mid_path = dir.join(fname);
        export_clip_to_smf(&clip, &mid_path, 132.0, (6, 8), fmt).expect("export SMF");

        let parsed = read_smf_file(&mid_path).expect("read SMF file");
        assert_eq!(parsed.format, fmt);
        assert_eq!(parsed.ppqn, 480);
        assert!((parsed.bpm - 132.0).abs() < 0.01);
        assert_eq!(parsed.time_signature, (6, 8));
        assert_eq!(parsed.merged_notes.len(), 4);

        let imported_clip = read_smf_to_clip(&mid_path).expect("import SMF to clip");
        assert_eq!(imported_clip.name, "Arp Idea");
        assert!((imported_clip.length_beats - 4.0).abs() < 1e-6);
        let ClipPayload::Midi(ref m) = imported_clip.payload else {
            panic!("expected MIDI clip payload");
        };
        assert_eq!(m.notes.len(), 4);
        assert_eq!(m.notes[0].pitch, 60);
        assert_eq!(m.notes[3].pitch, 72);
        assert_eq!(m.notes[3].velocity, 120.0);
    }

    let audio_clip = Clip::new_audio(ClipId(11), "Audio", "kick.wav", 4.0);
    let err = export_clip_to_smf(
        &audio_clip,
        &dir.join("invalid.mid"),
        120.0,
        (4, 4),
        SmfFormat::Format0,
    )
    .unwrap_err();
    assert!(matches!(err, SmfError::Unsupported(_)));

    assert_eq!(MAX_MIDI_BYTES, 32 * 1024 * 1024);
    let raw = write_smf(&[], 120.0, (4, 4), SmfFormat::Format0, None).expect("empty SMF");
    assert!(read_smf(&raw).is_ok());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn pack_install_extracts_valid_pack_and_reports_sets() {
    let dir = temp_test_dir("pack_valid");
    let alp_path = dir.join("SynthPack.alp");
    let dest = dir.join("Installed");

    assert_eq!(MAX_ENTRY_BYTES, 512 * 1024 * 1024);
    assert_eq!(MAX_TOTAL_BYTES, 2 * 1024 * 1024 * 1024);

    let set_bytes = b"fake-als-payload";
    let sample_bytes = b"RIFFsample";
    create_test_zip(
        &alp_path,
        &[
            ("Demo Project/Demo.als", set_bytes),
            ("Demo Project/Samples/Imported/bass.wav", sample_bytes),
            ("Demo Project/Ableton Project Info/Project8_1.cfg", b"cfg"),
        ],
    );

    let report = install_pack(&alp_path, &dest).expect("install valid pack");
    assert_eq!(report.extracted_files.len(), 3);
    assert_eq!(report.sets_found.len(), 1);
    assert_eq!(report.sets_found[0], dest.join("Demo Project/Demo.als"));
    assert_eq!(
        report.total_bytes,
        (set_bytes.len() + sample_bytes.len() + 3) as u64
    );
    assert_eq!(
        fs::read(dest.join("Demo Project/Demo.als")).expect("read extracted set"),
        set_bytes
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn pack_install_rejects_zip_slip_traversal_variants_with_zero_writes() {
    let dir = temp_test_dir("pack_zip_slip");
    let dest = dir.join("SafeDest");
    let outside_evil = dir.join("evil");

    for (idx, malicious_name) in ["../../evil", r"..\evil", "/tmp/evil"].iter().enumerate() {
        let alp_path = dir.join(format!("malicious_{idx}.alp"));
        let file = File::create(&alp_path).expect("create malicious zip");
        let mut zw = ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zw.start_file("safe_before.txt", opts).expect("start safe");
        zw.write_all(b"ok").expect("write safe");
        zw.start_file(*malicious_name, opts).expect("start evil");
        zw.write_all(b"pwned").expect("write evil");
        zw.finish().expect("finish malicious zip");

        let err = install_pack(&alp_path, &dest).unwrap_err();
        assert!(
            matches!(err, PackError::PathTraversal(_)),
            "expected PackError::PathTraversal for `{malicious_name}`, got {err:?}"
        );
        assert!(
            !outside_evil.exists(),
            "evil file leaked outside dest for `{malicious_name}`"
        );
        assert!(
            !dest.exists(),
            "no files should be written into dest when archive contains `{malicious_name}`"
        );
    }

    // Also verify symlink entry rejection.
    let sym_alp = dir.join("symlink.alp");
    let file = File::create(&sym_alp).expect("create symlink zip");
    let mut zw = ZipWriter::new(file);
    zw.add_symlink("escape_link", "../evil", SimpleFileOptions::default())
        .expect("add symlink");
    zw.finish().expect("finish symlink zip");
    let err = install_pack(&sym_alp, &dest).unwrap_err();
    assert!(matches!(err, PackError::PathTraversal(_)));
    assert!(!outside_evil.exists());
    assert!(!dest.exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn live_clip_midi_and_audio_roundtrip_with_sample_copy() {
    let dir = temp_test_dir("live_clip");
    let sample_dir = dir.join("ExternalSamples");
    let project_dir = dir.join("MyProject");
    fs::create_dir_all(&sample_dir).expect("create sample dir");
    fs::create_dir_all(&project_dir).expect("create project dir");

    // 1. MIDI Live Clip with launch mode & devices
    let mut midi_clip = Clip::new_midi(ClipId(40), "Lead Hook", 8.0);
    midi_clip.launch.mode = LaunchMode::Gate;
    midi_clip.launch.legato = true;
    if let ClipPayload::Midi(ref mut mdata) = midi_clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 1.0, 108));
        mdata.notes.push(MidiNote::new(67, 2.0, 1.5, 96));
    }
    let mut af = Device::new(DeviceId(50), "Sweep", DeviceType::AutoFilter);
    af.set_param("cutoff", 1800.0);
    af.set_param("resonance", 2.2);

    let midi_alc = project_dir.join("Clips/Lead Hook.alc");
    save_live_clip(
        &LiveClipFile {
            clip: midi_clip,
            devices: vec![af],
        },
        &midi_alc,
        None,
    )
    .expect("save MIDI .alc");

    let loaded_midi = load_live_clip(&midi_alc).expect("load MIDI .alc");
    assert_eq!(loaded_midi.clip.name, "Lead Hook");
    assert_eq!(loaded_midi.clip.launch.mode, LaunchMode::Gate);
    assert!(loaded_midi.clip.launch.legato);
    assert_eq!(loaded_midi.devices.len(), 1);
    assert!((loaded_midi.devices[0].get_param("cutoff") - 1800.0).abs() < 1e-3);

    // 2. Audio Live Clip with copy_audio_into
    let sample_path = sample_dir.join("vox_chop.wav");
    let wav_bytes = b"RIFF0000WAVEfmt ";
    fs::write(&sample_path, wav_bytes).expect("write wav");

    let mut audio_clip =
        Clip::new_audio(ClipId(41), "Vox Chop", sample_path.to_string_lossy(), 4.0);
    if let ClipPayload::Audio(ref mut adata) = audio_clip.payload {
        adata.warp_mode = WarpMode::ComplexPro;
        adata.pitch_cents = 300;
    }
    let audio_alc = project_dir.join("Clips/Vox Chop.alc");
    save_live_clip(
        &LiveClipFile {
            clip: audio_clip,
            devices: vec![Device::new(DeviceId(51), "Utility", DeviceType::Utility)],
        },
        &audio_alc,
        Some(&project_dir),
    )
    .expect("save Audio .alc");

    let copied_sample = project_dir.join("Samples/Imported/vox_chop.wav");
    assert!(copied_sample.is_file());
    assert_eq!(fs::read(&copied_sample).expect("read copy"), wav_bytes);
    assert_eq!(fs::read(&sample_path).expect("original intact"), wav_bytes);

    let xml = AlsParser::decompress_to_xml(&audio_alc).expect("decompress .alc");
    assert!(xml.contains("<RelativePath Value=\"Samples/Imported/vox_chop.wav\""));

    let loaded_audio = load_live_clip(&audio_alc).expect("load Audio .alc");
    assert_eq!(loaded_audio.devices.len(), 1);
    assert_eq!(loaded_audio.devices[0].device_type, DeviceType::Utility);
    if let ClipPayload::Audio(ref adata) = loaded_audio.clip.payload {
        assert!(adata.file_path.contains("Samples/Imported/vox_chop.wav"));
        assert_eq!(
            adata.relative_path.as_deref(),
            Some("Samples/Imported/vox_chop.wav")
        );
        assert_eq!(adata.warp_mode, WarpMode::ComplexPro);
        assert_eq!(adata.pitch_cents, 300);
    } else {
        panic!("expected Audio clip payload");
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn device_and_rack_presets_roundtrip() {
    let dir = temp_test_dir("presets");

    let mut rev = Device::new(DeviceId(91), "Hall Space", DeviceType::Reverb);
    rev.set_param("decay_time_sec", 4.5);
    rev.set_param("dry_wet", 0.42);
    let adv_path = dir.join("Hall Space.adv");
    save_device_preset(&rev, &adv_path).expect("save .adv");
    let loaded_rev = load_device_preset(&adv_path).expect("load .adv");
    assert_eq!(loaded_rev.name, "Hall Space");
    assert_eq!(loaded_rev.device_type, DeviceType::Reverb);
    assert!((loaded_rev.get_param("decay_time_sec") - 4.5).abs() < 1e-3);
    assert!((loaded_rev.get_param("dry_wet") - 0.42).abs() < 1e-3);

    let mut sat = Device::new(DeviceId(92), "Tape Warmth", DeviceType::Saturator);
    sat.set_param("drive_db", 7.25);
    let mut eq = Device::new(DeviceId(93), "Tone Shape", DeviceType::EqThree);
    eq.set_param("low_gain_db", 1.5);
    eq.set_param("high_on", 0.0);

    let rack = RackPreset {
        name: "Mix Bus Glue".to_owned(),
        devices: vec![sat, eq],
        macros: [0.2, 0.4, 0.6, 0.8, 1.0, 0.0, 0.5, 0.15],
    };
    let adg_path = dir.join("Mix Bus Glue.adg");
    save_rack_preset(&rack, &adg_path).expect("save .adg");
    let loaded_rack = load_rack_preset(&adg_path).expect("load .adg");

    assert_eq!(loaded_rack.name, "Mix Bus Glue");
    assert_eq!(loaded_rack.devices.len(), 2);
    assert_eq!(loaded_rack.devices[0].device_type, DeviceType::Saturator);
    assert!((loaded_rack.devices[0].get_param("drive_db") - 7.25).abs() < 1e-3);
    assert_eq!(loaded_rack.devices[1].device_type, DeviceType::EqThree);
    assert!((loaded_rack.devices[1].get_param("low_gain_db") - 1.5).abs() < 1e-3);
    assert!((loaded_rack.devices[1].get_param("high_on") - 0.0).abs() < 1e-3);
    for (a, b) in rack.macros.iter().zip(&loaded_rack.macros) {
        assert!((*a - *b).abs() < 1e-5);
    }

    let _ = fs::remove_dir_all(&dir);
}
