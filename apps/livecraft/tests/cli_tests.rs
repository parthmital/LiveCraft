use livecraft_devices::operator::{OperatorSynth, Waveform};
use livecraft_engine::bouncer::{
    ANALYSIS_FILE_VERSION, ANALYSIS_OVERVIEW_BLOCK_FRAMES, AnalysisFile, DitherMode,
    OfflineBouncer, RenderOptions, RenderReport,
};
use livecraft_formats::als::read::decompress_als_or_xml;
use livecraft_formats::als::{AlsDocument, AlsParser};
use livecraft_model::ids::DeviceId;
use livecraft_model::model::*;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Runs the `livecraft` binary with `args`.
fn run_livecraft(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_livecraft"))
        .args(args)
        .output()
        .expect("run the livecraft binary")
}

/// Runs `livecraft devices` with `args`.
fn livecraft_devices(args: &[&str]) -> Output {
    let mut full_args = vec!["devices"];
    full_args.extend_from_slice(args);
    run_livecraft(&full_args)
}

fn temp_cli_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("livecraft_cli_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn chord_project(name: &str) -> Project {
    let mut project = Project::new_default(name);
    project.transport.set_bpm(120.0);

    let mut clip = Clip::new_midi(1, "Chord Track", 4.0);
    if let ClipPayload::Midi(ref mut m) = clip.payload {
        m.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        m.notes.push(MidiNote::new(64, 0.0, 2.0, 100));
        m.notes.push(MidiNote::new(67, 0.0, 2.0, 100));
    }
    project.tracks[0].clip_slots[0].clip = Some(clip);
    project
}

fn write_project_json(dir: &Path, filename: &str, project: &Project) -> PathBuf {
    let path = dir.join(filename);
    let json = serde_json::to_string_pretty(project).expect("serialize project JSON");
    fs::write(&path, json).expect("write project JSON");
    path
}

fn sha256_hex(data: &[u8]) -> String {
    #[allow(clippy::unreadable_literal)]
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    #[allow(clippy::unreadable_literal)]
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (data.len() as u64) * 8;
    let mut padded = data.to_vec();
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for block in padded.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in block.as_chunks::<4>().0.iter().enumerate() {
            w[i] = u32::from_be_bytes(*word);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }

    let mut hex = String::with_capacity(64);
    for word in h {
        let _ = write!(hex, "{word:08x}");
    }
    hex
}

#[test]
fn test_cli_new_and_roundtrip_project() {
    let tmp_dir = std::env::temp_dir().join("livecraft_app_test_1");
    let _ = fs::create_dir_all(&tmp_dir);
    let als_path = tmp_dir.join("NewSong.als");
    let json_path = tmp_dir.join("NewSong.json");

    let mut project = Project::new_default("NewSong");
    project.transport.set_bpm(135.0);

    // Save as .als
    AlsParser::save_project_to_als(&project, &als_path).expect("Failed to save .als");
    assert!(als_path.exists());

    // Load from .als
    let loaded_als = AlsParser::load_project(&als_path).expect("Failed to load .als");
    assert_eq!(loaded_als.transport.bpm(), 135.0);
    assert_eq!(loaded_als.tracks.len(), 4);

    // Save as JSON
    let json_data = serde_json::to_string_pretty(&loaded_als).expect("Failed to serialize to JSON");
    fs::write(&json_path, json_data).expect("Failed to write JSON");
    assert!(json_path.exists());

    // Read back JSON
    let read_json = fs::read_to_string(&json_path).expect("Failed to read JSON");
    let loaded_json: Project = serde_json::from_str(&read_json).expect("Failed to parse JSON");
    assert_eq!(loaded_json.name, "NewSong");
    assert_eq!(loaded_json.transport.bpm(), 135.0);

    let _ = fs::remove_dir_all(tmp_dir);
}

