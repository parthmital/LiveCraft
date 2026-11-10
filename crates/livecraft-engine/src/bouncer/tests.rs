use super::report::sha256_file;
use super::*;
use livecraft_dsp::metrics::relative_residual_rms_dbfs;
use livecraft_formats::als::AlsParser;
use livecraft_model::ids::DeviceId;
use livecraft_model::model::{ArrangementClip, Clip, ClipPayload, Device, DeviceType, MidiNote};

fn chord_project(name: &str) -> Project {
    let mut project = Project::new_default(name);
    let mut clip = Clip::new_midi(1, "Lead Chords", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
        mdata.notes.push(MidiNote::new(64, 0.0, 2.0, 100));
        mdata.notes.push(MidiNote::new(67, 0.0, 2.0, 100));
    }
    project.tracks[0].clip_slots[0].clip = Some(clip);
    project
}

fn temp_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("livecraft_m08_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create test dir");
    dir
}

fn read_wav_f32(path: &Path) -> Vec<f32> {
    hound::WavReader::open(path)
        .expect("open wav")
        .samples::<f32>()
        .map(|s| s.expect("sample"))
        .collect()
}

fn base_opts(duration_bars: u32, bits_per_sample: u16) -> RenderOptions {
    RenderOptions {
        sample_rate: 44100,
        duration_bars,
        bits_per_sample,
        ..RenderOptions::default()
    }
}

fn assert_metrics_close(actual: &AudioMetrics, expected: &AudioMetrics) {
    assert!((actual.peak_dbfs - expected.peak_dbfs).abs() < 1e-12);
    assert!((actual.true_peak_dbtp - expected.true_peak_dbtp).abs() < 1e-12);
    assert!((actual.rms_dbfs - expected.rms_dbfs).abs() < 1e-12);
    assert!((actual.integrated_lufs - expected.integrated_lufs).abs() < 1e-12);
    assert!((actual.short_term_max_lufs - expected.short_term_max_lufs).abs() < 1e-12);
    assert!((actual.momentary_max_lufs - expected.momentary_max_lufs).abs() < 1e-12);
}

