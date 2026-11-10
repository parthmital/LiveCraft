use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use livecraft_formats::als::read::decompress_als_or_xml;
use livecraft_formats::als::{AlsDocument, AlsParser};
use livecraft_formats::analysis_cache::{
    ANALYSIS_VERSION, AnalysisCacheLocation, CachedSampleAnalysis, PeakFrame, analysis_cache_path,
};
use livecraft_formats::audio_decode::probe_audio_info;
use livecraft_formats::project_dir::create_project_layout;
use livecraft_model::clip::{WarpMarker, WarpMode};
use livecraft_model::ids::ClipId;
use livecraft_model::model::{Clip, ClipPayload, Project};

fn run_livecraft(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_livecraft"))
        .args(args)
        .output()
        .expect("run the livecraft binary")
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("test path is valid UTF-8")
}

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("audio")
}

fn test_dir(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("m11_cli")
        .join(name);
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create m11_cli test dir");
    dir
}

fn make_audio_set(name: &str, sample_path: &Path, relative_path: Option<&str>) -> Project {
    let mut project = Project::new_default(name);
    project.tracks[2].device_chain.clear();
    project.main_track.device_chain.clear();
    let mut clip = Clip::new_audio(
        ClipId(901),
        "AudioFixture",
        sample_path.to_string_lossy().into_owned(),
        4.0,
    );
    if let ClipPayload::Audio(ref mut audio) = clip.payload {
        audio.relative_path = relative_path.map(str::to_owned);
        audio.is_warped = false;
    }
    project.tracks[2].clip_slots[0].clip = Some(clip);
    project
}

fn write_tone_wav(path: &Path, sample_rate: u32, channels: u16, frames: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create wav parent dir");
    }
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    for i in 0..frames {
        let s = ((i as f32) * 440.0 * std::f32::consts::TAU / (sample_rate as f32)).sin() * 0.5;
        for _ in 0..channels {
            writer.write_sample(s).expect("write sample");
        }
    }
    writer.finalize().expect("finalize wav");
}

