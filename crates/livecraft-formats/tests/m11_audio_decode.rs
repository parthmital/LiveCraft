use std::f64::consts::PI;
use std::fs;
use std::path::{Path, PathBuf};

use livecraft_formats::analysis_cache::{
    ANALYSIS_VERSION, AnalysisCacheLocation, CachedSampleAnalysis, PeakFrame, analysis_cache_path,
    build_waveform_pyramid_streaming, load_or_build_analysis,
};
use livecraft_formats::audio_decode::{
    AudioDecodeError, AudioStreamReader, MAX_RAM_AUDIO_SAMPLES, decode_audio_file,
    decode_audio_file_with_limit, probe_audio_info,
};
use livecraft_formats::audio_encode::{AudioFileFormat, BitDepth, write_mp3_320, write_pcm_file};
use livecraft_formats::health::probe_audio_sample_rate;
use proptest::prelude::*;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("audio")
}

fn temp_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("livecraft_m11_{name}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn fitted_440hz_snr_db(samples: &[f32], sample_rate: u32) -> (f64, f64, f64, f64) {
    // Fit s[n] = a * sin(w*n) + b * cos(w*n) + c over interior frames to measure
    // amplitude, phase offset, and non-440Hz distortion/noise floor (THD+N).
    let start = 2048usize.min(samples.len() / 4);
    let end = samples.len().saturating_sub(2048).max(start + 1);
    let omega = 2.0 * PI * 440.0 / f64::from(sample_rate);
    let mut s_sin = 0.0f64;
    let mut s_cos = 0.0f64;
    let mut s_dc = 0.0f64;
    let mut n = 0usize;
    for (i, &x) in samples[start..end].iter().enumerate() {
        let idx = (start + i) as f64;
        let xf = f64::from(x);
        s_sin += xf * (omega * idx).sin();
        s_cos += xf * (omega * idx).cos();
        s_dc += xf;
        n += 1;
    }
    let nf = n.max(1) as f64;
    let a = 2.0 * s_sin / nf;
    let b = 2.0 * s_cos / nf;
    let c = s_dc / nf;
    let amp = (a * a + b * b).sqrt();
    let phase = b.atan2(a);

    let mut sig_sq = 0.0f64;
    let mut err_sq = 0.0f64;
    for (i, &x) in samples[start..end].iter().enumerate() {
        let idx = (start + i) as f64;
        let fit = a * (omega * idx).sin() + b * (omega * idx).cos() + c;
        let diff = f64::from(x) - fit;
        sig_sq += fit * fit;
        err_sq += diff * diff;
    }
    let thdn_db = 10.0 * (err_sq / sig_sq.max(1e-24)).log10();
    (amp, phase, c, thdn_db)
}

fn best_aligned_residual_db(
    decoded: &[f32],
    reference: &[f32],
    max_lag: usize,
) -> (usize, f64, f64, f64) {
    let mut best_lag = 0usize;
    let mut best_coarse_mse = f64::INFINITY;
    let margin = 1024usize;
    let end = reference.len().saturating_sub(margin);

    for lag in 0..=max_lag {
        let mut sum_sq = 0.0f64;
        let mut count = 0usize;
        let mut i = margin;
        while i < end {
            let dec_idx = i + lag;
            if dec_idx >= decoded.len() {
                break;
            }
            let diff = f64::from(decoded[dec_idx]) - f64::from(reference[i]);
            sum_sq += diff * diff;
            count += 1;
            i += 8;
        }
        if count >= 512 {
            let mse = sum_sq / count as f64;
            if mse < best_coarse_mse {
                best_coarse_mse = mse;
                best_lag = lag;
            }
        }
    }

    let mut sum_sq = 0.0f64;
    let mut ref_sq = 0.0f64;
    let mut count = 0usize;
    for (i, &r) in reference.iter().enumerate().take(end).skip(margin) {
        let dec_idx = i + best_lag;
        if dec_idx < decoded.len() {
            let d = f64::from(decoded[dec_idx]);
            let rf = f64::from(r);
            let diff = d - rf;
            sum_sq += diff * diff;
            ref_sq += rf * rf;
            count += 1;
        }
    }
    let rms_err = (sum_sq / count.max(1) as f64).sqrt();
    let rms_ref = (ref_sq / count.max(1) as f64).sqrt();
    let rel_db = 20.0 * (rms_err / rms_ref.max(1e-12)).log10();
    let dbfs_err = 20.0 * rms_err.max(1e-12).log10();
    (best_lag, rms_err, rel_db, dbfs_err)
}