#[test]
fn test_cli_synth_to_wav() {
    let tmp_dir = std::env::temp_dir().join("livecraft_app_test_2");
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
    let tmp_dir = temp_cli_dir("pipeline");
    let full_mix = tmp_dir.join("full_mix.wav");
    let stems_dir = tmp_dir.join("stems");

    let project = chord_project("PipelineTest");

    let opts = RenderOptions {
        sample_rate: 44100,
        duration_bars: 2,
        bits_per_sample: 16,
        ..RenderOptions::default()
    };

    // Full mix render
    OfflineBouncer::render_project(&project, &full_mix, opts.clone())
        .expect("Failed to render mix");
    assert!(full_mix.exists());

    // Stem export (per manual p. 127: 2-MIDI with empty device_chain is skipped,
    // while 1-MIDI, 3-Audio, 4-Audio, A-Reverb, B-Delay, and Master/Main are exported).
    let stems =
        OfflineBouncer::render_stems(&project, &stems_dir, opts).expect("Failed to render stems");
    assert_eq!(stems.len(), 6);
    assert!(stems.iter().all(|s| !s.contains("2-MIDI")));
    for s in stems {
        assert!(Path::new(&s).exists());
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

#[test]
fn devices_json_is_an_object_with_an_array_of_hosts() {
    let output = livecraft_devices(&["--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let list: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the output is JSON");
    assert!(list["hosts"].is_array(), "no hosts array in {list}");
}

#[test]
fn devices_text_has_an_outputs_block_for_each_host_in_the_json() {
    let json = livecraft_devices(&["--json"]);
    let list: serde_json::Value = serde_json::from_slice(&json.stdout).expect("the output is JSON");
    let hosts = list["hosts"].as_array().expect("a hosts array").len();

    let text = livecraft_devices(&[]);
    assert!(
        text.status.success(),
        "{}",
        String::from_utf8_lossy(&text.stderr)
    );
    let text = String::from_utf8_lossy(&text.stdout);
    assert_eq!(
        text.lines().filter(|line| *line == "  Outputs:").count(),
        hosts
    );
}

#[test]
fn devices_json_lists_the_midi_input_and_output_ports() {
    let output = livecraft_devices(&["--json"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let list: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the output is JSON");
    assert!(
        list["midi"]["inputs"].is_array(),
        "no MIDI inputs in {list}"
    );
    assert!(
        list["midi"]["outputs"].is_array(),
        "no MIDI outputs in {list}"
    );
}

#[test]
fn render_help_documents_all_live_export_audio_options() {
    let output = run_livecraft(&["render", "--help"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "--output",
        "--duration-bars",
        "--start-bars",
        "--start-beats",
        "--length-beats",
        "--track",
        "--selected-tracks",
        "--include-returns-main",
        "--render-as-loop",
        "--mono",
        "--normalize",
        "--analysis-file",
        "--sample-rate",
        "--source-sample-rate",
        "--no-pcm",
        "--format",
        "--bits",
        "--dither",
        "--mp3",
        "--no-report",
        "wav",
        "aiff",
        "flac",
        "none",
        "rectangular",
        "triangular",
        "pow-r-1",
        "pow-r-2",
        "pow-r-3",
    ] {
        assert!(
            help.contains(expected),
            "render --help missing '{expected}':\n{help}"
        );
    }
}

#[test]
fn stems_help_documents_all_stem_export_options() {
    let output = run_livecraft(&["stems", "--help"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let help = String::from_utf8_lossy(&output.stdout);
    for expected in [
        "--output-dir",
        "--duration-bars",
        "--sample-rate",
        "--bits",
        "--format",
        "--dither",
        "--include-returns-main",
        "--mono",
        "--normalize",
        "--render-as-loop",
        "--no-report",
    ] {
        assert!(
            help.contains(expected),
            "stems --help missing '{expected}':\n{help}"
        );
    }
}

#[test]
fn cli_render_wav_writes_audio_and_matching_report_json_sha256() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );

    let dir = temp_cli_dir("wav_report");
    let project_path = write_project_json(&dir, "song.json", &chord_project("WavReport"));
    let out_wav = dir.join("mix.wav");

    let output = run_livecraft(&[
        "render",
        project_path.to_str().expect("utf8 path"),
        "-o",
        out_wav.to_str().expect("utf8 path"),
        "-d",
        "1",
        "-b",
        "24",
        "--dither",
        "triangular",
    ]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out_wav.exists());

    let reader = hound::WavReader::open(&out_wav).expect("open WAV");
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.spec().sample_rate, 44100);
    assert_eq!(reader.spec().bits_per_sample, 24);
    assert_eq!(reader.duration(), 88200);

    let report_path = dir.join("mix.wav.report.json");
    assert!(report_path.exists());
    let report_json = fs::read_to_string(&report_path).expect("read report json");
    let report: RenderReport = serde_json::from_str(&report_json).expect("parse RenderReport");
    assert_eq!(report.outputs.len(), 1);
    assert_eq!(report.outputs[0].format, "wav");
    assert_eq!(report.outputs[0].bit_depth, 24);
    assert_eq!(report.outputs[0].sample_rate, 44100);
    assert_eq!(report.outputs[0].channels, 2);
    assert_eq!(report.outputs[0].frames, 88200);
    assert_eq!(report.settings.dither, DitherMode::Triangular);

    let wav_bytes = fs::read(&out_wav).expect("read wav bytes");
    let expected_sha = sha256_hex(&wav_bytes);
    assert_eq!(report.outputs[0].sha256, expected_sha);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&expected_sha),
        "stdout missing sha256:\n{stdout}"
    );
    assert!(
        stdout.contains("2.000s"),
        "stdout missing duration:\n{stdout}"
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_render_aiff_and_flac_formats() {
    let dir = temp_cli_dir("aiff_flac");
    let project_path = write_project_json(&dir, "song.json", &chord_project("FormatTest"));
    let out_aiff = dir.join("mix.aiff");
    let out_flac = dir.join("mix.flac");

    let aiff_out = run_livecraft(&[
        "render",
        project_path.to_str().expect("utf8"),
        "-o",
        out_aiff.to_str().expect("utf8"),
        "-d",
        "1",
        "-f",
        "aiff",
        "-b",
        "16",
        "--dither",
        "pow-r-2",
    ]);
    assert!(
        aiff_out.status.success(),
        "{}",
        String::from_utf8_lossy(&aiff_out.stderr)
    );
    let aiff_bytes = fs::read(&out_aiff).expect("read aiff");
    assert!(aiff_bytes.len() > 12);
    assert_eq!(&aiff_bytes[0..4], b"FORM");
    assert_eq!(&aiff_bytes[8..12], b"AIFF");
    let aiff_report: RenderReport = serde_json::from_str(
        &fs::read_to_string(dir.join("mix.aiff.report.json")).expect("read aiff report"),
    )
    .expect("parse aiff report");
    assert_eq!(aiff_report.outputs[0].format, "aiff");
    assert_eq!(aiff_report.outputs[0].bit_depth, 16);
    assert_eq!(aiff_report.settings.dither, DitherMode::PowR2);
    assert_eq!(aiff_report.outputs[0].sha256, sha256_hex(&aiff_bytes));

    let flac_out = run_livecraft(&[
        "render",
        project_path.to_str().expect("utf8"),
        "-o",
        out_flac.to_str().expect("utf8"),
        "-d",
        "1",
        "-f",
        "flac",
        "-b",
        "24",
        "--dither",
        "pow-r-1",
    ]);
    assert!(
        flac_out.status.success(),
        "{}",
        String::from_utf8_lossy(&flac_out.stderr)
    );
    let flac_bytes = fs::read(&out_flac).expect("read flac");
    assert!(flac_bytes.len() > 4);
    assert_eq!(&flac_bytes[0..4], b"fLaC");
    let flac_report: RenderReport = serde_json::from_str(
        &fs::read_to_string(dir.join("mix.flac.report.json")).expect("read flac report"),
    )
    .expect("parse flac report");
    assert_eq!(flac_report.outputs[0].format, "flac");
    assert_eq!(flac_report.outputs[0].bit_depth, 24);
    assert_eq!(flac_report.settings.dither, DitherMode::PowR1);
    assert_eq!(flac_report.outputs[0].sha256, sha256_hex(&flac_bytes));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_render_mp3_no_pcm_writes_only_mp3_and_report() {
    let dir = temp_cli_dir("mp3_only");
    let project_path = write_project_json(&dir, "song.json", &chord_project("Mp3Only"));
    let out_wav = dir.join("mix.wav");
    let out_mp3 = dir.join("mix.mp3");

    let output = run_livecraft(&[
        "render",
        project_path.to_str().expect("utf8"),
        "-o",
        out_wav.to_str().expect("utf8"),
        "-d",
        "1",
        "--mp3",
        "--no-pcm",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !out_wav.exists(),
        "PCM output should not exist with --no-pcm"
    );
    assert!(out_mp3.exists(), "MP3 output must be created");

    let mp3_bytes = fs::read(&out_mp3).expect("read mp3");
    assert!(!mp3_bytes.is_empty());

    let report: RenderReport = serde_json::from_str(
        &fs::read_to_string(dir.join("mix.wav.report.json")).expect("read mp3 report"),
    )
    .expect("parse mp3 report");
    assert_eq!(report.outputs.len(), 1);
    assert_eq!(report.outputs[0].format, "mp3");
    assert_eq!(report.outputs[0].bit_depth, 0);
    assert!(!report.settings.encode_pcm);
    assert!(report.settings.encode_mp3);
    assert_eq!(report.outputs[0].sha256, sha256_hex(&mp3_bytes));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_render_mono_normalize_and_analysis_file() {
    let dir = temp_cli_dir("mono_norm_analysis");
    let mut project = chord_project("MonoNormAnalysis");
    project.main_track.device_chain.clear();
    project.tracks[0].volume_db = -18.0;
    project.tracks[0].pan = -1.0;
    let project_path = write_project_json(&dir, "song.json", &project);
    let out_wav = dir.join("mono_norm.wav");

    let output = run_livecraft(&[
        "render",
        project_path.to_str().expect("utf8"),
        "-o",
        out_wav.to_str().expect("utf8"),
        "-d",
        "1",
        "-b",
        "32",
        "--mono",
        "--normalize",
        "--analysis-file",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let reader = hound::WavReader::open(&out_wav).expect("open mono wav");
    assert_eq!(reader.spec().channels, 1);
    assert_eq!(reader.spec().bits_per_sample, 32);
    assert_eq!(reader.spec().sample_format, hound::SampleFormat::Float);
    let samples: Vec<f32> = reader
        .into_samples::<f32>()
        .map(|s| s.expect("sample"))
        .collect();
    assert_eq!(samples.len(), 88200);
    let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    assert!(
        (peak - 1.0).abs() < 1e-4,
        "expected normalized peak ~1.0, got {peak}"
    );

    let analysis_path = dir.join("mono_norm.wav.lcanalysis");
    assert!(analysis_path.exists());
    let analysis: AnalysisFile =
        serde_json::from_str(&fs::read_to_string(&analysis_path).expect("read analysis"))
            .expect("parse AnalysisFile");
    assert_eq!(analysis.version, ANALYSIS_FILE_VERSION);
    assert_eq!(analysis.sample_rate, 44100);
    assert_eq!(analysis.channels, 1);
    assert_eq!(analysis.frames, 88200);
    assert_eq!(
        analysis.overview_block_frames,
        ANALYSIS_OVERVIEW_BLOCK_FRAMES as u32
    );
    assert_eq!(analysis.peak_overview.len(), 88200usize.div_ceil(256));

    let report: RenderReport = serde_json::from_str(
        &fs::read_to_string(dir.join("mono_norm.wav.report.json")).expect("read report"),
    )
    .expect("parse report");
    assert!((report.outputs[0].metrics.peak_dbfs - 0.0).abs() < 1e-3);
    assert_eq!(
        report.outputs[0].sha256,
        sha256_hex(&fs::read(&out_wav).expect("read wav"))
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_render_beats_range_resampling_loop_and_no_report() {
    let dir = temp_cli_dir("range_resample_loop");

    // Arrangement clip on beats 4..8, rendered at 48 kHz and resampled to 44.1 kHz, with --no-report.
    let mut subrange = Project::new_default("Subrange");
    subrange.main_track.device_chain.clear();
    let mut clip = Clip::new_midi(1, "Bar2", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
    }
    subrange.tracks[0]
        .arrangement_clips
        .push(ArrangementClip::new(10.into(), clip, 4.0, 4.0));
    let subrange_json = write_project_json(&dir, "subrange.json", &subrange);
    let subrange_wav = dir.join("subrange.wav");

    let sub_out = run_livecraft(&[
        "render",
        subrange_json.to_str().expect("utf8"),
        "-o",
        subrange_wav.to_str().expect("utf8"),
        "--start-beats",
        "4",
        "--length-beats",
        "4",
        "-s",
        "44100",
        "--source-sample-rate",
        "48000",
        "-b",
        "32",
        "--no-report",
    ]);
    assert!(
        sub_out.status.success(),
        "{}",
        String::from_utf8_lossy(&sub_out.stderr)
    );
    assert!(!dir.join("subrange.wav.report.json").exists());
    let reader = hound::WavReader::open(&subrange_wav).expect("open subrange wav");
    assert_eq!(reader.spec().sample_rate, 44100);
    assert_eq!(reader.duration(), 88200);
    let sub_samples: Vec<f32> = reader
        .into_samples::<f32>()
        .map(|s| s.expect("s"))
        .collect();
    assert!(sub_samples.iter().any(|&s| s.abs() > 0.01));

    // Render as Loop wraps Delay tail onto start.
    let mut loop_proj = Project::new_default("LoopCli");
    loop_proj.main_track.device_chain.clear();
    let mut late_clip = Clip::new_midi(2, "LateHit", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = late_clip.payload {
        mdata.notes.push(MidiNote::new(60, 2.0, 1.5, 110));
    }
    loop_proj.tracks[0].clip_slots[0].clip = Some(late_clip);
    let mut delay = Device::new(DeviceId(901), "Delay", DeviceType::Delay);
    delay.set_param("time_ms", 500.0);
    delay.set_param("feedback", 0.5);
    delay.set_param("dry_wet", 0.8);
    loop_proj.tracks[0].device_chain.push(delay);
    let loop_json = write_project_json(&dir, "loop.json", &loop_proj);
    let plain_wav = dir.join("plain.wav");
    let looped_wav = dir.join("looped.wav");

    let plain_out = run_livecraft(&[
        "render",
        loop_json.to_str().expect("utf8"),
        "-o",
        plain_wav.to_str().expect("utf8"),
        "-d",
        "1",
        "-b",
        "32",
        "--no-report",
    ]);
    assert!(plain_out.status.success());
    let looped_out = run_livecraft(&[
        "render",
        loop_json.to_str().expect("utf8"),
        "-o",
        looped_wav.to_str().expect("utf8"),
        "-d",
        "1",
        "-b",
        "32",
        "--render-as-loop",
        "--no-report",
    ]);
    assert!(looped_out.status.success());

    let plain_samples: Vec<f32> = hound::WavReader::open(&plain_wav)
        .expect("open plain")
        .into_samples::<f32>()
        .map(|s| s.expect("s"))
        .collect();
    let looped_samples: Vec<f32> = hound::WavReader::open(&looped_wav)
        .expect("open looped")
        .into_samples::<f32>()
        .map(|s| s.expect("s"))
        .collect();
    let head = 44100 * 2;
    assert!(plain_samples[..head].iter().all(|&s| s == 0.0));
    assert!(looped_samples[..head].iter().any(|&s| s.abs() > 1e-4));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_render_track_selection_and_stems_command() {
    let dir = temp_cli_dir("track_and_stems");
    let project_path = write_project_json(&dir, "song.json", &chord_project("TrackStems"));

    // Selected tracks via --track selected --selected-tracks 1,A-Reverb
    let selected_dir = dir.join("selected_stems");
    let sel_out = run_livecraft(&[
        "render",
        project_path.to_str().expect("utf8"),
        "-o",
        selected_dir.to_str().expect("utf8"),
        "-d",
        "1",
        "--track",
        "selected",
        "--selected-tracks",
        "1,A-Reverb",
        "--include-returns-main",
    ]);
    assert!(
        sel_out.status.success(),
        "{}",
        String::from_utf8_lossy(&sel_out.stderr)
    );
    let sel_report: RenderReport = serde_json::from_str(
        &fs::read_to_string(selected_dir.join("render.report.json")).expect("read selected report"),
    )
    .expect("parse selected report");
    assert_eq!(sel_report.outputs.len(), 2);
    for out in &sel_report.outputs {
        let bytes = fs::read(&out.path).expect("read selected stem");
        assert_eq!(out.sha256, sha256_hex(&bytes));
    }

    // Single track by name
    let single_wav = dir.join("track1.wav");
    let single_out = run_livecraft(&[
        "render",
        project_path.to_str().expect("utf8"),
        "-o",
        single_wav.to_str().expect("utf8"),
        "-d",
        "1",
        "--track",
        "1-MIDI",
        "--no-report",
    ]);
    assert!(
        single_out.status.success(),
        "{}",
        String::from_utf8_lossy(&single_out.stderr)
    );
    assert!(single_wav.exists());

    // Stems subcommand with full export options
    let stems_dir = dir.join("all_stems");
    let stems_out = run_livecraft(&[
        "stems",
        project_path.to_str().expect("utf8"),
        "-o",
        stems_dir.to_str().expect("utf8"),
        "-d",
        "1",
        "-s",
        "44100",
        "-b",
        "16",
        "-f",
        "flac",
        "--dither",
        "rectangular",
        "--include-returns-main",
        "--mono",
        "--normalize",
        "--render-as-loop",
    ]);
    assert!(
        stems_out.status.success(),
        "{}",
        String::from_utf8_lossy(&stems_out.stderr)
    );
    let stems_report: RenderReport = serde_json::from_str(
        &fs::read_to_string(stems_dir.join("render.report.json")).expect("read stems report"),
    )
    .expect("parse stems report");
    assert_eq!(stems_report.outputs.len(), 6);
    assert!(
        stems_report
            .outputs
            .iter()
            .all(|o| !o.path.contains("2-MIDI") && o.format == "flac" && o.channels == 1)
    );
    for out in &stems_report.outputs {
        let bytes = fs::read(&out.path).expect("read stem flac");
        assert_eq!(&bytes[0..4], b"fLaC");
        assert_eq!(out.sha256, sha256_hex(&bytes));
    }

    let _ = fs::remove_dir_all(dir);
}

const UNKNOWN_XML_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12117" SchemaChangeCount="10" Creator="LiveCraft Fixture 1.0" Revision="fixture-rev">
	<LiveSet>
		<NextPointeeId Value="5000" />
		<LomId Value="0" />
		<CustomUnknownRootTag Zebra="last" KeepMe='{"raw": [1, 2, 3]}' Alpha="first" />
		<!-- Preserve root comment -->
		<Tracks>
			<MidiTrack Id="12" SelectedToolPanel="7">
				<LomId Value="0" />
				<Name>
					<EffectiveName Value="1-MIDI" />
					<UserName Value="" />
				</Name>
				<Color Value="19" />
				<TrackGroupId Value="-1" />
				<TrackDelay Value="0" />
				<DeviceChain>
					<Mixer>
						<Sends>
							<TrackSendHolder Id="0">
								<Send>
									<Manual Value="0.0003162277571" />
								</Send>
							</TrackSendHolder>
						</Sends>
						<Speaker>
							<Manual Value="true" />
						</Speaker>
						<SoloSink Value="false" />
						<Pan>
							<Manual Value="0" />
						</Pan>
						<Volume>
							<Manual Value="1" />
						</Volume>
					</Mixer>
					<MainSequencer>
						<ClipSlotList>
							<ClipSlot Id="0">
								<ClipSlot>
									<Value />
								</ClipSlot>
								<HasStop Value="true" />
							</ClipSlot>
						</ClipSlotList>
					</MainSequencer>
					<DeviceChain>
						<Devices>
							<Operator Id="300">
								<UserName Value="Lead Op" />
								<On>
									<Manual Value="true" />
								</On>
								<FilterCutoff>
									<Manual Value="2400" />
								</FilterCutoff>
							</Operator>
							<MxDeviceAudioEffect Id="301">
								<UserName Value="M4L Shimmer" />
								<On>
									<Manual Value="true" />
								</On>
								<PatcherBlob><![CDATA[<maxpat>{opaque_data}</maxpat>]]></PatcherBlob>
							</MxDeviceAudioEffect>
						</Devices>
					</DeviceChain>
				</DeviceChain>
			</MidiTrack>
		</Tracks>
		<MainTrack>
			<Name>
				<EffectiveName Value="Main" />
				<UserName Value="" />
			</Name>
			<Color Value="4" />
			<TrackGroupId Value="-1" />
			<DeviceChain>
				<Mixer>
					<Speaker>
						<Manual Value="true" />
					</Speaker>
					<SoloSink Value="false" />
					<Pan>
						<Manual Value="0" />
					</Pan>
					<Volume>
						<Manual Value="1" />
					</Volume>
					<Tempo>
						<Manual Value="120" />
					</Tempo>
					<TimeSignature>
						<Manual Value="201" />
					</TimeSignature>
				</Mixer>
				<DeviceChain>
					<Devices />
				</DeviceChain>
			</DeviceChain>
		</MainTrack>
		<Scenes>
			<Scene Id="0">
				<Name Value="" />
				<Color Value="-1" />
			</Scene>
		</Scenes>
		<Transport>
			<LoopOn Value="false" />
			<LoopStart Value="0" />
			<LoopLength Value="16" />
			<CurrentTime Value="0" />
		</Transport>
		<GlobalQuantisation Value="4" />
		<ScaleInformation>
			<Root Value="0" />
			<Name Value="0" />
		</ScaleInformation>
		<InKey Value="false" />
		<TuningSystems />
	</LiveSet>
</Ableton>
"#;

#[test]
fn cli_convert_pretty_xml_roundtrip_preserves_unknowns_and_orders_attrs() {
    let dir = temp_cli_dir("convert_pretty_xml");
    let in_als = dir.join("input.als");
    let pretty_xml = dir.join("pretty.xml");
    let roundtrip_als = dir.join("roundtrip.als");
    let re_pretty_xml = dir.join("re_pretty.xml");
    let json_path = dir.join("exported.json");

    AlsDocument::from_xml(UNKNOWN_XML_TEMPLATE)
        .expect("parse fixture xml")
        .save_als(&in_als)
        .expect("save input.als");

    // 1. .als -> .xml with --pretty-xml
    let conv_xml = run_livecraft(&[
        "convert",
        "--pretty-xml",
        in_als.to_str().expect("utf8"),
        pretty_xml.to_str().expect("utf8"),
    ]);
    assert!(
        conv_xml.status.success(),
        "{}",
        String::from_utf8_lossy(&conv_xml.stderr)
    );
    let xml_text = fs::read_to_string(&pretty_xml).expect("read pretty.xml");
    // Attributes sorted alphabetically: Alpha, KeepMe, Zebra
    assert!(
        xml_text.contains(
            r#"<CustomUnknownRootTag Alpha="first" KeepMe='{"raw": [1, 2, 3]}' Zebra="last" />"#
        ),
        "expected deterministic attribute order in pretty XML:\n{xml_text}"
    );
    assert!(xml_text.contains("<!-- Preserve root comment -->"));
    assert!(xml_text.contains(r#"<TrackDelay Value="0" />"#));
    assert!(xml_text.contains(r#"<MxDeviceAudioEffect Id="301">"#));
    assert!(
        xml_text.contains("<PatcherBlob><![CDATA[<maxpat>{opaque_data}</maxpat>]]></PatcherBlob>")
    );

    // 2. .xml -> .als preserves unknown XML elements & M4L device
    let conv_als = run_livecraft(&[
        "convert",
        pretty_xml.to_str().expect("utf8"),
        roundtrip_als.to_str().expect("utf8"),
    ]);
    assert!(
        conv_als.status.success(),
        "{}",
        String::from_utf8_lossy(&conv_als.stderr)
    );
    let roundtrip_xml = decompress_als_or_xml(&roundtrip_als).expect("decompress roundtrip.als");
    assert_eq!(roundtrip_xml, xml_text);

    // 3. Re-running --pretty-xml from roundtrip.als is byte-identical
    let conv_again = run_livecraft(&[
        "convert",
        "--pretty-xml",
        roundtrip_als.to_str().expect("utf8"),
        re_pretty_xml.to_str().expect("utf8"),
    ]);
    assert!(conv_again.status.success());
    let re_xml_text = fs::read_to_string(&re_pretty_xml).expect("read re_pretty.xml");
    assert_eq!(re_xml_text, xml_text);

    // 4. .xml -> .json also succeeds
    let conv_json = run_livecraft(&[
        "convert",
        pretty_xml.to_str().expect("utf8"),
        json_path.to_str().expect("utf8"),
    ]);
    assert!(
        conv_json.status.success(),
        "{}",
        String::from_utf8_lossy(&conv_json.stderr)
    );
    let proj: Project =
        serde_json::from_str(&fs::read_to_string(&json_path).expect("read json")).expect("json");
    assert_eq!(proj.tracks[0].device_chain.len(), 2);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_diff_reports_tempo_track_clip_note_and_device_param_changes() {
    let dir = temp_cli_dir("diff_cmd");
    let path_a = dir.join("a.als");
    let path_b = dir.join("b.als");
    let path_b_xml = dir.join("b.xml");

    let mut proj_a = chord_project("DiffA");
    proj_a.transport.set_bpm(120.0);
    AlsDocument::from_project(&proj_a)
        .expect("from_project A")
        .save_als(&path_a)
        .expect("save a.als");

    let mut proj_b = proj_a.clone();
    proj_b.transport.set_bpm(136.0);
    proj_b.tracks[0].name = "Lead Synth".to_owned();
    proj_b.tracks[0].volume_db = -5.5;
    proj_b.tracks[0].device_chain[0].set_param("filter_cutoff", 950.0);
    if let Some(ref mut clip) = proj_b.tracks[0].clip_slots[0].clip
        && let ClipPayload::Midi(ref mut m) = clip.payload
    {
        m.notes.remove(2); // remove G3
        m.notes.push(MidiNote::new(72, 2.0, 1.0, 115)); // add C4
    }
    let doc_b = AlsDocument::from_project(&proj_b).expect("from_project B");
    doc_b.save_als(&path_b).expect("save b.als");
    fs::write(&path_b_xml, doc_b.to_pretty_xml().expect("pretty xml B")).expect("write b.xml");

    // Human-readable diff
    let text_diff = run_livecraft(&[
        "diff",
        path_a.to_str().expect("utf8"),
        path_b.to_str().expect("utf8"),
    ]);
    assert!(
        text_diff.status.success(),
        "{}",
        String::from_utf8_lossy(&text_diff.stderr)
    );
    let stdout = String::from_utf8_lossy(&text_diff.stdout);
    assert!(
        stdout.contains("120.00 BPM -> 136.00 BPM"),
        "stdout:\n{stdout}"
    );
    assert!(stdout.contains("1-MIDI -> Lead Synth"), "stdout:\n{stdout}");
    assert!(stdout.contains("-5.50 dB"), "stdout:\n{stdout}");
    assert!(stdout.contains("+ note C4 @ 2"), "stdout:\n{stdout}");
    assert!(stdout.contains("- note G3 @ 0"), "stdout:\n{stdout}");
    assert!(stdout.contains("param filter_cutoff"), "stdout:\n{stdout}");

    // JSON diff (cross-format .als vs .xml)
    let json_diff = run_livecraft(&[
        "diff",
        "--json",
        path_a.to_str().expect("utf8"),
        path_b_xml.to_str().expect("utf8"),
    ]);
    assert!(
        json_diff.status.success(),
        "{}",
        String::from_utf8_lossy(&json_diff.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&json_diff.stdout).expect("parse diff JSON");
    assert_eq!(parsed["bpm_changed"], serde_json::json!([120.0, 136.0]));
    let track_mod = parsed["tracks_modified"]
        .as_array()
        .expect("tracks_modified array");
    assert_eq!(track_mod.len(), 1);
    assert_eq!(
        track_mod[0]["renamed"],
        serde_json::json!(["1-MIDI", "Lead Synth"])
    );
    assert_eq!(
        track_mod[0]["clips_modified"][0]["notes_added"],
        serde_json::json!([[72, 2.0]])
    );
    assert_eq!(
        track_mod[0]["clips_modified"][0]["notes_removed"],
        serde_json::json!([[67, 0.0]])
    );
    assert_eq!(
        track_mod[0]["devices_modified"][0]["params_changed"][0]["key"],
        serde_json::json!("filter_cutoff")
    );

    // Identical diff reports no changes
    let same_diff = run_livecraft(&[
        "diff",
        path_a.to_str().expect("utf8"),
        path_a.to_str().expect("utf8"),
    ]);
    assert!(same_diff.status.success());
    assert_eq!(
        String::from_utf8_lossy(&same_diff.stdout).trim(),
        "No musical changes."
    );

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn cli_loads_and_inspects_all_five_fixture_sets() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let sets_dir = root.join("fixtures").join("sets");

    let expected_sets = [
        ("empty.als", 120.0, 4),
        ("midi_operator.als", 124.0, 4),
        ("audio_clip.als", 120.0, 4),
        ("sends_returns_automation.als", 118.0, 4),
        ("group_tracks.als", 122.0, 5),
    ];

    for (filename, expected_bpm, expected_tracks) in expected_sets {
        let path = sets_dir.join(filename);
        assert!(path.exists(), "missing fixture set: {}", path.display());
        let path_str = path.to_str().expect("utf8");

        let info = run_livecraft(&["info", path_str]);
        assert!(
            info.status.success(),
            "info failed for {filename}: {}",
            String::from_utf8_lossy(&info.stderr)
        );
        let info_out = String::from_utf8_lossy(&info.stdout);
        assert!(
            info_out.contains(&format!("{expected_bpm:.1} BPM")),
            "info output for {filename} missing BPM:\n{info_out}"
        );

        let inspect = run_livecraft(&["inspect", path_str, "--devices", "--clips"]);
        assert!(
            inspect.status.success(),
            "inspect failed for {filename}: {}",
            String::from_utf8_lossy(&inspect.stderr)
        );

        let inspect_json = run_livecraft(&["inspect", path_str, "--json"]);
        assert!(
            inspect_json.status.success(),
            "inspect --json failed for {filename}: {}",
            String::from_utf8_lossy(&inspect_json.stderr)
        );
        let proj: Project =
            serde_json::from_slice(&inspect_json.stdout).expect("parse inspect --json");
        assert!(
            proj.validate().is_empty(),
            "{filename} validation errors: {:?}",
            proj.validate()
        );
        assert_eq!(proj.tracks.len(), expected_tracks, "{filename} tracks");

        // Zero-difference XML round-trip check
        let original_xml = decompress_als_or_xml(&path).expect("decompress fixture");
        let mut doc = AlsDocument::open(&path).expect("open fixture");
        let unchanged = doc.project().clone();
        doc.apply_project(&unchanged).expect("apply unchanged");
        assert_eq!(
            doc.to_xml().expect("to_xml"),
            original_xml,
            "{filename} zero-diff XML round-trip"
        );
    }
}
