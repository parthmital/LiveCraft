//! Audio file encoders for PCM containers (WAV, AIFF/AIFF-C, FLAC) and 320 kbps CBR MP3.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use flacenc::bitsink::ByteSink;
use flacenc::component::BitRepr;
use flacenc::error::Verify;
use flacenc::source::MemSource;

/// Minimum signed 24-bit integer sample value (`-2^23`).
const I24_MIN: i32 = -(1 << 23);

/// Maximum signed 24-bit integer sample value (`2^23 - 1`).
const I24_MAX: i32 = (1 << 23) - 1;

/// Apple AIFF-C specification version timestamp (`1990-05-23`, `0xA2805140`).
const AIFC_VERSION_1: u32 = 0xA280_5140;

/// Pascal string for `fl32` compression name (length 15 + 1 length byte = 16 bytes, even-aligned).
const AIFC_FL32_NAME: &[u8; 16] = b"\x0f32-bit float   ";

/// Output container format for uncompressed or lossless PCM audio files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFileFormat {
    /// Microsoft RIFF WAVE (`.wav`).
    Wav,
    /// Apple AIFF (16/24-bit integer) or AIFF-C (32-bit float) (`.aif` / `.aiff`).
    Aiff,
    /// Free Lossless Audio Codec (`.flac`, 16-bit and 24-bit only).
    Flac,
}

/// Sample bit depth and numeric representation for PCM encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitDepth {
    /// Signed 16-bit integer PCM.
    Bits16,
    /// Signed 24-bit integer PCM.
    Bits24,
    /// IEEE 754 32-bit floating-point PCM (unclamped).
    Float32,
}