#[test]
fn all_seven_fixtures_probe_and_decode_within_codec_tolerances() {
    let dir = fixtures_dir();

    let wav_path = dir.join("sine_440.wav");
    let wav = decode_audio_file(&wav_path).expect("decode wav");
    assert_eq!(wav.sample_rate, 44100);
    assert_eq!(wav.channels, 2);
    assert!(!wav.samples_l.is_empty());
    assert_eq!(wav.samples_l.len(), wav.samples_r.len());

    // Check WAV against ideal 440 Hz sine wave (determine amplitude from RMS).
    let rms_l = (wav
        .samples_l
        .iter()
        .map(|&s| f64::from(s) * f64::from(s))
        .sum::<f64>()
        / wav.samples_l.len() as f64)
        .sqrt();
    let amp = rms_l * std::f64::consts::SQRT_2;
    let max_sine_err = wav
        .samples_l
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let expected = amp * (2.0 * PI * 440.0 * (i as f64 / 44100.0)).sin();
            (f64::from(s) - expected).abs()
        })
        .fold(0.0f64, f64::max);
    eprintln!(
        "sine_440.wav: frames={}, amp={amp:.6}, max_sine_err={max_sine_err:.9}",
        wav.samples_l.len()
    );
    assert!(
        max_sine_err <= 1e-3,
        "WAV 440 Hz sine max error {max_sine_err} exceeded 1e-3"
    );

    // Lossless fixtures: AIFF, FLAC, ALAC/M4A must match WAV PCM exactly.
    for name in ["sine_440.aif", "sine_440.flac", "sine_440_alac.m4a"] {
        let path = dir.join(name);
        let info = probe_audio_info(&path).expect("probe lossless info");
        assert_eq!(info.sample_rate, 44100, "{name} sample_rate");
        assert_eq!(info.channels, 2, "{name} channels");
        assert_eq!(
            probe_audio_sample_rate(&path).expect("health probe"),
            44100,
            "{name} health probe"
        );

        let dec = decode_audio_file(&path).expect("decode lossless");
        assert_eq!(dec.sample_rate, wav.sample_rate, "{name} rate");
        assert_eq!(dec.channels, wav.channels, "{name} channels");
        assert_eq!(dec.samples_l.len(), wav.samples_l.len(), "{name} len");
        let max_diff_l = dec
            .samples_l
            .iter()
            .zip(&wav.samples_l)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        let max_diff_r = dec
            .samples_r
            .iter()
            .zip(&wav.samples_r)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        eprintln!("{name}: max_diff_vs_wav L={max_diff_l:.9}, R={max_diff_r:.9}");
        assert!(max_diff_l <= 1e-5, "{name} L diff {max_diff_l}");
        assert!(max_diff_r <= 1e-5, "{name} R diff {max_diff_r}");
    }

    // Lossy fixtures: MP3, OGG Vorbis, AAC/M4A.
    let lossy_specs = [
        ("sine_440.mp3", -75.0_f64, -62.0_f64),
        ("sine_440.ogg", -41.0_f64, -47.5_f64),
        ("sine_440_aac.m4a", -37.5_f64, -37.5_f64),
    ];
    for (name, max_rel_db, max_thdn_db) in lossy_specs {
        let path = dir.join(name);
        let info = probe_audio_info(&path).expect("probe lossy info");
        assert_eq!(info.sample_rate, 44100, "{name} sample_rate");
        assert_eq!(info.channels, 2, "{name} channels");
        assert_eq!(
            probe_audio_sample_rate(&path).expect("health probe"),
            44100,
            "{name} health probe"
        );

        let dec = decode_audio_file(&path).expect("decode lossy");
        assert_eq!(dec.sample_rate, 44100, "{name} rate");
        assert_eq!(dec.channels, 2, "{name} channels");
        assert!(!dec.samples_l.is_empty(), "{name} empty decode");

        let (lag_l, rms_l_err, rel_db_l, dbfs_l) =
            best_aligned_residual_db(&dec.samples_l, &wav.samples_l, 1536);
        let (lag_r, rms_r_err, rel_db_r, dbfs_r) =
            best_aligned_residual_db(&dec.samples_r, &wav.samples_r, 1536);
        let (amp_l, _phase_l, _dc_l, thdn_l) = fitted_440hz_snr_db(&dec.samples_l, 44100);
        let (amp_r, _phase_r, _dc_r, thdn_r) = fitted_440hz_snr_db(&dec.samples_r, 44100);
        eprintln!(
            "{name}: frames={}, L(lag={lag_l}, rms_err={rms_l_err:.6}, rel_db={rel_db_l:.2} dB, dbfs={dbfs_l:.2} dBFS, thdn={thdn_l:.2} dB, amp={amp_l:.6}), R(lag={lag_r}, rms_err={rms_r_err:.6}, rel_db={rel_db_r:.2} dB, dbfs={dbfs_r:.2} dBFS, thdn={thdn_r:.2} dB, amp={amp_r:.6})",
            dec.samples_l.len()
        );
        assert!(
            (amp_l - amp).abs() <= 0.002,
            "{name} L amp {amp_l} vs {amp}"
        );
        assert!(
            (amp_r - amp).abs() <= 0.002,
            "{name} R amp {amp_r} vs {amp}"
        );
        assert!(
            dbfs_l <= -60.0 && dbfs_r <= -60.0,
            "{name} full-scale RMS residual ({dbfs_l:.2}, {dbfs_r:.2}) exceeded -60 dBFS"
        );
        assert!(
            rel_db_l <= max_rel_db && rel_db_r <= max_rel_db,
            "{name} relative residual ({rel_db_l:.2}, {rel_db_r:.2}) exceeded {max_rel_db} dB"
        );
        assert!(
            thdn_l <= max_thdn_db && thdn_r <= max_thdn_db,
            "{name} THD+N ({thdn_l:.2}, {thdn_r:.2}) exceeded {max_thdn_db} dB"
        );
    }
}