#[test]
fn analyze_formats_with_cache_dir_no_write_and_cache_reuse() {
    let dir = test_dir("analyze_cache_dir");
    let no_write_dir = dir.join("no_write_cache");
    let cache_dir = dir.join("cache");
    let fixtures = fixtures_dir();

    for name in [
        "sine_440.flac",
        "sine_440.ogg",
        "sine_440.mp3",
        "sine_440_alac.m4a",
        "sine_440_aac.m4a",
    ] {
        let src = fixtures.join(name);
        let probed = probe_audio_info(&src).expect("probe audio fixture");

        // 1. --no-write builds analysis in memory without writing a .lcanalysis file.
        let no_write_out = run_livecraft(&[
            "analyze",
            path_str(&src),
            "--cache-dir",
            path_str(&no_write_dir),
            "--no-write",
            "--json",
        ]);
        assert!(
            no_write_out.status.success(),
            "analyze --no-write failed for {name}: {}",
            String::from_utf8_lossy(&no_write_out.stderr)
        );
        let no_write_json: serde_json::Value =
            serde_json::from_slice(&no_write_out.stdout).expect("parse --no-write JSON");
        let frames = no_write_json["frames"].as_u64().expect("frames in JSON");
        assert!(frames > 0, "{name} frames should be > 0");
        assert_eq!(no_write_json["sample_rate"], probed.sample_rate);
        assert_eq!(no_write_json["channels"], probed.channels);
        assert_eq!(no_write_json["cache_exists"], false);
        assert_eq!(no_write_json["write_cache"], false);
        assert_eq!(no_write_json["pyramid_blocks"]["256"], frames.div_ceil(256));
        assert_eq!(
            no_write_json["pyramid_blocks"]["4096"],
            frames.div_ceil(4096)
        );
        assert_eq!(
            no_write_json["pyramid_blocks"]["65536"],
            frames.div_ceil(65536)
        );
        let missing_cache =
            analysis_cache_path(&src, &AnalysisCacheLocation::Folder(no_write_dir.clone()));
        assert!(
            !missing_cache.exists(),
            "--no-write must not create {missing_cache:?}"
        );

        // 2. With --cache-dir (no --no-write), .lcanalysis is created in cache_dir.
        let json_out = run_livecraft(&[
            "analyze",
            path_str(&src),
            "--cache-dir",
            path_str(&cache_dir),
            "--json",
        ]);
        assert!(
            json_out.status.success(),
            "analyze failed for {name}: {}",
            String::from_utf8_lossy(&json_out.stderr)
        );
        let parsed: serde_json::Value =
            serde_json::from_slice(&json_out.stdout).expect("parse analyze JSON");
        let expected_cache =
            analysis_cache_path(&src, &AnalysisCacheLocation::Folder(cache_dir.clone()));
        assert!(
            expected_cache.is_file(),
            "expected cache file {}",
            expected_cache.display()
        );
        assert_eq!(parsed["cache_exists"], true);
        assert_eq!(
            PathBuf::from(parsed["cache_path"].as_str().expect("cache_path")),
            expected_cache
        );

        let mut cached: CachedSampleAnalysis =
            serde_json::from_slice(&fs::read(&expected_cache).expect("read written .lcanalysis"))
                .expect("parse CachedSampleAnalysis");
        assert_eq!(cached.version, ANALYSIS_VERSION);
        assert_eq!(cached.sample_rate, 44_100);
        assert_eq!(cached.channels, 2);
        assert_eq!(cached.frames, frames);
        assert_eq!(cached.peak_overview.len(), frames.div_ceil(256) as usize);
        assert_eq!(cached.overview_4096.len(), frames.div_ceil(4096) as usize);
        assert_eq!(cached.overview_65536.len(), frames.div_ceil(65536) as usize);

        // 3. Inject sentinel into cached file and re-run in text mode to prove cache reuse.
        let sentinel = PeakFrame {
            min: -0.1234,
            max: 0.4321,
        };
        cached.peak_overview[0] = sentinel;
        fs::write(
            &expected_cache,
            serde_json::to_vec_pretty(&cached).expect("serialize sentinel"),
        )
        .expect("write sentinel cache");

        let text_out = run_livecraft(&[
            "analyze",
            path_str(&src),
            "--cache-dir",
            path_str(&cache_dir),
        ]);
        assert!(
            text_out.status.success(),
            "analyze text failed for {name}: {}",
            String::from_utf8_lossy(&text_out.stderr)
        );
        let stdout = String::from_utf8_lossy(&text_out.stdout);
        for expected_substr in [
            "44100 Hz",
            &frames.to_string(),
            &format!("256 -> {}", frames.div_ceil(256)),
            &format!("4096 -> {}", frames.div_ceil(4096)),
            &format!("65536 -> {}", frames.div_ceil(65536)),
            &expected_cache.to_string_lossy(),
        ] {
            assert!(
                stdout.contains(expected_substr),
                "{name} text stdout missing '{expected_substr}':\n{stdout}"
            );
        }

        let after_reuse: CachedSampleAnalysis = serde_json::from_slice(
            &fs::read(&expected_cache).expect("re-read .lcanalysis after reuse"),
        )
        .expect("parse reused CachedSampleAnalysis");
        assert_eq!(
            after_reuse.peak_overview[0], sentinel,
            "{name} second analyze run must reuse valid cache without overwriting"
        );
    }
}

#[test]
fn analyze_beside_sample_creates_and_reuses_sidecar_and_preserves_asd() {
    let dir = test_dir("analyze_beside_sample");
    let fixtures = fixtures_dir();

    for name in [
        "sine_440.flac",
        "sine_440.ogg",
        "sine_440.mp3",
        "sine_440_aac.m4a",
        "sine_440_alac.m4a",
    ] {
        let sample_copy = dir.join(name);
        fs::copy(fixtures.join(name), &sample_copy).expect("copy audio fixture");

        let mut asd_os = sample_copy.as_os_str().to_os_string();
        asd_os.push(".asd");
        let asd_path = PathBuf::from(asd_os);
        fs::write(&asd_path, b"ABLETON-ASD-DO-NOT-TOUCH").expect("write dummy .asd");

        let json_out = run_livecraft(&["analyze", path_str(&sample_copy), "--json"]);
        assert!(
            json_out.status.success(),
            "analyze beside sample failed for {name}: {}",
            String::from_utf8_lossy(&json_out.stderr)
        );
        let parsed: serde_json::Value =
            serde_json::from_slice(&json_out.stdout).expect("parse beside-sample JSON");

        let sidecar = analysis_cache_path(&sample_copy, &AnalysisCacheLocation::BesideSample);
        assert!(sidecar.is_file(), "sidecar missing for {name}");
        assert_eq!(parsed["cache_exists"], true);
        assert_eq!(
            PathBuf::from(parsed["cache_path"].as_str().expect("cache_path")),
            sidecar
        );

        let mut cached: CachedSampleAnalysis =
            serde_json::from_slice(&fs::read(&sidecar).expect("read sidecar"))
                .expect("parse sidecar");
        let sentinel = PeakFrame {
            min: -0.25,
            max: 0.75,
        };
        cached.peak_overview[0] = sentinel;
        fs::write(
            &sidecar,
            serde_json::to_vec_pretty(&cached).expect("serialize sentinel"),
        )
        .expect("write sentinel");

        let reuse_out = run_livecraft(&["analyze", path_str(&sample_copy)]);
        assert!(reuse_out.status.success());
        let reused: CachedSampleAnalysis =
            serde_json::from_slice(&fs::read(&sidecar).expect("read reused sidecar"))
                .expect("parse reused sidecar");
        assert_eq!(reused.peak_overview[0], sentinel);
        assert_eq!(
            fs::read(&asd_path).expect("read .asd"),
            b"ABLETON-ASD-DO-NOT-TOUCH"
        );
    }

    // Missing and invalid audio files fail cleanly.
    let missing_out = run_livecraft(&["analyze", path_str(&dir.join("missing.flac"))]);
    assert!(!missing_out.status.success());

    let invalid_path = dir.join("not_audio.txt");
    fs::write(&invalid_path, b"not valid audio data").expect("write invalid file");
    let invalid_out = run_livecraft(&["analyze", path_str(&invalid_path)]);
    assert!(!invalid_out.status.success());
}

