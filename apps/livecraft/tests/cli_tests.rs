use livecraft_devices::operator::{OperatorSynth, Waveform};
use livecraft_engine::bouncer::{OfflineBouncer, RenderOptions};
use livecraft_formats::als::AlsParser;
use livecraft_model::model::*;
use std::fs;

#[test]
fn test_cli_new_and_roundtrip_project() {
    let tmp_dir = std::env::temp_dir().join("livecraft_cli_test_1");
    let _ = fs::create_dir_all(&tmp_dir);
    let als_path = tmp_dir.join("NewSong.als");
    let json_path = tmp_dir.join("NewSong.json");

    let mut project = Project::new_default("NewSong");
    project.transport.bpm = 135.0;

    // Save as .als
    AlsParser::save_project_to_als(&project, &als_path).expect("Failed to save .als");
    assert!(als_path.exists());

    // Load from .als
    let loaded_als = AlsParser::load_project(&als_path).expect("Failed to load .als");
    assert_eq!(loaded_als.transport.bpm, 135.0);
    assert_eq!(loaded_als.tracks.len(), 4);

    // Save as JSON
    let json_data = serde_json::to_string_pretty(&loaded_als).expect("Failed to serialize to JSON");
    fs::write(&json_path, json_data).expect("Failed to write JSON");
    assert!(json_path.exists());

    // Read back JSON
    let read_json = fs::read_to_string(&json_path).expect("Failed to read JSON");
    let loaded_json: Project = serde_json::from_str(&read_json).expect("Failed to parse JSON");
    assert_eq!(loaded_json.name, "NewSong");
    assert_eq!(loaded_json.transport.bpm, 135.0);

    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_cli_synth_to_wav() {
    let tmp_dir = std::env::temp_dir().join("livecraft_cli_test_2");
    let _ = fs::create_dir_all(&tmp_dir);
    let out_wav = tmp_dir.join("synth_saw.wav");

    let sample_rate = 44100;
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&out_wav, spec).expect("Failed to create WAV");

    let mut synth = OperatorSynth::new(sample_rate as f32);
    synth.osc1_wave = Waveform::Sawtooth;
    synth.note_on(69, 100); // A4 (440 Hz)

    for _ in 0..sample_rate {
        let (l, r) = synth.process_sample();
        writer
            .write_sample((l.clamp(-1.0, 1.0) * 32767.0) as i16)
            .unwrap();
        writer
            .write_sample((r.clamp(-1.0, 1.0) * 32767.0) as i16)
            .unwrap();
    }
    writer.finalize().expect("Failed to finalize WAV");

    assert!(out_wav.exists());
    let reader = hound::WavReader::open(&out_wav).expect("Failed to open written WAV");
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.spec().sample_rate, 44100);
    assert!(reader.len() > 0);

    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_cli_render_and_stem_pipeline() {
    let tmp_dir = std::env::temp_dir().join("livecraft_cli_test_3");
    let _ = fs::create_dir_all(&tmp_dir);
    let full_mix = tmp_dir.join("full_mix.wav");
    let stems_dir = tmp_dir.join("stems");

    let mut project = Project::new_default("PipelineTest");
    project.transport.bpm = 120.0;

    let mut clip = Clip::new_midi(1, "Chord Track", 4.0);
    if let ClipPayload::Midi(ref mut m) = clip.payload {
        m.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        m.notes.push(MidiNote::new(64, 0.0, 2.0, 100));
        m.notes.push(MidiNote::new(67, 0.0, 2.0, 100));
    }
    project.tracks[0].clip_slots[0].clip = Some(clip);

    let opts = RenderOptions {
        sample_rate: 44100,
        duration_bars: 2,
        bits_per_sample: 16,
    };

    // Full mix render
    OfflineBouncer::render_project(&project, &full_mix, opts.clone())
        .expect("Failed to render mix");
    assert!(full_mix.exists());

    // Stem export
    let stems =
        OfflineBouncer::render_stems(&project, &stems_dir, opts).expect("Failed to render stems");
    assert_eq!(stems.len(), project.tracks.len());
    for s in stems {
        assert!(std::path::Path::new(&s).exists());
    }

    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_cli_midi_transformations_and_tuning() {
    let mut project = Project::new_default("MidiToolTest");
    let mut clip = Clip::new_midi(1, "Pattern", 4.0);
    if let ClipPayload::Midi(ref mut m) = clip.payload {
        m.generate_euclidean(4, 16, 60, 4.0, 100);
        assert_eq!(m.notes.len(), 4);

        m.chop(2);
        assert_eq!(m.notes.len(), 8);
    }
    project.tracks[0].clip_slots[0].clip = Some(clip);

    let tuning =
        TuningSystem::from_scala("! slendro.scl\nSlendro\n5\n240.0\n480.0\n720.0\n960.0\n1200.0\n")
            .expect("Failed to parse slendro tuning");
    project.tuning = Some(tuning);

    let xml = AlsParser::export_to_xml(&project).expect("Failed to export XML with tuning");
    assert!(xml.contains("Pattern"));
}