#[test]
fn stream_reader_chunked_read_and_frame_accurate_seek_match_full_decode() {
    let dir = fixtures_dir();
    let files = [
        "sine_440.wav",
        "sine_440.aif",
        "sine_440.flac",
        "sine_440.mp3",
        "sine_440.ogg",
        "sine_440_aac.m4a",
        "sine_440_alac.m4a",
    ];

    for name in files {
        let path = dir.join(name);
        let full = decode_audio_file(&path).expect("full decode");
        let total = full.samples_l.len();

        // 1. Sequential chunked streaming (odd chunk size 517 frames) matches decode_audio_file bit-for-bit.
        let mut reader = AudioStreamReader::open(&path).expect("open stream");
        assert_eq!(reader.sample_rate(), full.sample_rate);
        assert_eq!(reader.channels(), full.channels);
        assert_eq!(reader.position_frames(), 0);

        let mut streamed_l = Vec::new();
        let mut streamed_r = Vec::new();
        let mut chunk = Vec::with_capacity(517);
        loop {
            let n = reader.read_frames(517, &mut chunk).expect("read chunk");
            assert_eq!(chunk.len(), n);
            if n == 0 {
                break;
            }
            for &[l, r] in &chunk {
                streamed_l.push(l);
                streamed_r.push(r);
            }
        }
        assert_eq!(
            reader.position_frames(),
            total as u64,
            "{name} end position"
        );
        assert_eq!(streamed_l, full.samples_l, "{name} streamed L");
        assert_eq!(streamed_r, full.samples_r, "{name} streamed R");

        // 2. Seek forward, backward, to 0 after EOF, and compare against full decode slice.
        let end_seek_anchor = reader
            .total_frames()
            .map(|t| t as usize)
            .unwrap_or(total)
            .min(total);
        let targets = [
            1234usize,
            22050,
            5000,
            0,
            end_seek_anchor.saturating_sub(250),
        ];
        for &target in &targets {
            reader.seek(target as u64).expect("seek");
            assert_eq!(
                reader.position_frames(),
                target as u64,
                "{name} pos after seek({target})"
            );
            let mut buf = Vec::new();
            let want = 1500.min(total - target);
            let n = reader.read_frames(want, &mut buf).expect("read after seek");
            assert_eq!(n, want, "{name} read count after seek({target})");
            assert_eq!(
                reader.position_frames(),
                (target + n) as u64,
                "{name} pos after read"
            );

            let mut max_diff = 0.0f32;
            for (i, &[l, r]) in buf.iter().enumerate() {
                max_diff = max_diff
                    .max((l - full.samples_l[target + i]).abs())
                    .max((r - full.samples_r[target + i]).abs());
            }
            eprintln!("{name} seek({target}): max_diff={max_diff:.8}");
            assert!(
                max_diff <= 1e-4,
                "{name} seek({target}) max_diff {max_diff} exceeded 1e-4"
            );
        }

        // Seeking to exact total_frames leaves 0 frames remaining; seeking past total errors cleanly.
        if let Some(known_total) = reader.total_frames() {
            reader.seek(known_total).expect("seek to exact EOF");
            let mut eof_buf = Vec::new();
            assert_eq!(reader.read_frames(64, &mut eof_buf).expect("eof read"), 0);
            assert!(reader.seek(known_total + 100).is_err());
        }
    }
}