#[test]
fn render_compressed_and_lossless_formats_produces_non_silent_deterministic_wav() {
    let dir = test_dir("render_formats");
    let fixtures = fixtures_dir();

    for name in [
        "sine_440.flac",
        "sine_440.ogg",
        "sine_440.mp3",
        "sine_440_alac.m4a",
        "sine_440_aac.m4a",
    ] {
        let sample_path = fixtures.join(name);
        let project = make_audio_set(&format!("Render_{name}"), &sample_path, None);
        let als_path = dir.join(format!("{name}.als"));
        AlsDocument::from_project(&project)
            .expect("build als doc")
            .save_als(&als_path)
            .expect("save als");

        let wav_a = dir.join(format!("{name}_a.wav"));
        let wav_b = dir.join(format!("{name}_b.wav"));

        for out_wav in [&wav_a, &wav_b] {
            let render_out = run_livecraft(&[
                "render",
                path_str(&als_path),
                "-o",
                path_str(out_wav),
                "-d",
                "1",
                "-b",
                "32",
                "--no-report",
            ]);
            assert!(
                render_out.status.success(),
                "render failed for {name}: {}",
                String::from_utf8_lossy(&render_out.stderr)
            );
        }

        let bytes_a = fs::read(&wav_a).expect("read wav_a");
        let bytes_b = fs::read(&wav_b).expect("read wav_b");
        assert_eq!(
            bytes_a, bytes_b,
            "{name} CLI render must produce byte-identical deterministic WAV output"
        );

        let reader = hound::WavReader::open(&wav_a).expect("open rendered WAV");
        assert_eq!(reader.spec().channels, 2, "{name} channels");
        assert_eq!(reader.spec().sample_rate, 44_100, "{name} sample rate");
        let samples: Vec<f32> = reader
            .into_samples::<f32>()
            .map(|s| s.expect("decode rendered f32 sample"))
            .collect();
        assert_eq!(
            samples.len(),
            88_200 * 2,
            "{name} 1-bar stereo sample count"
        );
        let peak = samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(
            peak > 0.02,
            "{name} rendered WAV should be non-silent (peak={peak})"
        );
    }
}