/// Errors produced when validating or encoding audio files.
#[derive(Debug, thiserror::Error)]
pub enum AudioEncodeError {
    /// Underlying filesystem I/O failure.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// WAV writer error from `hound`.
    #[error("WAV encode error: {0}")]
    Wav(#[from] hound::Error),
    /// FLAC encoder failure from `flacenc`.
    #[error("FLAC encode error: {0}")]
    Flac(String),
    /// MP3 encoder failure from `rusty_mp3`.
    #[error("MP3 encode error: {0}")]
    Mp3(String),
    /// Requested container/bit-depth/sample-rate combination is unsupported.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),
    /// Caller supplied invalid audio parameters or out-of-range sample data.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

enum AiffSamples<'a> {
    Pcm16(&'a [i16]),
    Pcm24(&'a [i32]),
    Float32(&'a [f32]),
}

/// Encodes interleaved 32-bit floating-point samples into the requested format and bit depth.
///
/// For `BitDepth::Float32`, samples are written unclamped (preserving headroom above `1.0` and
/// below `-1.0`). For integer bit depths (`Bits16`, `Bits24`), samples are clamped to `[-1.0, 1.0]`
/// and rounded to the nearest integer sample.
pub fn write_pcm_file<P: AsRef<Path>>(
    path: P,
    format: AudioFileFormat,
    bit_depth: BitDepth,
    sample_rate: u32,
    channels: u16,
    samples: &[f32],
) -> Result<(), AudioEncodeError> {
    match bit_depth {
        BitDepth::Bits16 => {
            validate_common(sample_rate, channels, samples)?;
            let quantized: Vec<i16> = samples
                .iter()
                .map(|&s| {
                    let clamped = sanitize_for_int(s);
                    (clamped * 32767.0).round() as i16
                })
                .collect();
            write_pcm_i16(path, format, sample_rate, channels, &quantized)
        }
        BitDepth::Bits24 => {
            validate_common(sample_rate, channels, samples)?;
            let quantized: Vec<i32> = samples
                .iter()
                .map(|&s| {
                    let clamped = sanitize_for_int(s);
                    (clamped * 8_388_607.0).round() as i32
                })
                .collect();
            write_pcm_i24(path, format, sample_rate, channels, &quantized)
        }
        BitDepth::Float32 => write_pcm_f32(path, format, sample_rate, channels, samples),
    }
}

/// Writes interleaved signed 16-bit PCM samples to `path` in `format`.
pub fn write_pcm_i16<P: AsRef<Path>>(
    path: P,
    format: AudioFileFormat,
    sample_rate: u32,
    channels: u16,
    samples: &[i16],
) -> Result<(), AudioEncodeError> {
    let num_frames = validate_common(sample_rate, channels, samples)?;
    match format {
        AudioFileFormat::Wav => write_wav_i16(path.as_ref(), sample_rate, channels, samples),
        AudioFileFormat::Aiff => write_aiff_impl(
            path.as_ref(),
            sample_rate,
            channels,
            num_frames,
            AiffSamples::Pcm16(samples),
        ),
        AudioFileFormat::Flac => {
            let widened: Vec<i32> = samples.iter().map(|&s| i32::from(s)).collect();
            write_flac_pcm(path.as_ref(), sample_rate, channels, 16, &widened)
        }
    }
}

/// Writes interleaved signed 24-bit PCM samples (stored in `i32` within `[-2^23, 2^23 - 1]`) to
/// `path` in `format`.
pub fn write_pcm_i24<P: AsRef<Path>>(
    path: P,
    format: AudioFileFormat,
    sample_rate: u32,
    channels: u16,
    samples: &[i32],
) -> Result<(), AudioEncodeError> {
    let num_frames = validate_common(sample_rate, channels, samples)?;
    validate_i24_samples(samples)?;
    match format {
        AudioFileFormat::Wav => write_wav_i24(path.as_ref(), sample_rate, channels, samples),
        AudioFileFormat::Aiff => write_aiff_impl(
            path.as_ref(),
            sample_rate,
            channels,
            num_frames,
            AiffSamples::Pcm24(samples),
        ),
        AudioFileFormat::Flac => write_flac_pcm(path.as_ref(), sample_rate, channels, 24, samples),
    }
}

/// Writes interleaved 32-bit IEEE 754 floating-point samples (unclamped) to `path` in `format`.
///
/// Returns [`AudioEncodeError::UnsupportedFormat`] if `format` is [`AudioFileFormat::Flac`].
pub fn write_pcm_f32<P: AsRef<Path>>(
    path: P,
    format: AudioFileFormat,
    sample_rate: u32,
    channels: u16,
    samples: &[f32],
) -> Result<(), AudioEncodeError> {
    let num_frames = validate_common(sample_rate, channels, samples)?;
    match format {
        AudioFileFormat::Wav => write_wav_f32(path.as_ref(), sample_rate, channels, samples),
        AudioFileFormat::Aiff => write_aiff_impl(
            path.as_ref(),
            sample_rate,
            channels,
            num_frames,
            AiffSamples::Float32(samples),
        ),
        AudioFileFormat::Flac => Err(AudioEncodeError::UnsupportedFormat(
            "FLAC does not support 32-bit float samples".into(),
        )),
    }
}

/// Encodes interleaved 32-bit float audio as 320 kbps CBR MP3 via `rusty_mp3`.
///
/// MPEG-1 sample rates (`32000`, `44100`, `48000` Hz) encode at 320 kbps CBR. MPEG-2/2.5 sample
/// rates (`8000`, `11025`, `12000`, `16000`, `22050`, `24000` Hz) are also accepted and capped at
/// their standard maximum CBR bitrate (160 kbps) by `rusty_mp3`; all other sample rates return
/// [`AudioEncodeError::UnsupportedFormat`].
pub fn write_mp3_320<P: AsRef<Path>>(
    path: P,
    sample_rate: u32,
    channels: u16,
    interleaved_f32: &[f32],
) -> Result<(), AudioEncodeError> {
    validate_common(sample_rate, channels, interleaved_f32)?;
    if !matches!(
        sample_rate,
        8000 | 11025 | 12000 | 16000 | 22050 | 24000 | 32000 | 44100 | 48000
    ) {
        return Err(AudioEncodeError::UnsupportedFormat(format!(
            "MP3 does not support sample rate {sample_rate} Hz (supported MPEG rates: 8000, 11025, 12000, 16000, 22050, 24000, 32000, 44100, 48000)"
        )));
    }

    let mut encoder = rusty_mp3::Mp3Encoder::new(rusty_mp3::Mp3EncoderConfig {
        bitrate_kbps: 320,
        vbr_quality: None,
    });
    encoder
        .push_pcm_f32(interleaved_f32, channels, sample_rate)
        .map_err(|e| AudioEncodeError::Mp3(e.to_string()))?;
    encoder.finish();

    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);
    loop {
        match encoder.next_packet() {
            Ok(packet) => writer.write_all(&packet)?,
            Err(rusty_mp3::Error::Eof) => break,
            Err(rusty_mp3::Error::Again) => {
                return Err(AudioEncodeError::Mp3(
                    "MP3 encoder requested more input after finish".into(),
                ));
            }
            Err(e) => return Err(AudioEncodeError::Mp3(e.to_string())),
        }
    }
    writer.flush()?;
    Ok(())
}

fn sanitize_for_int(sample: f32) -> f32 {
    if sample.is_nan() {
        0.0
    } else {
        sample.clamp(-1.0, 1.0)
    }
}

fn validate_common<T>(
    sample_rate: u32,
    channels: u16,
    samples: &[T],
) -> Result<usize, AudioEncodeError> {
    if sample_rate == 0 {
        return Err(AudioEncodeError::InvalidInput(
            "sample_rate must be greater than 0".into(),
        ));
    }
    if channels != 1 && channels != 2 {
        return Err(AudioEncodeError::InvalidInput(format!(
            "channels must be 1 or 2, got {channels}"
        )));
    }
    let ch = usize::from(channels);
    if !samples.len().is_multiple_of(ch) {
        return Err(AudioEncodeError::InvalidInput(format!(
            "sample count ({}) must be a multiple of channel count ({channels})",
            samples.len()
        )));
    }
    Ok(samples.len() / ch)
}

fn validate_i24_samples(samples: &[i32]) -> Result<(), AudioEncodeError> {
    if let Some(&bad) = samples.iter().find(|&&s| !(I24_MIN..=I24_MAX).contains(&s)) {
        return Err(AudioEncodeError::InvalidInput(format!(
            "24-bit PCM sample {bad} is outside [{I24_MIN}, {I24_MAX}]"
        )));
    }
    Ok(())
}

fn write_wav_i16(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    samples: &[i16],
) -> Result<(), AudioEncodeError> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

fn write_wav_i24(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    samples: &[i32],
) -> Result<(), AudioEncodeError> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

fn write_wav_f32(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    samples: &[f32],
) -> Result<(), AudioEncodeError> {
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec)?;
    for &s in samples {
        writer.write_sample(s)?;
    }
    writer.finalize()?;
    Ok(())
}

/// Encodes an IEEE 754 `f64` as an 80-bit IEEE 754 extended-precision float in big-endian order
/// for the AIFF/AIFF-C `COMM` chunk sample-rate field.
fn f64_to_ieee754_extended_be(value: f64) -> [u8; 10] {
    let bits = value.to_bits();
    let sign = ((bits >> 48) as u16) & 0x8000;
    let exp64 = ((bits >> 52) & 0x7ff) as u32;
    let mant64 = bits & ((1u64 << 52) - 1);

    let (sign_exponent, fraction) = if exp64 == 0 {
        if mant64 == 0 {
            (sign, 0)
        } else {
            let lz = mant64.leading_zeros();
            let exp80 = (16383 - 1022 + 11 - lz) as u16;
            (sign | exp80, mant64 << lz)
        }
    } else if exp64 == 0x7ff {
        (sign | 0x7fff, if mant64 == 0 { 0 } else { u64::MAX })
    } else {
        let exp80 = (exp64 + 16383 - 1023) as u16;
        (sign | exp80, (1u64 << 63) | (mant64 << 11))
    };

    let mut out = [0u8; 10];
    out[0..2].copy_from_slice(&sign_exponent.to_be_bytes());
    out[2..10].copy_from_slice(&fraction.to_be_bytes());
    out
}

fn write_aiff_impl(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    num_frames: usize,
    payload: AiffSamples<'_>,
) -> Result<(), AudioEncodeError> {
    let num_frames_u32 = u32::try_from(num_frames).map_err(|_| {
        AudioEncodeError::InvalidInput("frame count exceeds AIFF 32-bit header limit".into())
    })?;

    let (is_aifc, sample_size, pcm_byte_len) = match payload {
        AiffSamples::Pcm16(s) => (false, 16u16, (s.len() as u64) * 2),
        AiffSamples::Pcm24(s) => (false, 24u16, (s.len() as u64) * 3),
        AiffSamples::Float32(s) => (true, 32u16, (s.len() as u64) * 4),
    };

    let ssnd_body_len_u64 = 8 + pcm_byte_len;
    let ssnd_body_len = u32::try_from(ssnd_body_len_u64).map_err(|_| {
        AudioEncodeError::InvalidInput("AIFF SSND chunk exceeds 4 GiB limit".into())
    })?;
    let ssnd_pad = pcm_byte_len & 1;

    let fver_total_len: u64 = if is_aifc { 12 } else { 0 };
    let comm_body_len: u32 = if is_aifc { 38 } else { 18 };
    let form_size_u64 =
        4 + fver_total_len + (8 + u64::from(comm_body_len)) + (8 + ssnd_body_len_u64 + ssnd_pad);
    let form_size = u32::try_from(form_size_u64).map_err(|_| {
        AudioEncodeError::InvalidInput("AIFF file exceeds 4 GiB FORM size limit".into())
    })?;

    let file = File::create(path)?;
    let mut w = BufWriter::new(file);

    w.write_all(b"FORM")?;
    w.write_all(&form_size.to_be_bytes())?;
    w.write_all(if is_aifc { b"AIFC" } else { b"AIFF" })?;

    if is_aifc {
        w.write_all(b"FVER")?;
        w.write_all(&4u32.to_be_bytes())?;
        w.write_all(&AIFC_VERSION_1.to_be_bytes())?;
    }

    w.write_all(b"COMM")?;
    w.write_all(&comm_body_len.to_be_bytes())?;
    w.write_all(&channels.to_be_bytes())?;
    w.write_all(&num_frames_u32.to_be_bytes())?;
    w.write_all(&sample_size.to_be_bytes())?;
    w.write_all(&f64_to_ieee754_extended_be(f64::from(sample_rate)))?;
    if is_aifc {
        w.write_all(b"fl32")?;
        w.write_all(AIFC_FL32_NAME)?;
    }

    w.write_all(b"SSND")?;
    w.write_all(&ssnd_body_len.to_be_bytes())?;
    w.write_all(&0u32.to_be_bytes())?; // offset
    w.write_all(&0u32.to_be_bytes())?; // blockSize

    match payload {
        AiffSamples::Pcm16(samples) => {
            for &s in samples {
                w.write_all(&s.to_be_bytes())?;
            }
        }
        AiffSamples::Pcm24(samples) => {
            for &s in samples {
                let b = s.to_be_bytes();
                w.write_all(&[b[1], b[2], b[3]])?;
            }
        }
        AiffSamples::Float32(samples) => {
            for &s in samples {
                w.write_all(&s.to_be_bytes())?;
            }
        }
    }

    if ssnd_pad == 1 {
        w.write_all(&[0u8])?;
    }

    w.flush()?;
    Ok(())
}

fn write_flac_pcm(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: usize,
    samples: &[i32],
) -> Result<(), AudioEncodeError> {
    if sample_rate > 96_000 {
        return Err(AudioEncodeError::UnsupportedFormat(format!(
            "FLAC encoder supports sample rates up to 96000 Hz, got {sample_rate} Hz"
        )));
    }

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| AudioEncodeError::Flac(e.to_string()))?;
    let source = MemSource::from_samples(
        samples,
        usize::from(channels),
        bits_per_sample,
        sample_rate as usize,
    );
    let mut flac_stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| AudioEncodeError::Flac(e.to_string()))?;
    // Keep STREAMINFO's min/max block size equal to the fixed block size so decoders (including
    // Symphonia) recognize fixed-blocking streams whose final frame is shorter than `block_size`.
    flac_stream
        .stream_info_mut()
        .set_block_sizes(config.block_size, config.block_size)
        .map_err(|e| AudioEncodeError::Flac(e.to_string()))?;

    let mut sink = ByteSink::new();
    flac_stream
        .write(&mut sink)
        .map_err(|e| AudioEncodeError::Flac(e.to_string()))?;

    std::fs::write(path, sink.as_slice())?;
    Ok(())
}