#[test]
fn mono_duplication_and_sample_limit_and_invalid_inputs() {
    assert_eq!(MAX_RAM_AUDIO_SAMPLES, 1 << 28);
    let dir = temp_test_dir("limits_mono");

    // Mono WAV duplicates channel to both samples_l and samples_r.
    let mono_wav = dir.join("mono.wav");
    let mono_samples: Vec<f32> = (0..1024)
        .map(|i| 0.5 * (2.0 * PI * 220.0 * (i as f64 / 44100.0)).sin() as f32)
        .collect();
    write_pcm_file(
        &mono_wav,
        AudioFileFormat::Wav,
        BitDepth::Bits16,
        44100,
        1,
        &mono_samples,
    )
    .expect("write mono wav");
    let dec = decode_audio_file(&mono_wav).expect("decode mono wav");
    assert_eq!(dec.channels, 1);
    assert_eq!(dec.samples_l.len(), 1024);
    assert_eq!(dec.samples_l, dec.samples_r);

    // Pre-decode limit rejection (WAV knows n_frames in header).
    let wav_err = decode_audio_file_with_limit(&mono_wav, 500).unwrap_err();
    assert!(
        matches!(wav_err, AudioDecodeError::ExceedsLimit { limit: 500, .. }),
        "expected ExceedsLimit, got {wav_err:?}"
    );

    // During-decode limit rejection (strip the Xing/Info frame so total_frames is unknown).
    let mp3_with_xing = dir.join("with_xing.mp3");
    let pcm = vec![0.25f32; 8192];
    write_mp3_320(&mp3_with_xing, 44100, 1, &pcm).expect("write mp3");
    let raw_mp3 = fs::read(&mp3_with_xing).expect("read mp3");
    let second_frame_offset = raw_mp3
        .windows(3)
        .enumerate()
        .skip(512)
        .find(|(_, w)| w[0] == 0xFF && (w[1] & 0xFE) == 0xFA && (w[2] >> 4) == 0x0E)
        .map(|(idx, _)| idx)
        .expect("find second MPEG frame after Xing tag");
    let mp3_path = dir.join("no_xing.mp3");
    fs::write(&mp3_path, &raw_mp3[second_frame_offset..]).expect("write raw mp3 without Xing");
    assert_eq!(
        probe_audio_info(&mp3_path).expect("probe mp3").total_frames,
        None
    );
    let mp3_err = decode_audio_file_with_limit(&mp3_path, 2000).unwrap_err();
    assert!(
        matches!(mp3_err, AudioDecodeError::ExceedsLimit { limit: 2000, .. }),
        "expected ExceedsLimit for unknown-length MP3, got {mp3_err:?}"
    );

    // Unknown-length MP3 streaming + seeking + out-of-range error.
    let full_no_xing = decode_audio_file(&mp3_path).expect("full decode no-xing mp3");
    let mut mp3_stream = AudioStreamReader::open(&mp3_path).expect("open no-xing mp3");
    assert_eq!(mp3_stream.total_frames(), None);
    mp3_stream.seek(2304).expect("seek no-xing mp3");
    let mut tail_buf = Vec::new();
    let n_read = mp3_stream
        .read_frames(1024, &mut tail_buf)
        .expect("read no-xing mp3");
    assert_eq!(n_read, 1024);
    for (i, &[l, r]) in tail_buf.iter().enumerate() {
        assert!((l - full_no_xing.samples_l[2304 + i]).abs() <= 1e-6);
        assert!((r - full_no_xing.samples_r[2304 + i]).abs() <= 1e-6);
    }
    assert!(
        mp3_stream
            .seek((full_no_xing.samples_l.len() as u64) + 5000)
            .is_err()
    );

    // Zero-frame WAV decodes cleanly without panic.
    let zero_frame_wav = dir.join("empty.wav");
    write_pcm_file(
        &zero_frame_wav,
        AudioFileFormat::Wav,
        BitDepth::Bits16,
        44100,
        2,
        &[],
    )
    .expect("write empty wav");
    let empty_dec = decode_audio_file(&zero_frame_wav).expect("decode empty wav");
    assert!(empty_dec.samples_l.is_empty());
    let empty_analysis =
        load_or_build_analysis(&zero_frame_wav, &AnalysisCacheLocation::BesideSample, true)
            .expect("analyze empty wav");
    assert_eq!(empty_analysis.frames, 0);
    assert!(empty_analysis.peak_overview.is_empty());
    assert!(empty_analysis.overview_4096.is_empty());
    assert!(empty_analysis.overview_65536.is_empty());

    // 0-channel WAV rejected cleanly.
    let zero_ch_wav = dir.join("zero_ch.wav");
    let mut bad_wav = vec![0u8; 44];
    bad_wav[0..4].copy_from_slice(b"RIFF");
    bad_wav[4..8].copy_from_slice(&36u32.to_le_bytes());
    bad_wav[8..16].copy_from_slice(b"WAVEfmt ");
    bad_wav[16..20].copy_from_slice(&16u32.to_le_bytes());
    bad_wav[20..22].copy_from_slice(&1u16.to_le_bytes());
    bad_wav[22..24].copy_from_slice(&0u16.to_le_bytes()); // 0 channels
    bad_wav[24..28].copy_from_slice(&44100u32.to_le_bytes());
    bad_wav[34..36].copy_from_slice(&16u16.to_le_bytes());
    bad_wav[36..40].copy_from_slice(b"data");
    fs::write(&zero_ch_wav, &bad_wav).expect("write zero_ch wav");
    assert!(probe_audio_info(&zero_ch_wav).is_err());
    assert!(decode_audio_file(&zero_ch_wav).is_err());

    // Non-audio file rejected cleanly.
    let text_file = dir.join("not_audio.txt");
    fs::write(&text_file, b"plain text not audio").expect("write text");
    assert!(decode_audio_file(&text_file).is_err());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn waveform_pyramid_and_analysis_cache_roundtrip_invalidation_and_asd_preservation() {
    let dir = temp_test_dir("analysis_cache");

    // Generate > 65536 frames (70,000 stereo frames) so all 3 pyramid levels have multiple blocks.
    let wav_path = dir.join("long_tone.wav");
    let frames = 70_000usize;
    let mut interleaved = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let env = (i as f32 / frames as f32) * 0.9;
        let s = env * (2.0 * std::f32::consts::PI * 100.0 * (i as f32 / 44100.0)).sin();
        interleaved.push(s);
        interleaved.push(-s * 0.5);
    }
    write_pcm_file(
        &wav_path,
        AudioFileFormat::Wav,
        BitDepth::Bits16,
        44100,
        2,
        &interleaved,
    )
    .expect("write long wav");

    // Create dummy .asd files beside the sample; they must never be modified or deleted.
    let asd_a = dir.join("long_tone.wav.asd");
    let asd_b = dir.join("long_tone.asd");
    fs::write(&asd_a, b"ASD-BINARY-A").expect("write asd a");
    fs::write(&asd_b, b"ASD-BINARY-B").expect("write asd b");

    let (pyramid, info, actual_frames) =
        build_waveform_pyramid_streaming(&wav_path).expect("build pyramid");
    assert_eq!(info.sample_rate, 44100);
    assert_eq!(info.channels, 2);
    assert_eq!(actual_frames, frames as u64);
    assert_eq!(pyramid.block_256.len(), frames.div_ceil(256));
    assert_eq!(pyramid.block_4096.len(), frames.div_ceil(4096));
    assert_eq!(pyramid.block_65536.len(), frames.div_ceil(65536));
    assert_eq!(pyramid.block_65536.len(), 2);

    for (idx, p) in pyramid.block_256.iter().enumerate() {
        assert!(p.min <= p.max, "block_256[{idx}] min > max");
    }
    // Check pyramid aggregation invariant: each 4096 block covers 16 block_256 blocks.
    for (idx, p4k) in pyramid.block_4096.iter().enumerate() {
        let slice = &pyramid.block_256[idx * 16..((idx + 1) * 16).min(pyramid.block_256.len())];
        let min = slice.iter().map(|p| p.min).fold(f32::INFINITY, f32::min);
        let max = slice
            .iter()
            .map(|p| p.max)
            .fold(f32::NEG_INFINITY, f32::max);
        assert_eq!(p4k.min, min);
        assert_eq!(p4k.max, max);
    }

    // Build and write cache BesideSample.
    let built = load_or_build_analysis(&wav_path, &AnalysisCacheLocation::BesideSample, true)
        .expect("build and save cache");
    assert_eq!(built.version, ANALYSIS_VERSION);
    assert_eq!(built.overview_block_frames, 256);
    assert_eq!(built.peak_overview, pyramid.block_256);
    assert_eq!(built.overview_4096, pyramid.block_4096);
    assert_eq!(built.overview_65536, pyramid.block_65536);

    let cache_file = analysis_cache_path(&wav_path, &AnalysisCacheLocation::BesideSample);
    assert!(cache_file.is_file());
    assert_eq!(fs::read(&asd_a).expect("read asd a"), b"ASD-BINARY-A");
    assert_eq!(fs::read(&asd_b).expect("read asd b"), b"ASD-BINARY-B");

    // write_cache = false builds without creating a new sidecar file.
    let nocache_dir = dir.join("nocache");
    let nocache_loc = AnalysisCacheLocation::Folder(nocache_dir.clone());
    let nocache_path = analysis_cache_path(&wav_path, &nocache_loc);
    let built_nocache =
        load_or_build_analysis(&wav_path, &nocache_loc, false).expect("build without writing");
    assert_eq!(built_nocache.frames, frames as u64);
    assert!(!nocache_path.exists());

    // Corrupt cache file recovers cleanly by rebuilding.
    fs::write(&cache_file, b"{corrupt json").expect("corrupt cache");
    let recovered = load_or_build_analysis(&wav_path, &AnalysisCacheLocation::BesideSample, true)
        .expect("recover from corrupt cache");
    assert_eq!(recovered.peak_overview, pyramid.block_256);

    // Cache hit verification: inject a sentinel into peak_overview while keeping source metadata matching.
    let mut sentinel_cache = built.clone();
    sentinel_cache.peak_overview[0] = PeakFrame {
        min: -0.1234,
        max: 0.4321,
    };
    fs::write(
        &cache_file,
        serde_json::to_vec_pretty(&sentinel_cache).expect("serialize sentinel"),
    )
    .expect("overwrite sentinel");
    let hit = load_or_build_analysis(&wav_path, &AnalysisCacheLocation::BesideSample, true)
        .expect("cache hit");
    assert_eq!(hit.peak_overview[0], sentinel_cache.peak_overview[0]);

    // Stale invalidation when source file length changes.
    let shorter = vec![0.1f32; 1024];
    write_pcm_file(
        &wav_path,
        AudioFileFormat::Wav,
        BitDepth::Bits16,
        44100,
        2,
        &shorter,
    )
    .expect("rewrite shorter wav");
    let rebuilt = load_or_build_analysis(&wav_path, &AnalysisCacheLocation::BesideSample, true)
        .expect("rebuild stale cache");
    assert_eq!(rebuilt.frames, 512);
    assert_eq!(rebuilt.peak_overview.len(), 2);
    assert_eq!(rebuilt.overview_4096.len(), 1);
    assert_eq!(rebuilt.overview_65536.len(), 1);

    // Folder cache location avoids filename collisions across directories.
    let sub_a = dir.join("dir_a");
    let sub_b = dir.join("dir_b");
    let cache_dir = dir.join("cache_folder");
    fs::create_dir_all(&sub_a).expect("sub_a");
    fs::create_dir_all(&sub_b).expect("sub_b");
    let path_a = sub_a.join("kick.wav");
    let path_b = sub_b.join("kick.wav");
    fs::copy(&wav_path, &path_a).expect("copy a");
    fs::copy(&wav_path, &path_b).expect("copy b");

    let folder_loc = AnalysisCacheLocation::Folder(cache_dir);
    let cache_a = analysis_cache_path(&path_a, &folder_loc);
    let cache_b = analysis_cache_path(&path_b, &folder_loc);
    assert_ne!(cache_a, cache_b);
    assert_eq!(
        analysis_cache_path(&path_a, &folder_loc),
        cache_a,
        "folder cache path must be deterministic"
    );
    load_or_build_analysis(&path_a, &folder_loc, true).expect("cache a");
    load_or_build_analysis(&path_b, &folder_loc, true).expect("cache b");
    assert!(cache_a.is_file() && cache_b.is_file());

    // Compatibility with M08 AnalysisFile JSON format (which includes `metrics` and omits M11 pyramid fields).
    let m08_json = r#"{
        "version": 1,
        "sample_rate": 44100,
        "channels": 2,
        "frames": 256,
        "metrics": {
            "peak_dbfs": -3.0,
            "true_peak_dbtp": -2.9,
            "rms_dbfs": -6.0,
            "integrated_lufs": -14.0,
            "short_term_max_lufs": -13.0,
            "momentary_max_lufs": -12.0
        },
        "overview_block_frames": 256,
        "peak_overview": [{ "min": -0.5, "max": 0.5 }]
    }"#;
    let parsed_m08: CachedSampleAnalysis =
        serde_json::from_str(m08_json).expect("parse M08 AnalysisFile JSON");
    assert_eq!(parsed_m08.version, 1);
    assert_eq!(parsed_m08.frames, 256);
    assert!(parsed_m08.metrics.is_some());
    assert!(parsed_m08.overview_4096.is_empty());

    let _ = fs::remove_dir_all(&dir);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn arbitrary_bytes_never_panic_in_decode_probe_or_analysis_json(
        ext_idx in 0usize..7,
        bytes in proptest::collection::vec(any::<u8>(), 0..2048)
    ) {
        let ext = ["wav", "aif", "flac", "mp3", "ogg", "m4a", "bin"][ext_idx];
        let path = std::env::temp_dir().join(format!(
            "livecraft_m11_prop_{}.{ext}",
            std::process::id()
        ));
        let _ = fs::write(&path, &bytes);
        let _ = probe_audio_info(&path);
        let _ = decode_audio_file_with_limit(&path, 65_536);
        let _ = fs::remove_file(&path);

        let _ = serde_json::from_slice::<CachedSampleAnalysis>(&bytes);
    }
}
