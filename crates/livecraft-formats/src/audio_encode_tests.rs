use crate::audio_encode::*;
use std::path::{Path, PathBuf};
use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::audio::{AudioCodecId, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, TrackType as SymphoniaTrackType, probe::Hint};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

#[derive(Debug)]
struct SymphoniaDecoded {
    codec: AudioCodecId,
    sample_rate: u32,
    channels: usize,
    bits_per_sample: Option<u32>,
    verify_ok: Option<bool>,
    i16_samples: Vec<i16>,
    i24_samples: Vec<i32>,
    i32_samples: Vec<i32>,
    f32_samples: Vec<f32>,
}

fn temp_test_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("livecraft_formats_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn decode_with_symphonia(path: &Path, ext_hint: &str) -> SymphoniaDecoded {
    use symphonia::core::audio::Audio;

    let file = std::fs::File::open(path).expect("open encoded audio file");
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(ext_hint);

    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .expect("probe encoded audio file");

    let track = format
        .default_track(SymphoniaTrackType::Audio)
        .expect("default audio track");
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|cp| cp.audio())
        .expect("audio codec params")
        .clone();

    let codec = params.codec;
    let sample_rate = params.sample_rate.expect("sample rate");
    let channels = params.channels.as_ref().expect("channels").count();
    let bits_per_sample = params.bits_per_sample;

    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default().verify(true))
        .expect("make audio decoder");

    let mut i16_samples = Vec::new();
    let mut i24_samples = Vec::new();
    let mut i32_samples = Vec::new();
    let mut f32_samples = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(err) => panic!("error reading packet: {err}"),
        };
        if packet.track_id != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(err) => panic!("error decoding packet: {err}"),
        };

        match decoded {
            GenericAudioBufferRef::S16(buf) => {
                i16_samples.extend(buf.iter_interleaved());
            }
            GenericAudioBufferRef::S24(buf) => {
                i24_samples.extend(buf.iter_interleaved().map(|s| s.inner()));
            }
            GenericAudioBufferRef::S32(buf) => {
                i32_samples.extend(buf.iter_interleaved());
            }
            GenericAudioBufferRef::F32(buf) => {
                f32_samples.extend(buf.iter_interleaved());
            }
            other => {
                other.copy_to_vec_interleaved::<f32>(&mut f32_samples);
            }
        }
    }

    let verify_ok = decoder.finalize().verify_ok;

    SymphoniaDecoded {
        codec,
        sample_rate,
        channels,
        bits_per_sample,
        verify_ok,
        i16_samples,
        i24_samples,
        i32_samples,
        f32_samples,
    }
}

