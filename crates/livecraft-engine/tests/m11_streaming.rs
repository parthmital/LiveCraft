//! M11 integration and verification tests: real-time safety (`assert_no_alloc`), 1-second
//! preload, `MAX_WAV_SAMPLES` RAM limit vs unlimited disk streaming, `rubato` sample-rate
//! conversion, `DecodeQueue` via `basedrop`, `disk_load` & `disk_underruns` telemetry, deterministic
//! `OfflineBouncer` rendering across formats, and 1-hour 96 kHz stereo streaming verification.

use basedrop::{Collector, Shared};
use livecraft_engine::audio_clip::{DecodedAudio, MAX_WAV_SAMPLES, load_file};
use livecraft_engine::bouncer::{OfflineBouncer, RenderOptions};
use livecraft_engine::compiled::MAX_BLOCK;
use livecraft_engine::compiler::GraphCompiler;
use livecraft_engine::decode_queue::DecodeQueue;
use livecraft_engine::driver::{AudioDriver, VirtualDriver};
use livecraft_engine::executor::{Engine, EngineCommand};
use livecraft_engine::output::Playback;
use livecraft_engine::rt::checked;
use livecraft_model::ids::TrackId;
use livecraft_model::model::{Clip, ClipPayload, Project};
use std::f64::consts::TAU;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

#[cfg(any(debug_assertions, not(feature = "disable-release-checks")))]
#[global_allocator]
static ALLOCATOR: assert_no_alloc::AllocDisabler = assert_no_alloc::AllocDisabler;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("audio")
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "livecraft_m11_stream_{name}_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_sine_wav(path: &Path, sample_rate: u32, frames: usize, freq_hz: f64, amplitude: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).expect("create wav");
    for i in 0..frames {
        let s = (TAU * freq_hz * (i as f64) / f64::from(sample_rate)).sin() as f32 * amplitude;
        writer.write_sample(s).expect("write L");
        writer.write_sample(s).expect("write R");
    }
    writer.finalize().expect("finalize wav");
}