#[test]
fn a_bounce_writes_a_stereo_wav_at_the_requested_rate() {
    let project = chord_project("BounceTest");
    let out_dir = temp_test_dir("stereo_wav");
    let wav_path = out_dir.join("test_bounce.wav");

    OfflineBouncer::render_project(&project, &wav_path, base_opts(2, 16))
        .expect("Failed to render project");
    assert!(wav_path.exists());

    let reader = hound::WavReader::open(&wav_path).expect("Failed to read rendered WAV");
    assert_eq!(reader.spec().channels, 2);
    assert_eq!(reader.spec().sample_rate, 44100);
    assert!(reader.len() > 0);

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn two_offline_renders_are_byte_identical_across_wav_aiff_flac_and_mp3() {
    let project = chord_project("RepeatAllFormats");
    let out_dir = temp_test_dir("byte_identical_all");

    let cases = [
        ("wav16.wav", AudioFileFormat::Wav, 16, true, false),
        ("wav24.wav", AudioFileFormat::Wav, 24, true, false),
        ("wav32.wav", AudioFileFormat::Wav, 32, true, false),
        ("aiff24.aiff", AudioFileFormat::Aiff, 24, true, false),
        ("aiff32.aiff", AudioFileFormat::Aiff, 32, true, false),
        ("flac16.flac", AudioFileFormat::Flac, 16, true, false),
        ("mp3.mp3", AudioFileFormat::Wav, 24, false, true),
    ];

    for (file_name, file_type, bits, encode_pcm, encode_mp3) in cases {
        let a = out_dir.join(format!("a_{file_name}"));
        let b = out_dir.join(format!("b_{file_name}"));
        let opts = RenderOptions {
            file_type,
            encode_pcm,
            encode_mp3,
            ..base_opts(1, bits)
        };
        OfflineBouncer::render_project(&project, &a, opts.clone()).expect("render a");
        OfflineBouncer::render_project(&project, &b, opts).expect("render b");

        let bytes_a = fs::read(&a).expect("read a");
        let bytes_b = fs::read(&b).expect("read b");
        assert!(!bytes_a.is_empty(), "{file_name} empty");
        assert_eq!(bytes_a, bytes_b, "{file_name} differed between runs");
    }

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn rejects_render_when_both_pcm_and_mp3_are_disabled() {
    let project = chord_project("NoCodec");
    let out_dir = temp_test_dir("no_codec");
    let opts = RenderOptions {
        encode_pcm: false,
        encode_mp3: false,
        ..RenderOptions::default()
    };
    let err = OfflineBouncer::export(&project, out_dir.join("out.wav"), opts)
        .expect_err("must reject when neither PCM nor MP3 is enabled");
    assert!(err.to_string().contains("encode_pcm"));
    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn float32_wav_and_aiff_preserve_peaks_above_one_without_clamping_or_dither() {
    let mut project = chord_project("HotFloat");
    project.main_track.device_chain.clear();
    project.tracks[0].volume_db = 6.0;
    let mut boost = Device::new(DeviceId(801), "Utility", DeviceType::Utility);
    boost.set_param("gain_db", 12.0);
    project.tracks[0].device_chain.push(boost);

    let out_dir = temp_test_dir("hot_float");
    let wav_path = out_dir.join("hot.wav");
    let aiff_path = out_dir.join("hot.aiff");

    let opts_wav_a = RenderOptions {
        dither: DitherMode::PowR3,
        ..base_opts(1, 32)
    };
    let opts_wav_b = RenderOptions {
        dither: DitherMode::NoDither,
        ..opts_wav_a.clone()
    };

    let wav_b = out_dir.join("hot_nodither.wav");
    OfflineBouncer::render_project(&project, &wav_path, opts_wav_a).expect("wav 32f");
    OfflineBouncer::render_project(&project, &wav_b, opts_wav_b).expect("wav 32f nodither");
    assert_eq!(
        fs::read(&wav_path).expect("read"),
        fs::read(&wav_b).expect("read"),
        "32-bit float must ignore dither options"
    );

    let samples = read_wav_f32(&wav_path);
    let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    assert!(
        peak > 1.2,
        "expected unclamped float peak > 1.2, got {peak}"
    );

    let opts_aiff = RenderOptions {
        file_type: AudioFileFormat::Aiff,
        ..base_opts(1, 32)
    };
    OfflineBouncer::render_project(&project, &aiff_path, opts_aiff).expect("aiff 32f");
    let aiff_bytes = fs::read(&aiff_path).expect("read aiff");
    assert_eq!(&aiff_bytes[0..4], b"FORM");
    assert_eq!(&aiff_bytes[8..12], b"AIFC");
    assert!(aiff_bytes.windows(4).any(|w| w == b"fl32"));

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn all_dither_modes_work_deterministically_for_16_and_24_bit() {
    let mut project = chord_project("DitherTest");
    project.main_track.device_chain.clear();
    project.tracks[0].volume_db = -48.0;

    let out_dir = temp_test_dir("dither_modes");
    let mut rendered_16 = Vec::new();
    for mode in DitherMode::ALL {
        let p16 = out_dir.join(format!("out_{mode:?}_16.wav"));
        let p24 = out_dir.join(format!("out_{mode:?}_24.wav"));
        let opts16 = RenderOptions {
            dither: mode,
            ..base_opts(1, 16)
        };
        let opts24 = RenderOptions {
            bits_per_sample: 24,
            ..opts16.clone()
        };
        OfflineBouncer::render_project(&project, &p16, opts16).expect("render 16");
        OfflineBouncer::render_project(&project, &p24, opts24).expect("render 24");
        rendered_16.push(fs::read(&p16).expect("read 16"));
        assert!(!fs::read(&p24).expect("read 24").is_empty());
    }
    assert_ne!(
        rendered_16[0], rendered_16[2],
        "NoDither and Triangular must differ at 16-bit"
    );
    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn normalize_brings_peak_to_unity_and_leaves_silence_silent() {
    let mut project = chord_project("Normalize");
    project.main_track.device_chain.clear();
    project.tracks[0].volume_db = -18.0;

    let out_dir = temp_test_dir("normalize");
    let norm_path = out_dir.join("norm.wav");
    let opts = RenderOptions {
        normalize: true,
        ..base_opts(1, 32)
    };
    let report = OfflineBouncer::render_project_with_report(&project, &norm_path, opts.clone())
        .expect("render normalized");
    assert!(
        (report.outputs[0].metrics.peak_dbfs - 0.0).abs() < 1e-4,
        "normalized peak should be 0 dBFS, got {}",
        report.outputs[0].metrics.peak_dbfs
    );

    let samples = read_wav_f32(&norm_path);
    let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    assert!((peak - 1.0).abs() < 1e-5, "expected peak 1.0, got {peak}");

    let silent = Project::new_default("SilentNorm");
    let silent_path = out_dir.join("silent.wav");
    OfflineBouncer::render_project(&silent, &silent_path, opts).expect("silent normalize");
    assert!(read_wav_f32(&silent_path).iter().all(|&s| s == 0.0));

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn convert_to_mono_outputs_single_channel_half_sum() {
    let mut project = chord_project("Mono");
    project.main_track.device_chain.clear();
    project.tracks[0].pan = -1.0;

    let out_dir = temp_test_dir("mono");
    let stereo_path = out_dir.join("stereo.wav");
    let mono_path = out_dir.join("mono.wav");

    let stereo_opts = base_opts(1, 32);
    let mono_opts = RenderOptions {
        convert_to_mono: true,
        ..stereo_opts.clone()
    };

    OfflineBouncer::render_project(&project, &stereo_path, stereo_opts).expect("stereo");
    OfflineBouncer::render_project(&project, &mono_path, mono_opts).expect("mono");

    assert_eq!(
        hound::WavReader::open(&stereo_path)
            .expect("open")
            .spec()
            .channels,
        2
    );
    assert_eq!(
        hound::WavReader::open(&mono_path)
            .expect("open")
            .spec()
            .channels,
        1
    );

    let stereo = read_wav_f32(&stereo_path);
    let mono = read_wav_f32(&mono_path);
    assert_eq!(mono.len(), stereo.len() / 2);

    for (i, frame) in stereo.as_chunks::<2>().0.iter().enumerate() {
        let expected = (frame[0] + frame[1]) * 0.5;
        assert!((mono[i] - expected).abs() < 1e-6);
    }

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn render_as_loop_wraps_delay_tail_onto_beginning() {
    let mut project = Project::new_default("LoopTail");
    project.main_track.device_chain.clear();
    let mut clip = Clip::new_midi(1, "LateHit", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 2.0, 1.5, 110));
    }
    project.tracks[0].clip_slots[0].clip = Some(clip);
    let mut delay = Device::new(DeviceId(901), "Delay", DeviceType::Delay);
    delay.set_param("time_ms", 500.0);
    delay.set_param("feedback", 0.5);
    delay.set_param("dry_wet", 0.8);
    project.tracks[0].device_chain.push(delay);

    let out_dir = temp_test_dir("loop_tail");
    let plain_path = out_dir.join("plain.wav");
    let looped_path = out_dir.join("looped.wav");

    let plain_opts = base_opts(1, 32);
    let loop_opts = RenderOptions {
        render_as_loop: true,
        ..plain_opts.clone()
    };

    OfflineBouncer::render_project(&project, &plain_path, plain_opts).expect("plain");
    OfflineBouncer::render_project(&project, &looped_path, loop_opts).expect("looped");

    let plain = read_wav_f32(&plain_path);
    let looped = read_wav_f32(&looped_path);
    assert_eq!(plain.len(), looped.len());

    let head_samples = 44100 * 2;
    assert!(plain[..head_samples].iter().all(|&s| s == 0.0));
    assert!(
        looped[..head_samples].iter().any(|&s| s.abs() > 1e-4),
        "render_as_loop should wrap Pass 1's post-end tail into the start of Pass 2"
    );

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn stem_export_handles_dry_vs_wet_groups_returns_selected_and_skips_empty_midi() {
    let mut project = Project::new_default("StemsM08");
    let mut clip1 = Clip::new_midi(1, "Lead", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip1.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
    }
    project.tracks[0].clip_slots[0].clip = Some(clip1);
    project.tracks[0].sends = vec![1.0, 0.0];

    assert!(project.tracks[1].device_chain.is_empty());

    let mut fx_midi = Track::new(TrackId(8), "MidiWithFx", TrackType::Midi, 4);
    fx_midi
        .device_chain
        .push(Device::new(DeviceId(802), "Utility", DeviceType::Utility));
    project.tracks.push(fx_midi);

    let group_id = TrackId(9);
    let group = Track::new(group_id, "DrumBus", TrackType::Group, 0);
    project.tracks[2].parent = Some(group_id);
    project.tracks.push(group);

    project.main_track.device_chain =
        vec![Device::new(DeviceId(803), "Utility", DeviceType::Utility)];
    project.main_track.device_chain[0].set_param("gain_db", -12.0);

    let out_dir = temp_test_dir("stems_full");
    let dry_dir = out_dir.join("dry");
    let wet_dir = out_dir.join("wet");

    let dry_report = OfflineBouncer::export(
        &project,
        &dry_dir,
        RenderOptions {
            rendered_track: RenderedTrack::AllIndividualTracks,
            sample_rate: 44100,
            duration_bars: 1,
            bits_per_sample: 32,
            include_return_and_main_effects: false,
            ..RenderOptions::default()
        },
    )
    .expect("dry stems");

    assert_eq!(dry_report.outputs.len(), 8);
    assert!(
        dry_report
            .outputs
            .iter()
            .all(|o| !o.path.contains("2-MIDI")),
        "empty MIDI track must be skipped"
    );

    let fx_midi_out = dry_report
        .outputs
        .iter()
        .find(|o| o.path.contains("MidiWithFx"))
        .expect("MidiWithFx exported");
    assert!(fx_midi_out.metrics.peak_dbfs.is_infinite());

    let wet_report = OfflineBouncer::export(
        &project,
        &wet_dir,
        RenderOptions {
            rendered_track: RenderedTrack::AllIndividualTracks,
            sample_rate: 44100,
            duration_bars: 1,
            bits_per_sample: 32,
            include_return_and_main_effects: true,
            ..RenderOptions::default()
        },
    )
    .expect("wet stems");

    let dry_t1_peak = dry_report.outputs[0].metrics.peak_dbfs;
    let wet_t1_peak = wet_report.outputs[0].metrics.peak_dbfs;
    assert!(
        dry_t1_peak > wet_t1_peak + 6.0,
        "wet stem passes through -12 dB Main Utility (dry={dry_t1_peak}, wet={wet_t1_peak})"
    );

    let sel_dir = out_dir.join("selected");
    let sel_report = OfflineBouncer::export(
        &project,
        &sel_dir,
        RenderOptions {
            rendered_track: RenderedTrack::SelectedTracks(vec![
                TrackId(1),
                project.return_tracks[0].id,
            ]),
            sample_rate: 44100,
            duration_bars: 1,
            bits_per_sample: 24,
            include_return_and_main_effects: true,
            ..RenderOptions::default()
        },
    )
    .expect("selected");
    assert_eq!(sel_report.outputs.len(), 2);
    assert!(sel_report.outputs[1].metrics.peak_dbfs > -60.0);

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn rubato_downsampling_renders_at_source_rate_and_resamples_to_target_rate() {
    let project = chord_project("Resample");
    let out_dir = temp_test_dir("rubato");
    let resampled_path = out_dir.join("downsampled.wav");

    let opts = RenderOptions {
        source_sample_rate: Some(96000),
        ..base_opts(1, 32)
    };
    let report = OfflineBouncer::render_project_with_report(&project, &resampled_path, opts)
        .expect("resample render");
    assert_eq!(report.outputs[0].sample_rate, 44100);
    assert_eq!(report.outputs[0].frames, 88200);
    assert!(report.outputs[0].metrics.peak_dbfs > -40.0);

    let reader = hound::WavReader::open(&resampled_path).expect("open resampled wav");
    assert_eq!(reader.spec().sample_rate, 44100);

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn tempo_and_meter_maps_compute_exact_frame_counts() {
    let mut project = chord_project("TempoMeter");
    project
        .transport
        .time_signature
        .insert(0, 7, 8)
        .expect("7/8");
    project.transport.tempo.insert(Beats(3.5), 60.0, false);

    let out_dir = temp_test_dir("tempo_meter");
    let out_path = out_dir.join("meter.wav");
    let report = OfflineBouncer::render_project_with_report(&project, &out_path, base_opts(2, 24))
        .expect("render");
    assert_eq!(report.outputs[0].frames, 231_525);

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn create_analysis_file_writes_valid_sidecar_with_256_frame_peak_overview() {
    let project = chord_project("Analysis");
    let out_dir = temp_test_dir("analysis");
    let wav_path = out_dir.join("mix.wav");
    let opts = RenderOptions {
        create_analysis_file: true,
        ..base_opts(1, 24)
    };
    OfflineBouncer::render_project(&project, &wav_path, opts).expect("render");

    let analysis_path = out_dir.join("mix.wav.lcanalysis");
    assert!(analysis_path.exists());
    let json = fs::read_to_string(&analysis_path).expect("read analysis");
    let parsed: AnalysisFile = serde_json::from_str(&json).expect("parse analysis");

    assert_eq!(parsed.version, ANALYSIS_FILE_VERSION);
    assert_eq!(parsed.sample_rate, 44100);
    assert_eq!(parsed.channels, 2);
    assert_eq!(parsed.frames, 88200);
    assert_eq!(parsed.overview_block_frames, 256);
    assert_eq!(parsed.peak_overview.len(), 88200usize.div_ceil(256));
    assert!(parsed.metrics.peak_dbfs > -40.0);

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn render_report_writes_json_with_matching_sha256_and_roundtrips_silent_metrics() {
    let project = chord_project("Report");
    let out_dir = temp_test_dir("report");
    let wav_path = out_dir.join("song.wav");

    let opts = RenderOptions {
        encode_mp3: true,
        write_report: true,
        ..base_opts(1, 24)
    };
    let report =
        OfflineBouncer::render_project_with_report(&project, &wav_path, opts).expect("render");
    assert_eq!(report.outputs.len(), 2);
    assert_eq!(report.livecraft_version, env!("CARGO_PKG_VERSION"));
    assert!(!report.git_commit.is_empty());

    let report_path = out_dir.join("song.wav.report.json");
    assert!(report_path.exists());
    let json = fs::read_to_string(&report_path).expect("read report");
    let parsed: RenderReport = serde_json::from_str(&json).expect("parse report");
    assert_eq!(parsed.livecraft_version, report.livecraft_version);
    assert_eq!(parsed.git_commit, report.git_commit);
    assert_eq!(parsed.settings, report.settings);
    assert_eq!(parsed.outputs.len(), report.outputs.len());
    for (actual, expected) in parsed.outputs.iter().zip(&report.outputs) {
        assert_eq!(actual.path, expected.path);
        assert_eq!(actual.format, expected.format);
        assert_eq!(actual.bit_depth, expected.bit_depth);
        assert_eq!(actual.sample_rate, expected.sample_rate);
        assert_eq!(actual.channels, expected.channels);
        assert_eq!(actual.frames, expected.frames);
        assert_eq!(actual.sha256, expected.sha256);
        assert_metrics_close(&actual.metrics, &expected.metrics);
    }

    for output in &parsed.outputs {
        assert_eq!(output.sha256.len(), 64);
        assert!(
            output
                .sha256
                .chars()
                .all(|c| matches!(c, '0'..='9' | 'a'..='f'))
        );
        let recomputed = sha256_file(Path::new(&output.path)).expect("recompute hash");
        assert_eq!(output.sha256, recomputed);
    }

    let silent = Project::new_default("SilentReport");
    let silent_wav = out_dir.join("silent.wav");
    let silent_report = OfflineBouncer::render_project_with_report(
        &silent,
        &silent_wav,
        RenderOptions {
            sample_rate: 44100,
            duration_bars: 1,
            write_report: true,
            ..RenderOptions::default()
        },
    )
    .expect("silent report");
    let silent_json =
        fs::read_to_string(out_dir.join("silent.wav.report.json")).expect("read silent");
    let parsed_silent: RenderReport =
        serde_json::from_str(&silent_json).expect("roundtrip silent report");
    assert_eq!(parsed_silent, silent_report);
    assert!(parsed_silent.outputs[0].metrics.peak_dbfs.is_infinite());

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn metronome_is_excluded_from_offline_renders() {
    let mut project = Project::new_default("NoClick");
    project.transport.metronome_enabled = true;
    let out_dir = temp_test_dir("no_click");
    let wav_path = out_dir.join("noclick.wav");

    OfflineBouncer::render_project(&project, &wav_path, base_opts(1, 32)).expect("render");
    assert!(read_wav_f32(&wav_path).iter().all(|&s| s == 0.0));

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn arrangement_render_start_and_length_beats_isolate_subrange() {
    let mut project = Project::new_default("Subrange");
    project.main_track.device_chain.clear();
    let mut clip = Clip::new_midi(1, "Late", 4.0);
    if let ClipPayload::Midi(ref mut mdata) = clip.payload {
        mdata.notes.push(MidiNote::new(60, 0.0, 2.0, 100));
    }
    project.tracks[0]
        .arrangement_clips
        .push(ArrangementClip::new(10.into(), clip, 4.0, 4.0));

    let out_dir = temp_test_dir("subrange");
    let bar1_path = out_dir.join("bar1.wav");
    let bar2_path = out_dir.join("bar2.wav");

    OfflineBouncer::render_project(&project, &bar1_path, base_opts(1, 32)).expect("bar 1");
    OfflineBouncer::render_project(
        &project,
        &bar2_path,
        RenderOptions {
            render_start_beats: Some(4.0),
            render_length_beats: Some(4.0),
            ..base_opts(1, 32)
        },
    )
    .expect("bar 2");

    assert!(read_wav_f32(&bar1_path).iter().all(|&s| s == 0.0));
    assert!(read_wav_f32(&bar2_path).iter().any(|&s| s.abs() > 0.01));

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn render_stems_exports_all_individual_non_empty_tracks() {
    let project = chord_project("RenderStemsApi");
    let out_dir = temp_test_dir("render_stems_api");
    let stems =
        OfflineBouncer::render_stems(&project, &out_dir, base_opts(1, 16)).expect("render stems");

    assert_eq!(stems.len(), 6);
    for stem in &stems {
        assert!(Path::new(stem).exists());
    }

    let _ = fs::remove_dir_all(out_dir);
}

#[test]
fn oracle_fixtures_render_deterministically_and_match_analytical_linear_math() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let oracle_dir = workspace_root.join("fixtures/oracle");
    let out_dir = temp_test_dir("oracle_self_consistency");

    let load_oracle = |file: &str| -> Project {
        let mut project = AlsParser::load_project(oracle_dir.join(file)).expect("load oracle als");
        for track in &mut project.tracks {
            for clip in track
                .clip_slots
                .iter_mut()
                .filter_map(|s| s.clip.as_mut())
                .chain(track.arrangement_clips.iter_mut().map(|a| &mut a.clip))
            {
                if let ClipPayload::Audio(ref mut audio) = clip.payload
                    && !audio.file_path.is_empty()
                    && Path::new(&audio.file_path).is_relative()
                {
                    audio.file_path = workspace_root
                        .join(&audio.file_path)
                        .to_string_lossy()
                        .into_owned();
                }
            }
        }
        project
    };

    let render_twice = |name: &str, project: &Project| -> Vec<f32> {
        let opts = base_opts(1, 32);
        let path_a = out_dir.join(format!("{name}_a.wav"));
        let path_b = out_dir.join(format!("{name}_b.wav"));
        OfflineBouncer::render_project(project, &path_a, opts.clone()).expect("render a");
        OfflineBouncer::render_project(project, &path_b, opts).expect("render b");
        assert_eq!(
            fs::read(&path_a).expect("read a"),
            fs::read(&path_b).expect("read b"),
            "{name} must be byte-identical across runs"
        );
        let samples = read_wav_f32(&path_a);
        assert!(
            samples.iter().any(|&s| s.abs() > 0.01),
            "{name} must not be silent"
        );
        samples
    };

    let source = crate::audio_clip::load_file(&oracle_dir.join("audio/sine_1khz.wav"))
        .expect("load sine_1khz.wav");
    let unity_proj = load_oracle("sine_unity.als");
    let unity = render_twice("sine_unity", &unity_proj);
    let center_gain = std::f32::consts::FRAC_PI_4.cos();
    let expected_unity: Vec<f32> = source
        .samples_l
        .iter()
        .zip(&source.samples_r)
        .flat_map(|(&l, &r)| [l * center_gain, r * center_gain])
        .collect();
    assert!(
        relative_residual_rms_dbfs(&expected_unity, &unity) <= -110.0,
        "sine_unity residual too high"
    );

    let panned_proj = load_oracle("panned_sine.als");
    let panned = render_twice("panned_sine", &panned_proj);
    let pan_angle = (0.5_f32 + 1.0) * 0.5 * std::f32::consts::FRAC_PI_2;
    let (pan_l, pan_r) = (pan_angle.cos(), pan_angle.sin());
    let expected_panned: Vec<f32> = source
        .samples_l
        .iter()
        .zip(&source.samples_r)
        .flat_map(|(&l, &r)| [l * pan_l, r * pan_r])
        .collect();
    assert!(
        relative_residual_rms_dbfs(&expected_panned, &panned) <= -110.0,
        "panned_sine residual too high"
    );

    let summed_proj = load_oracle("two_tracks_summed.als");
    let summed = render_twice("two_tracks_summed", &summed_proj);
    let expected_summed: Vec<f32> = unity.iter().map(|&s| 2.0 * s).collect();
    assert!(
        relative_residual_rms_dbfs(&expected_summed, &summed) <= -110.0,
        "two_tracks_summed residual too high"
    );

    let send_proj = load_oracle("send_to_return_neg6db.als");
    let send_level = send_proj.tracks[0].sends[0];
    assert!((send_level - 10.0f32.powf(-6.0 / 20.0)).abs() < 1e-5);
    let sent = render_twice("send_to_return_neg6db", &send_proj);
    let expected_sent: Vec<f32> = unity.iter().map(|&s| (1.0 + send_level) * s).collect();
    assert!(
        relative_residual_rms_dbfs(&expected_sent, &sent) <= -110.0,
        "send_to_return_neg6db residual too high"
    );

    let _ = fs::remove_dir_all(out_dir);
}