#[test]
fn test_wav_roundtrip_16_24_f32_unclamped() {
    let dir = temp_test_dir("wav_roundtrip");

    // 16-bit mono
    let wav16_path = dir.join("test16.wav");
    let pcm16 = vec![0i16, 1234, -5678, i16::MAX, i16::MIN, 42];
    write_pcm_i16(&wav16_path, AudioFileFormat::Wav, 44100, 1, &pcm16).expect("write wav16");
    let dec16 = decode_with_symphonia(&wav16_path, "wav");
    assert_eq!(dec16.sample_rate, 44100);
    assert_eq!(dec16.channels, 1);
    assert_eq!(dec16.bits_per_sample, Some(16));
    assert_eq!(dec16.i16_samples, pcm16);

    // 24-bit stereo
    let wav24_path = dir.join("test24.wav");
    let pcm24 = vec![
        0i32, 1_000_000, -1_000_000, 8_388_607, -8_388_608, 123_456, -654_321, 1,
    ];
    write_pcm_i24(&wav24_path, AudioFileFormat::Wav, 48000, 2, &pcm24).expect("write wav24");
    let dec24 = decode_with_symphonia(&wav24_path, "wav");
    assert_eq!(dec24.sample_rate, 48000);
    assert_eq!(dec24.channels, 2);
    assert_eq!(dec24.bits_per_sample, Some(24));
    assert_eq!(dec24.i24_samples, pcm24);

    // 32-bit float stereo (unclamped > 1.0 and < -1.0 preserved!)
    let wav32_path = dir.join("test32f.wav");
    let pcm32 = vec![1.5f32, -1.75, 0.0, 0.25, -0.5, 2.25];
    write_pcm_f32(&wav32_path, AudioFileFormat::Wav, 96000, 2, &pcm32).expect("write wav32f");
    let dec32 = decode_with_symphonia(&wav32_path, "wav");
    assert_eq!(dec32.sample_rate, 96000);
    assert_eq!(dec32.channels, 2);
    assert_eq!(dec32.bits_per_sample, Some(32));
    assert_eq!(dec32.f32_samples, pcm32);
    assert_eq!(dec32.f32_samples[0], 1.5);

    // Also verify write_pcm_file preserves 1.5 unclamped for Float32 and quantizes Bits16 / Bits24
    let wav32_via_file = dir.join("test32f_generic.wav");
    write_pcm_file(
        &wav32_via_file,
        AudioFileFormat::Wav,
        BitDepth::Float32,
        44100,
        2,
        &pcm32,
    )
    .expect("write_pcm_file wav32f");
    let dec32_via_file = decode_with_symphonia(&wav32_via_file, "wav");
    assert_eq!(dec32_via_file.f32_samples, pcm32);

    let float_unit = vec![0.0f32, 0.5, -0.5, 1.5, -1.5, 0.25];
    let wav16_via_file = dir.join("test16_generic.wav");
    write_pcm_file(
        &wav16_via_file,
        AudioFileFormat::Wav,
        BitDepth::Bits16,
        44100,
        2,
        &float_unit,
    )
    .expect("write_pcm_file wav16");
    let dec16_via_file = decode_with_symphonia(&wav16_via_file, "wav");
    assert_eq!(
        dec16_via_file.i16_samples,
        vec![0, 16384, -16384, 32767, -32767, 8192]
    );

    let aiff24_via_file = dir.join("test24_generic.aiff");
    write_pcm_file(
        &aiff24_via_file,
        AudioFileFormat::Aiff,
        BitDepth::Bits24,
        48000,
        2,
        &[0.0, 1.0, -1.0, 0.5],
    )
    .expect("write_pcm_file aiff24");
    let dec24_via_file = decode_with_symphonia(&aiff24_via_file, "aiff");
    assert_eq!(
        dec24_via_file.i24_samples,
        vec![0, 8_388_607, -8_388_607, 4_194_304]
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_aiff_and_aifc_roundtrip_16_24_f32_unclamped() {
    let dir = temp_test_dir("aiff_roundtrip");

    // 16-bit stereo AIFF
    let aiff16_path = dir.join("test16.aiff");
    let pcm16 = vec![0i16, 30000, -30000, i16::MAX, i16::MIN, 777];
    write_pcm_i16(&aiff16_path, AudioFileFormat::Aiff, 44100, 2, &pcm16).expect("write aiff16");
    let raw16 = std::fs::read(&aiff16_path).expect("read aiff16");
    assert_eq!(&raw16[0..4], b"FORM");
    assert_eq!(&raw16[8..12], b"AIFF");
    let dec16 = decode_with_symphonia(&aiff16_path, "aiff");
    assert_eq!(dec16.sample_rate, 44100);
    assert_eq!(dec16.channels, 2);
    assert_eq!(dec16.bits_per_sample, Some(16));
    assert_eq!(dec16.i16_samples, pcm16);

    // 24-bit mono AIFF with odd sample count (tests odd SSND byte length + pad byte)
    let aiff24_path = dir.join("test24_odd.aiff");
    let pcm24 = vec![-8_388_608i32, 123_456, 8_388_607];
    write_pcm_i24(&aiff24_path, AudioFileFormat::Aiff, 48000, 1, &pcm24).expect("write aiff24");
    let raw24 = std::fs::read(&aiff24_path).expect("read aiff24");
    assert_eq!(raw24.len() % 2, 0, "AIFF file must be padded to even size");
    let dec24 = decode_with_symphonia(&aiff24_path, "aiff");
    assert_eq!(dec24.sample_rate, 48000);
    assert_eq!(dec24.channels, 1);
    assert_eq!(dec24.bits_per_sample, Some(24));
    assert_eq!(dec24.i24_samples, pcm24);

    // 32-bit float stereo AIFF-C (unclamped > 1.0 preserved!)
    let aifc32_path = dir.join("test32f.aifc");
    let pcm32 = vec![1.5f32, -2.5, 0.125, -0.875, 0.0, 3.25];
    write_pcm_f32(&aifc32_path, AudioFileFormat::Aiff, 44100, 2, &pcm32).expect("write aifc32f");
    let raw32 = std::fs::read(&aifc32_path).expect("read aifc32f");
    assert_eq!(&raw32[0..4], b"FORM");
    assert_eq!(&raw32[8..12], b"AIFC");
    assert!(raw32.windows(4).any(|w| w == b"FVER"));
    assert!(raw32.windows(4).any(|w| w == b"fl32"));

    let dec32 = decode_with_symphonia(&aifc32_path, "aiff");
    assert_eq!(
        dec32.codec,
        symphonia::core::codecs::audio::well_known::CODEC_ID_PCM_F32BE
    );
    assert_eq!(dec32.sample_rate, 44100);
    assert_eq!(dec32.channels, 2);
    assert_eq!(dec32.f32_samples, pcm32);
    assert_eq!(dec32.f32_samples[0], 1.5);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_flac_roundtrip_16_24_and_rejects_float32() {
    let dir = temp_test_dir("flac_roundtrip");

    // 16-bit stereo FLAC (5000 frames: spans a full 4096-frame block plus a 904-frame tail block)
    let flac16_path = dir.join("test16.flac");
    let pcm16: Vec<i16> = (0..5000)
        .flat_map(|i| {
            let t = i as f32 / 44100.0;
            let l = (16000.0 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()) as i16;
            let r = (12000.0 * (2.0 * std::f32::consts::PI * 660.0 * t).sin()) as i16;
            [l, r]
        })
        .collect();
    write_pcm_i16(&flac16_path, AudioFileFormat::Flac, 44100, 2, &pcm16).expect("write flac16");
    let dec16 = decode_with_symphonia(&flac16_path, "flac");
    assert_eq!(dec16.sample_rate, 44100);
    assert_eq!(dec16.channels, 2);
    assert_eq!(dec16.bits_per_sample, Some(16));
    assert_eq!(dec16.verify_ok, Some(true));
    let decoded_16: Vec<i16> = if !dec16.i16_samples.is_empty() {
        dec16.i16_samples
    } else {
        dec16
            .i32_samples
            .into_iter()
            .map(|s| (s >> 16) as i16)
            .collect()
    };
    assert_eq!(decoded_16, pcm16);

    // 24-bit mono FLAC (including full 24-bit extrema!)
    let flac24_path = dir.join("test24.flac");
    let mut pcm24: Vec<i32> = (0..512)
        .map(|i| {
            let t = i as f64 / 48000.0;
            (4_000_000.0 * (2.0 * std::f64::consts::PI * 330.0 * t).sin()) as i32
        })
        .collect();
    pcm24[0] = 8_388_607;
    pcm24[1] = -8_388_608;
    write_pcm_i24(&flac24_path, AudioFileFormat::Flac, 48000, 1, &pcm24).expect("write flac24");
    let dec24 = decode_with_symphonia(&flac24_path, "flac");
    assert_eq!(dec24.sample_rate, 48000);
    assert_eq!(dec24.channels, 1);
    assert_eq!(dec24.bits_per_sample, Some(24));
    assert_eq!(dec24.verify_ok, Some(true));
    let decoded_24: Vec<i32> = if !dec24.i24_samples.is_empty() {
        dec24.i24_samples
    } else {
        // Symphonia's FLAC decoder may emit S32 shifted or unshifted; check S24/S32 cleanly.
        dec24.i32_samples.into_iter().map(|s| s >> 8).collect()
    };
    assert_eq!(decoded_24, pcm24);

    // Float32 must be rejected cleanly for FLAC
    let flac_f32_path = dir.join("invalid_f32.flac");
    let err1 = write_pcm_f32(
        &flac_f32_path,
        AudioFileFormat::Flac,
        44100,
        2,
        &[0.5, -0.5],
    )
    .unwrap_err();
    assert!(matches!(err1, AudioEncodeError::UnsupportedFormat(_)));

    let err2 = write_pcm_file(
        &flac_f32_path,
        AudioFileFormat::Flac,
        BitDepth::Float32,
        44100,
        2,
        &[0.5, -0.5],
    )
    .unwrap_err();
    assert!(matches!(err2, AudioEncodeError::UnsupportedFormat(_)));

    // FLAC > 96 kHz rejected cleanly
    let err_rate = write_pcm_i16(
        &flac_f32_path,
        AudioFileFormat::Flac,
        192_000,
        1,
        &[0i16; 64],
    )
    .unwrap_err();
    assert!(matches!(err_rate, AudioEncodeError::UnsupportedFormat(_)));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_mp3_320_roundtrip_symphonia_and_validation() {
    use symphonia::core::codecs::audio::well_known::CODEC_ID_MP3;

    let dir = temp_test_dir("mp3_roundtrip");

    // 44.1 kHz stereo 1 kHz / 440 Hz tone
    let mp3_path = dir.join("tone320.mp3");
    let sr = 44100u32;
    let frames = 22050usize;
    let mut pcm = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        let t = i as f32 / sr as f32;
        pcm.push(0.4 * (2.0 * std::f32::consts::PI * 1000.0 * t).sin());
        pcm.push(0.4 * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
    }
    write_mp3_320(&mp3_path, sr, 2, &pcm).expect("write mp3 320");

    // Verify CBR 320 kbps MPEG-1 Layer III frame headers exist in bitstream (bitrate_idx = 0xE)
    let mp3_bytes = std::fs::read(&mp3_path).expect("read mp3");
    let has_320_frame = mp3_bytes.windows(4).any(|w| {
        w[0] == 0xFF
            && (w[1] & 0xFE) == 0xFA // MPEG-1 Layer III sync + version + layer
            && (w[2] >> 4) == 0x0E // 320 kbps index in MPEG-1 Layer III
    });
    assert!(has_320_frame, "expected 320 kbps MPEG-1 Layer III frame");

    let dec = decode_with_symphonia(&mp3_path, "mp3");
    assert_eq!(dec.codec, CODEC_ID_MP3);
    assert_eq!(dec.sample_rate, 44100);
    assert_eq!(dec.channels, 2);
    assert!(
        dec.f32_samples.len() >= frames,
        "decoded too few MP3 samples: {}",
        dec.f32_samples.len()
    );
    let rms: f32 =
        (dec.f32_samples.iter().map(|s| s * s).sum::<f32>() / dec.f32_samples.len() as f32).sqrt();
    assert!(
        rms > 0.15 && rms < 0.45,
        "decoded MP3 RMS out of expected range: {rms}"
    );

    // 48 kHz mono MP3 320 kbps
    let mono_path = dir.join("mono48.mp3");
    let mono_pcm: Vec<f32> = (0..12000)
        .map(|i| 0.35 * (2.0 * std::f32::consts::PI * 880.0 * (i as f32 / 48000.0)).sin())
        .collect();
    write_mp3_320(&mono_path, 48000, 1, &mono_pcm).expect("write mono mp3 320");
    let dec_mono = decode_with_symphonia(&mono_path, "mp3");
    assert_eq!(dec_mono.sample_rate, 48000);
    assert_eq!(dec_mono.channels, 1);
    assert!(!dec_mono.f32_samples.is_empty());

    // Unsupported rate / channels return clean errors
    let bad_path = dir.join("bad.mp3");
    assert!(matches!(
        write_mp3_320(&bad_path, 96000, 2, &[0.0; 8]),
        Err(AudioEncodeError::UnsupportedFormat(_))
    ));
    assert!(matches!(
        write_mp3_320(&bad_path, 44100, 0, &[]),
        Err(AudioEncodeError::InvalidInput(_))
    ));
    assert!(matches!(
        write_mp3_320(&bad_path, 44100, 3, &[0.0; 6]),
        Err(AudioEncodeError::InvalidInput(_))
    ));
    assert!(matches!(
        write_mp3_320(&bad_path, 44100, 2, &[0.0; 3]),
        Err(AudioEncodeError::InvalidInput(_))
    ));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_audio_encode_validation_errors() {
    let dir = temp_test_dir("validation");
    let p = dir.join("bad.wav");

    assert!(matches!(
        write_pcm_i16(&p, AudioFileFormat::Wav, 0, 2, &[0, 0]),
        Err(AudioEncodeError::InvalidInput(_))
    ));
    assert!(matches!(
        write_pcm_i16(&p, AudioFileFormat::Wav, 44100, 0, &[]),
        Err(AudioEncodeError::InvalidInput(_))
    ));
    assert!(matches!(
        write_pcm_i16(&p, AudioFileFormat::Wav, 44100, 3, &[0, 0, 0]),
        Err(AudioEncodeError::InvalidInput(_))
    ));
    assert!(matches!(
        write_pcm_f32(&p, AudioFileFormat::Aiff, 44100, 2, &[0.1, 0.2, 0.3]),
        Err(AudioEncodeError::InvalidInput(_))
    ));
    assert!(matches!(
        write_pcm_i24(&p, AudioFileFormat::Wav, 44100, 1, &[8_388_608]),
        Err(AudioEncodeError::InvalidInput(_))
    ));
    assert!(matches!(
        write_pcm_i24(&p, AudioFileFormat::Aiff, 44100, 1, &[-8_388_609]),
        Err(AudioEncodeError::InvalidInput(_))
    ));

    let _ = std::fs::remove_dir_all(&dir);
}