#[test]
fn command_run_clip_set_ram_mode_and_replace_sample_roundtrip_als() {
    let list_out = run_livecraft(&["command", "list"]);
    assert!(list_out.status.success());
    let list_stdout = String::from_utf8_lossy(&list_out.stdout);
    let names: Vec<&str> = list_stdout.lines().collect();
    assert!(names.contains(&"clip.set_ram_mode"));
    assert!(names.contains(&"clip.replace_sample"));

    let dir = test_dir("command_m11");
    let proj_root = dir.join("ClipCmd Project");
    create_project_layout(&proj_root).expect("create project layout");

    let wav_a = proj_root.join("Samples/Imported/tone_a.wav");
    let wav_b_mono_48k = proj_root.join("Samples/Imported/tone_b_48k.wav");
    write_tone_wav(&wav_a, 44_100, 2, 44_100);
    write_tone_wav(&wav_b_mono_48k, 48_000, 1, 48_000);

    let mut project = make_audio_set("ClipCmd", &wav_a, Some("Samples/Imported/tone_a.wav"));
    {
        let clip = project.tracks[2].clip_slots[0]
            .clip
            .as_mut()
            .expect("audio clip");
        clip.loop_start_beat = 0.5;
        clip.loop_end_beat = 3.5;
        clip.is_looping = true;
        clip.start_marker_beat = 0.25;
        clip.end_marker_beat = 3.75;
        if let ClipPayload::Audio(ref mut audio) = clip.payload {
            audio.gain_db = -3.0;
            audio.pitch_cents = 200;
            audio.warp_mode = WarpMode::Complex;
            audio.is_warped = true;
            audio.ram_mode = false;
            audio.warp_markers = vec![
                WarpMarker {
                    sample_time_sec: 0.0,
                    beat_time: 0.0,
                },
                WarpMarker {
                    sample_time_sec: 0.5,
                    beat_time: 1.0,
                },
            ];
        }
    }

    let set_0 = proj_root.join("Set_0.als");
    AlsDocument::from_project(&project)
        .expect("build Set_0 als")
        .save_als(&set_0)
        .expect("save Set_0 als");

    // 1. clip.set_ram_mode -> enabled: true
    let set_ram_on = proj_root.join("Set_ram_on.als");
    let ram_on_out = run_livecraft(&[
        "command",
        "run",
        path_str(&set_0),
        "clip.set_ram_mode",
        r#"{"clip": 901, "enabled": true}"#,
        "-o",
        path_str(&set_ram_on),
    ]);
    assert!(
        ram_on_out.status.success(),
        "clip.set_ram_mode true failed: {}",
        String::from_utf8_lossy(&ram_on_out.stderr)
    );
    let xml_ram_on = decompress_als_or_xml(&set_ram_on).expect("decompress ram_on.als");
    assert!(xml_ram_on.contains(r#"<Ram Value="true""#), "{xml_ram_on}");

    let loaded_ram_on = AlsParser::load_project(&set_ram_on).expect("load Set_ram_on.als");
    let clip_ram_on = loaded_ram_on.tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip in Set_ram_on");
    match &clip_ram_on.payload {
        ClipPayload::Audio(audio) => {
            assert!(audio.ram_mode);
            assert_eq!(audio.warp_markers.len(), 2);
            assert_eq!(audio.pitch_cents, 200);
            assert!((audio.gain_db - (-3.0)).abs() < 1e-2);
        }
        other => panic!("expected audio clip, got {other:?}"),
    }

    // 2. clip.set_ram_mode -> enabled: false
    let set_ram_off = proj_root.join("Set_ram_off.als");
    let ram_off_out = run_livecraft(&[
        "command",
        "run",
        path_str(&set_ram_on),
        "clip.set_ram_mode",
        r#"{"clip": 901, "enabled": false}"#,
        "-o",
        path_str(&set_ram_off),
    ]);
    assert!(
        ram_off_out.status.success(),
        "clip.set_ram_mode false failed: {}",
        String::from_utf8_lossy(&ram_off_out.stderr)
    );
    let xml_ram_off = decompress_als_or_xml(&set_ram_off).expect("decompress ram_off.als");
    assert!(
        xml_ram_off.contains(r#"<Ram Value="false""#),
        "{xml_ram_off}"
    );
    let loaded_ram_off = AlsParser::load_project(&set_ram_off).expect("load Set_ram_off.als");
    match &loaded_ram_off.tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip")
        .payload
    {
        ClipPayload::Audio(audio) => assert!(!audio.ram_mode),
        other => panic!("expected audio clip, got {other:?}"),
    }

    // 3. clip.replace_sample with keep_warp_markers: true (preserving clip settings & RAM mode)
    let set_replaced_keep = proj_root.join("Set_replaced_keep.als");
    let replace_keep_args = serde_json::json!({
        "clip": 901,
        "new_path": wav_b_mono_48k.to_string_lossy(),
        "new_relative_path": "Samples/Imported/tone_b_48k.wav",
        "sample_rate": 48_000,
        "channels": 1,
        "keep_warp_markers": true
    })
    .to_string();
    let replace_keep_out = run_livecraft(&[
        "command",
        "run",
        path_str(&set_ram_on),
        "clip.replace_sample",
        &replace_keep_args,
        "-o",
        path_str(&set_replaced_keep),
    ]);
    assert!(
        replace_keep_out.status.success(),
        "clip.replace_sample keep failed: {}",
        String::from_utf8_lossy(&replace_keep_out.stderr)
    );
    let loaded_keep =
        AlsParser::load_project(&set_replaced_keep).expect("load Set_replaced_keep.als");
    let clip_keep = loaded_keep.tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip in Set_replaced_keep");
    assert_eq!(clip_keep.loop_start_beat, 0.5);
    assert_eq!(clip_keep.loop_end_beat, 3.5);
    assert!(clip_keep.is_looping);
    assert_eq!(clip_keep.start_marker_beat, 0.25);
    assert_eq!(clip_keep.end_marker_beat, 3.75);
    match &clip_keep.payload {
        ClipPayload::Audio(audio) => {
            assert_eq!(PathBuf::from(&audio.file_path), wav_b_mono_48k);
            assert_eq!(
                audio.relative_path.as_deref(),
                Some("Samples/Imported/tone_b_48k.wav")
            );
            assert_eq!(audio.sample_rate, 48_000);
            assert_eq!(audio.channels, 1);
            assert_eq!(audio.warp_markers.len(), 2);
            assert!(audio.ram_mode);
            assert_eq!(audio.warp_mode, WarpMode::Complex);
            assert!(audio.is_warped);
            assert_eq!(audio.pitch_cents, 200);
            assert!((audio.gain_db - (-3.0)).abs() < 1e-2);
        }
        other => panic!("expected audio clip, got {other:?}"),
    }

    // 4. clip.replace_sample with keep_warp_markers: false targeting fixtures/audio/sine_440.flac
    let flac_fixture = fixtures_dir().join("sine_440.flac");
    let set_replaced_flac = proj_root.join("Set_replaced_flac.als");
    let replace_clear_args = serde_json::json!({
        "clip": 901,
        "new_path": flac_fixture.to_string_lossy(),
        "new_relative_path": null,
        "sample_rate": 44_100,
        "channels": 2,
        "keep_warp_markers": false
    })
    .to_string();
    let replace_clear_out = run_livecraft(&[
        "command",
        "run",
        path_str(&set_replaced_keep),
        "clip.replace_sample",
        &replace_clear_args,
        "-o",
        path_str(&set_replaced_flac),
    ]);
    assert!(
        replace_clear_out.status.success(),
        "clip.replace_sample clear failed: {}",
        String::from_utf8_lossy(&replace_clear_out.stderr)
    );
    let loaded_clear =
        AlsParser::load_project(&set_replaced_flac).expect("load Set_replaced_flac.als");
    let clip_clear = loaded_clear.tracks[2].clip_slots[0]
        .clip
        .as_ref()
        .expect("clip in Set_replaced_flac");
    match &clip_clear.payload {
        ClipPayload::Audio(audio) => {
            assert_eq!(PathBuf::from(&audio.file_path), flac_fixture);
            assert_eq!(audio.relative_path, None);
            assert_eq!(audio.sample_rate, 44_100);
            assert_eq!(audio.channels, 2);
            assert!(audio.warp_markers.is_empty());
            assert!(audio.ram_mode);
            assert_eq!(audio.warp_mode, WarpMode::Complex);
            assert_eq!(audio.pitch_cents, 200);
            assert!((audio.gain_db - (-3.0)).abs() < 1e-2);
        }
        other => panic!("expected audio clip, got {other:?}"),
    }

    // Rendering the round-tripped RAM-mode FLAC .als produces non-silent WAV output.
    let rendered_wav = dir.join("replaced_flac.wav");
    let render_out = run_livecraft(&[
        "render",
        path_str(&set_replaced_flac),
        "-o",
        path_str(&rendered_wav),
        "-d",
        "1",
        "-b",
        "32",
        "--no-report",
    ]);
    assert!(
        render_out.status.success(),
        "render after clip.replace_sample failed: {}",
        String::from_utf8_lossy(&render_out.stderr)
    );
    let rendered_samples: Vec<f32> = hound::WavReader::open(&rendered_wav)
        .expect("open rendered WAV")
        .into_samples::<f32>()
        .map(|s| s.expect("sample"))
        .collect();
    assert!(
        rendered_samples.iter().any(|&s| s.abs() > 0.02),
        "rendered replaced FLAC clip should be non-silent"
    );

    // 5. Missing clip ID fails cleanly without writing output.
    let unused_out = proj_root.join("should_not_exist.als");
    let missing_clip_out = run_livecraft(&[
        "command",
        "run",
        path_str(&set_0),
        "clip.set_ram_mode",
        r#"{"clip": 99999, "enabled": true}"#,
        "-o",
        path_str(&unused_out),
    ]);
    assert!(!missing_clip_out.status.success());
    assert!(!unused_out.exists());
}