fn flac_crc8(bytes: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in bytes {
        crc ^= b;
        for _ in 0..8 {
            if (crc & 0x80) != 0 {
                crc = (crc << 1) ^ 0x07;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

fn flac_crc16(bytes: &[u8]) -> u16 {
    let mut crc = 0u16;
    for &b in bytes {
        crc ^= u16::from(b) << 8;
        for _ in 0..8 {
            if (crc & 0x8000) != 0 {
                crc = (crc << 1) ^ 0x8005;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

fn push_flac_utf8(v: u32, out: &mut Vec<u8>) {
    if v <= 0x7F {
        out.push(v as u8);
    } else if v <= 0x7FF {
        out.push(0xC0 | ((v >> 6) as u8));
        out.push(0x80 | ((v & 0x3F) as u8));
    } else if v <= 0xFFFF {
        out.push(0xE0 | ((v >> 12) as u8));
        out.push(0x80 | (((v >> 6) & 0x3F) as u8));
        out.push(0x80 | ((v & 0x3F) as u8));
    } else if v <= 0x1F_FFFF {
        out.push(0xF0 | ((v >> 18) as u8));
        out.push(0x80 | (((v >> 12) & 0x3F) as u8));
        out.push(0x80 | (((v >> 6) & 0x3F) as u8));
        out.push(0x80 | ((v & 0x3F) as u8));
    } else {
        out.push(0xF8 | ((v >> 24) as u8));
        out.push(0x80 | (((v >> 18) & 0x3F) as u8));
        out.push(0x80 | (((v >> 12) & 0x3F) as u8));
        out.push(0x80 | (((v >> 6) & 0x3F) as u8));
        out.push(0x80 | ((v & 0x3F) as u8));
    }
}

/// Writes a compact, valid fixed-block-size (1024-frame) 16-bit stereo compressed FLAC file
/// containing alternating `CONSTANT` subframes (`+16384` / `-16384`), taking ~16 bytes per 1024
/// frames (~5.5 MB for 1 hour @ 96 kHz).
fn write_constant_blocks_flac(path: &Path, sample_rate: u32, num_1024_blocks: u32) {
    const BLOCK_SIZE: u16 = 1024;
    let total_frames = u64::from(num_1024_blocks) * u64::from(BLOCK_SIZE);
    let file = File::create(path).expect("create flac");
    let mut writer = BufWriter::with_capacity(256 * 1024, file);

    writer.write_all(b"fLaC").expect("magic");
    // STREAMINFO (last metadata block = 0x80, type = 0, len = 34)
    writer.write_all(&[0x80, 0x00, 0x00, 0x22]).expect("hdr");
    writer
        .write_all(&BLOCK_SIZE.to_be_bytes())
        .expect("min blk");
    writer
        .write_all(&BLOCK_SIZE.to_be_bytes())
        .expect("max blk");
    writer.write_all(&[0, 0, 0]).expect("min frm");
    writer.write_all(&[0, 0, 0]).expect("max frm");
    let packed: u64 = (u64::from(sample_rate) << 44)
        | (1u64 << 41) // 2 channels -> 2 - 1 = 1
        | (15u64 << 36) // 16 bps -> 16 - 1 = 15
        | (total_frames & 0xF_FFFF_FFFF);
    writer.write_all(&packed.to_be_bytes()).expect("packed");
    writer.write_all(&[0u8; 16]).expect("md5");

    let mut frame_buf = Vec::with_capacity(32);
    for idx in 0..num_1024_blocks {
        frame_buf.clear();
        // Fixed blocking (0xFF, 0xF8), block_size 1024 (0xA) + rate from STREAMINFO (0x0) = 0xA0,
        // 2 channels independent (0x1) + 16-bit (0x4 << 1 = 0x08) = 0x18.
        frame_buf.extend_from_slice(&[0xFF, 0xF8, 0xA0, 0x18]);
        push_flac_utf8(idx, &mut frame_buf);
        let crc8 = flac_crc8(&frame_buf);
        frame_buf.push(crc8);

        let val: i16 = if idx % 2 == 0 { 16384 } else { -16384 };
        // Subframe L: CONSTANT (0x00) + i16 BE
        frame_buf.push(0x00);
        frame_buf.extend_from_slice(&val.to_be_bytes());
        // Subframe R: CONSTANT (0x00) + i16 BE
        frame_buf.push(0x00);
        frame_buf.extend_from_slice(&(-val).to_be_bytes());

        let crc16 = flac_crc16(&frame_buf);
        frame_buf.extend_from_slice(&crc16.to_be_bytes());
        writer.write_all(&frame_buf).expect("write flac frame");
    }
    writer.flush().expect("flush flac");
}

fn audio_project(file_path: &Path, length_beats: f64, is_looping: bool, ram_mode: bool) -> Project {
    let mut project = Project::new_default("M11Audio");
    project.tracks[2].device_chain.clear();
    project.main_track.device_chain.clear();
    let mut clip = Clip::new_audio(
        901,
        "StreamClip",
        file_path.to_string_lossy().into_owned(),
        length_beats,
    );
    clip.is_looping = is_looping;
    clip.loop_end_beat = length_beats;
    if let ClipPayload::Audio(ref mut audio) = clip.payload {
        audio.ram_mode = ram_mode;
        audio.is_warped = false;
        audio.original_bpm = 120.0;
    }
    project.tracks[2].clip_slots[0].clip = Some(clip);
    project
}

fn estimate_dominant_freq_hz(samples: &[f32], sample_rate: u32) -> f64 {
    // Measure 440 Hz vs neighboring bins and verify zero-crossing period matches ~440 Hz.
    let start = (sample_rate as usize / 10).min(samples.len() / 4);
    let end = samples.len().saturating_sub(512).max(start + 2);
    let slice = &samples[start..end];
    let mut crossings = 0usize;
    for w in slice.windows(2) {
        if w[0] <= 0.0 && w[1] > 0.0 {
            crossings += 1;
        }
    }
    let duration_secs = slice.len() as f64 / f64::from(sample_rate);
    crossings as f64 / duration_secs
}

#[test]
fn streamed_clip_playback_loop_wrap_seek_and_graph_swap_allocate_nothing() {
    let dir = temp_dir("rt_no_alloc");
    let wav_path = dir.join("loop_sine.wav");
    // 1.5 second audio file, clip length 1.0 second (2 beats at 120 BPM) so it wraps inside a block.
    write_sine_wav(&wav_path, 44100, 44100 + 22050, 440.0, 0.5);

    let project = audio_project(&wav_path, 2.0, true, false);
    let collector = Collector::new();
    let mut compiler = GraphCompiler::new(44100.0, collector.handle());
    let graph = compiler.compile(&project);
    let (mut engine, mut handle) = Engine::start(graph, 44100.0, collector).expect("start");

    handle
        .send(EngineCommand::TriggerClip {
            track: TrackId(3),
            slot: 0,
        })
        .expect("trigger");
    handle.send(EngineCommand::SetPlaying(true)).expect("play");

    let mut out = vec![0.0f32; MAX_BLOCK * 2];
    let mut energy = 0.0f64;

    // Play 2.5 seconds (~216 blocks) covering loop wraps & graph swaps inside assert_no_alloc.
    let total_blocks = (44100 * 5 / 2) / MAX_BLOCK;
    for block_idx in 0..total_blocks {
        if block_idx == 60 {
            // Recompile and swap graph mid-playback while streamed clip is actively playing.
            handle
                .submit_graph(compiler.compile(&project))
                .expect("submit swap");
        }
        if block_idx == 140 {
            // Seek/jump backwards via JumpTo / SetPlayhead.
            handle.send(EngineCommand::SetPlayhead(0.25)).expect("seek");
        }
        checked(|| engine.process(&mut out, 2));
        energy += out.iter().map(|&s| f64::from(s * s)).sum::<f64>();
    }

    assert!(energy > 10.0, "streamed audio must produce energy");
    assert_eq!(handle.snapshot().disk_underruns, 0);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn first_one_second_plays_immediately_from_preload_without_background_refills() {
    let dir = temp_dir("preload");
    let wav_path = dir.join("three_sec.wav");
    write_sine_wav(&wav_path, 44100, 44100 * 3, 440.0, 0.6);

    let project = audio_project(&wav_path, 6.0, false, false);
    let (mut playback, mut engine) =
        Playback::start_render_in_context(&project, 44100.0, Default::default())
            .expect("start manual");

    playback
        .handle
        .send(EngineCommand::TriggerClip {
            track: TrackId(3),
            slot: 0,
        })
        .expect("trigger");
    playback
        .handle
        .send(EngineCommand::SetPlaying(true))
        .expect("play");

    // Do NOT call playback.streamer().pump_sync() at all!
    // Render the entire first 1.0 second (44100 frames) in 512-frame blocks.
    let mut out = vec![0.0f32; MAX_BLOCK * 2];
    let mut left_all = Vec::with_capacity(44100);
    let mut remaining = 44100usize;
    while remaining > 0 {
        let n = remaining.min(MAX_BLOCK);
        checked(|| engine.process_offline(&mut out[..n * 2], 2));
        for &[l, _] in out[..n * 2].as_chunks::<2>().0 {
            left_all.push(l);
        }
        remaining -= n;
    }

    let snap = playback.handle.snapshot();
    assert_eq!(snap.disk_underruns, 0, "first 1 second must not underrun");
    let freq = estimate_dominant_freq_hz(&left_all, 44100);
    assert!(
        (freq - 440.0).abs() < 3.0,
        "expected ~440 Hz from preloaded first second, got {freq} Hz"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn ram_mode_enforces_max_samples_limit_while_stream_mode_streams_oversized_file() {
    let dir = temp_dir("oversized_limit");
    let flac_path = dir.join("oversized.flac");
    // 135,000 * 1024 = 138,240,000 stereo frames => 276,480,000 samples (> MAX_WAV_SAMPLES = 268,435,456).
    write_constant_blocks_flac(&flac_path, 44100, 135_000);

    // 1. Direct load_file rejects it via MAX_WAV_SAMPLES.
    let probed = livecraft_formats::audio_decode::probe_audio_info(&flac_path).expect("probe");
    assert!(
        probed.total_frames.expect("total frames") * u64::from(probed.channels) > MAX_WAV_SAMPLES
    );
    let err = load_file(&flac_path).expect_err("oversized file must fail RAM decode");
    assert!(
        err.to_string().contains("over the limit"),
        "expected limit error, got: {err}"
    );

    // 2. GraphCompiler with ram_mode == true rejects it cleanly (silent clip, no panic).
    let ram_project = audio_project(&flac_path, 4.0, false, true);
    let collector_ram = Collector::new();
    let mut compiler_ram = GraphCompiler::new(44100.0, collector_ram.handle());
    let mut graph_ram = compiler_ram.compile(&ram_project);
    graph_ram.node_mut(TrackId(3)).expect("t3").clips.launch(0);
    let (mut engine_ram, _h_ram) =
        Engine::start(graph_ram, 44100.0, collector_ram).expect("start ram");
    let mut out = vec![0.0f32; MAX_BLOCK * 2];
    engine_ram.process(&mut out, 2);
    assert!(
        out.iter().all(|&s| s == 0.0),
        "oversized RAM-mode clip must remain silent"
    );

    // 3. GraphCompiler with ram_mode == false streams it cleanly with audible output and 0 underruns.
    let stream_project = audio_project(&flac_path, 16.0, false, false);
    let (mut playback, engine) =
        Playback::start_render_in_context(&stream_project, 44100.0, Default::default())
            .expect("start streamed");
    playback
        .handle
        .send(EngineCommand::TriggerClip {
            track: TrackId(3),
            slot: 0,
        })
        .expect("trigger");
    playback
        .handle
        .send(EngineCommand::SetPlaying(true))
        .expect("play");

    let mut driver = VirtualDriver::new(44100, MAX_BLOCK as u32, 2);
    driver.set_streamer(Arc::clone(playback.streamer()));
    driver.start(engine).expect("start virtual driver");

    // Render 2.0 seconds (well past the 1s preload) to verify active streaming on the oversized file.
    let mut rendered = vec![0.0f32; 44100 * 2 * 2];
    driver.render(&mut rendered);
    assert!(
        rendered.iter().any(|&s| s.abs() > 0.1),
        "streamed oversized file must produce audio"
    );
    assert_eq!(playback.handle.snapshot().disk_underruns, 0);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn rubato_sample_rate_conversion_preserves_440hz_pitch_in_ram_and_streamed_modes() {
    let dir = temp_dir("rubato_pitch");
    let wav_48 = dir.join("sine_440_48k.wav");
    let wav_96 = dir.join("sine_440_96k.wav");
    write_sine_wav(&wav_48, 48000, 48000 * 2, 440.0, 0.5);
    write_sine_wav(&wav_96, 96000, 96000 * 2, 440.0, 0.5);

    for (label, path) in [("48k", &wav_48), ("96k", &wav_96)] {
        for ram_mode in [true, false] {
            let project = audio_project(path, 4.0, false, ram_mode);
            let (mut playback, engine) =
                Playback::start_render_in_context(&project, 44100.0, Default::default())
                    .expect("start");
            playback
                .handle
                .send(EngineCommand::TriggerClip {
                    track: TrackId(3),
                    slot: 0,
                })
                .expect("trigger");
            playback
                .handle
                .send(EngineCommand::SetPlaying(true))
                .expect("play");

            let mut driver = VirtualDriver::new(44100, MAX_BLOCK as u32, 2);
            driver.set_streamer(Arc::clone(playback.streamer()));
            driver.start(engine).expect("start driver");

            let mut out = vec![0.0f32; 44100 * 2];
            driver.render(&mut out);
            let left: Vec<f32> = out.as_chunks::<2>().0.iter().map(|f| f[0]).collect();
            let freq = estimate_dominant_freq_hz(&left, 44100);
            assert!(
                (freq - 440.0).abs() < 3.0,
                "{label} (ram_mode={ram_mode}) expected ~440 Hz at 44.1 kHz engine, got {freq} Hz"
            );
            assert_eq!(playback.handle.snapshot().disk_underruns, 0);
        }
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn decode_queue_and_compiler_prepare_shared_audio_off_thread() {
    let dir = temp_dir("decode_queue");
    let wav_48 = dir.join("tone_48k.wav");
    write_sine_wav(&wav_48, 48000, 48000, 440.0, 0.4);

    let mut collector = Collector::new();
    {
        let queue = DecodeQueue::new(collector.handle());
        queue.submit(&wav_48, 44100);
        let ready = queue
            .recv_ready_timeout(Duration::from_secs(5))
            .expect("ready");
        let shared: Shared<DecodedAudio> = ready.result.expect("decoded");
        assert_eq!(shared.sample_rate, 44100);
        assert_eq!(shared.samples_l.len(), 44100);

        let mut compiler = GraphCompiler::new(44100.0, collector.handle());
        compiler.enqueue_ram_decode_path(&wav_48);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while compiler.poll_decoded() == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "timed out waiting for compiler DecodeQueue"
            );
            std::thread::sleep(Duration::from_millis(5));
        }

        let project = audio_project(&wav_48, 2.0, false, true);
        let mut graph = compiler.compile(&project);
        graph.node_mut(TrackId(3)).expect("t3").clips.launch(0);
        let ctx = livecraft_devices::ProcessContext {
            sample_rate: 44100.0,
            frames: MAX_BLOCK,
            bpm: 120.0,
            playhead_beats: 0.0,
            beats_per_frame: 120.0 / 60.0 / 44100.0,
        };
        graph.render(&ctx);
        assert!(graph.output(MAX_BLOCK).0.iter().any(|&s| s.abs() > 0.05));
    }
    collector.collect();
    assert!(collector.try_cleanup().is_ok());

    // Verify Playback::enqueue_ram_decode & poll_decoded public methods.
    let empty_proj = Project::new_default("EmptyPlayback");
    let (mut playback, _engine) = Playback::start(&empty_proj, 44100.0).expect("start playback");
    let queued = playback.enqueue_ram_decode(&wav_48.to_string_lossy(), None);
    assert!(queued.is_some());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while playback.poll_decoded() == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for Playback::poll_decoded"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn starved_stream_increments_disk_underruns_and_raises_disk_load() {
    let dir = temp_dir("starved");
    let wav_path = dir.join("long_3s.wav");
    write_sine_wav(&wav_path, 44100, 44100 * 3, 440.0, 0.5);

    let project = audio_project(&wav_path, 6.0, false, false);
    let (mut playback, mut engine) =
        Playback::start_render_in_context(&project, 44100.0, Default::default()).expect("start");
    playback
        .handle
        .send(EngineCommand::TriggerClip {
            track: TrackId(3),
            slot: 0,
        })
        .expect("trigger");
    playback
        .handle
        .send(EngineCommand::SetPlaying(true))
        .expect("play");

    let mut out = vec![0.0f32; MAX_BLOCK * 2];

    // Render 1.5 seconds WITHOUT ever calling pump_sync(): first 1.0s plays from preload,
    // remaining 0.5s starves!
    let blocks = (44100 * 3 / 2) / MAX_BLOCK;
    for _ in 0..blocks {
        engine.process_offline(&mut out, 2);
    }

    let starved_snap = playback.handle.snapshot();
    assert!(
        starved_snap.disk_underruns > 0,
        "starved stream must increment disk_underruns"
    );
    assert!(
        starved_snap.disk_load > 0.0,
        "starved stream must raise disk_load > 0.0 (got {})",
        starved_snap.disk_load
    );

    // Pumping refills the ring buffer and stops further underruns.
    let count_before_pump = starved_snap.disk_underruns;
    playback.streamer().pump_sync();
    engine.process_offline(&mut out, 2);
    let recovered_snap = playback.handle.snapshot();
    assert_eq!(recovered_snap.disk_underruns, count_before_pump);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn offline_bouncer_renders_streamed_fixtures_deterministically_across_all_formats() {
    let dir = temp_dir("bouncer_streamed");
    let fixtures = fixtures_dir();

    // 1. Render a > 1.0s looping streamed WAV twice and assert byte-identical output.
    let loop_wav = dir.join("loop_stream.wav");
    write_sine_wav(&loop_wav, 44100, 44100 + 11025, 440.0, 0.5);
    let loop_project = audio_project(&loop_wav, 2.5, true, false);
    let opts = RenderOptions {
        sample_rate: 44100,
        duration_bars: 2, // 4 seconds -> wraps across 1.25s loop boundary multiple times!
        bits_per_sample: 32,
        ..RenderOptions::default()
    };
    let bounce_a = dir.join("loop_a.wav");
    let bounce_b = dir.join("loop_b.wav");
    OfflineBouncer::render_project(&loop_project, &bounce_a, opts.clone()).expect("bounce a");
    OfflineBouncer::render_project(&loop_project, &bounce_b, opts).expect("bounce b");
    assert_eq!(
        fs::read(&bounce_a).expect("read a"),
        fs::read(&bounce_b).expect("read b"),
        "streamed looping bounce must be bit-identical across runs"
    );

    // 2. Render compressed fixtures (FLAC, OGG, MP3, AAC M4A, ALAC M4A) with ram_mode == false.
    for name in [
        "sine_440.flac",
        "sine_440.ogg",
        "sine_440.mp3",
        "sine_440_aac.m4a",
        "sine_440_alac.m4a",
    ] {
        let src = fixtures.join(name);
        let proj = audio_project(&src, 4.0, false, false);
        let out_a = dir.join(format!("{name}_a.wav"));
        let out_b = dir.join(format!("{name}_b.wav"));
        let report_opts = RenderOptions {
            sample_rate: 44100,
            duration_bars: 1,
            bits_per_sample: 32,
            ..RenderOptions::default()
        };
        let rep = OfflineBouncer::render_project_with_report(&proj, &out_a, report_opts.clone())
            .expect("render fixture");
        OfflineBouncer::render_project(&proj, &out_b, report_opts).expect("render fixture b");
        assert_eq!(
            fs::read(&out_a).expect("read a"),
            fs::read(&out_b).expect("read b"),
            "{name} streamed render must be byte-identical"
        );
        assert!(
            rep.outputs[0].metrics.peak_dbfs > -30.0,
            "{name} output should be audible"
        );
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
#[ignore = "1-hour 96 kHz stereo streaming verification; run explicitly with --ignored"]
fn one_hour_96khz_stereo_plays_with_zero_underruns() {
    let dir = temp_dir("one_hour_96k");
    let flac_path = dir.join("one_hour_96khz_stereo.flac");

    // 1 hour at 96 kHz = 345,600,000 frames = 337,500 * 1024-frame FLAC blocks (~5.5 MB).
    const RATE: u32 = 96_000;
    const BLOCKS_1024: u32 = 337_500;
    const TOTAL_FRAMES: u64 = 345_600_000;
    write_constant_blocks_flac(&flac_path, RATE, BLOCKS_1024);

    // 1 hour = 3600s = 7200 beats at 120 BPM.
    let project = audio_project(&flac_path, 7200.0, false, false);
    let (mut playback, mut engine) =
        Playback::start_render_in_context(&project, RATE as f32, Default::default())
            .expect("start 96 kHz engine");

    playback
        .handle
        .send(EngineCommand::TriggerClip {
            track: TrackId(3),
            slot: 0,
        })
        .expect("trigger");
    playback
        .handle
        .send(EngineCommand::SetPlaying(true))
        .expect("play");

    let streamer = Arc::clone(playback.streamer());
    let mut out = vec![0.0f32; MAX_BLOCK * 2];
    let mut rendered: u64 = 0;
    let mut energy = 0.0f64;

    while rendered < TOTAL_FRAMES {
        // Pump synchronously every 64 blocks (~340 ms of audio, well within the 4-second ring).
        if (rendered / MAX_BLOCK as u64).is_multiple_of(64) {
            streamer.pump_sync();
        }
        let block = (TOTAL_FRAMES - rendered).min(MAX_BLOCK as u64) as usize;
        let slice = &mut out[..block * 2];
        checked(|| engine.process_offline(slice, 2));
        for &s in slice.iter() {
            energy += f64::from(s * s);
        }
        rendered += block as u64;
    }

    assert!(energy > 1000.0, "1-hour stream must produce audio energy");
    assert_eq!(playback.handle.snapshot().disk_underruns, 0);

    let _ = fs::remove_dir_all(&dir);
}
